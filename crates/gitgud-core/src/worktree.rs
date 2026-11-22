use std::io::Read;
use std::path::Path;

use gix::bstr::BStr;
use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::diff::{FileDiff, Hunk, LineKind, file_diff, hunks_keeping_eol, is_binary};
use crate::repo::{Error, Repo, read_err};

/// Which half of a working-copy change: HEAD → index, or index → work tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Staged,
    Unstaged,
}

/// Identifies a hunk by its `@@` header, so a stale view can't stage the wrong lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkHeader {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

impl From<&Hunk> for HunkHeader {
    fn from(h: &Hunk) -> Self {
        Self {
            old_start: h.old_start,
            old_lines: h.old_lines,
            new_start: h.new_start,
            new_lines: h.new_lines,
        }
    }
}

/// Both versions of a path for one side, `None` where the path doesn't exist.
struct Versions {
    old: Option<Vec<u8>>,
    new: Option<Vec<u8>>,
}

impl Repo {
    /// The diff of `path` on one side of the working copy. The work tree side is
    /// compared after git's to-git filters (autocrlf, eol), as `git diff` does.
    ///
    /// # Errors
    /// [`Error::Read`] when the path is conflicted or can't be read.
    pub fn worktree_file_diff(&self, path: &str, side: Side) -> Result<FileDiff, Error> {
        let v = self.versions(path, side)?;
        Ok(file_diff(
            path,
            &v.old.unwrap_or_default(),
            &v.new.unwrap_or_default(),
        ))
    }

    /// Stages one hunk of the unstaged diff of `path`.
    ///
    /// # Errors
    /// [`Error::Stale`] when the diff no longer has that hunk, [`Error::Command`]
    /// when git refuses the patch, [`Error::Refused`] for paths that only stage
    /// whole (new, deleted, binary or conflicted files).
    pub fn stage_hunk(&self, git: &GitCli, path: &str, hunk: HunkHeader) -> Result<(), Error> {
        self.apply_hunk(git, path, Side::Unstaged, hunk)
    }

    /// Unstages one hunk of the staged diff of `path`.
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn unstage_hunk(&self, git: &GitCli, path: &str, hunk: HunkHeader) -> Result<(), Error> {
        self.apply_hunk(git, path, Side::Staged, hunk)
    }

    /// Throws away one hunk of the unstaged diff of `path`: the work tree file
    /// gets that part of its staged version back. Destructive: record it with
    /// [`crate::oplog::OpLog::record_destructive`].
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn discard_hunk(&self, git: &GitCli, path: &str, hunk: HunkHeader) -> Result<(), Error> {
        self.apply_hunk_to(git, path, Side::Unstaged, hunk, false)
    }

    /// Throws away the unstaged changes of `paths`: tracked files go back to their
    /// staged version (staged changes stay), untracked files are deleted.
    /// Destructive: record it with [`crate::oplog::OpLog::record_destructive`].
    ///
    /// # Errors
    /// [`Error::Command`] when git can't restore a file, [`Error::Read`] when a
    /// file can't be deleted.
    pub fn discard(&self, git: &GitCli, paths: &[String]) -> Result<(), Error> {
        let index = self.gix().index_or_empty().map_err(read_err)?;
        let (tracked, untracked): (Vec<&String>, Vec<&String>) = paths
            .iter()
            .partition(|p| index.entry_by_path(BStr::new(p.as_str())).is_some());
        for path in untracked {
            std::fs::remove_file(self.root().join(path)).map_err(read_err)?;
        }
        if tracked.is_empty() {
            return Ok(());
        }
        let list: Vec<&str> = tracked.iter().map(|p| p.as_str()).collect();
        git.run(
            self.root(),
            &[
                "--literal-pathspecs",
                "restore",
                "--worktree",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(list.join("\0").as_bytes()),
        )
        .map(drop)
    }

    fn apply_hunk(
        &self,
        git: &GitCli,
        path: &str,
        side: Side,
        wanted: HunkHeader,
    ) -> Result<(), Error> {
        self.apply_hunk_to(git, path, side, wanted, true)
    }

    /// Applies the reverse (staged side) or forward (unstaged side) of one hunk to
    /// the index, or with `to_index == false` reverses it in the work tree.
    fn apply_hunk_to(
        &self,
        git: &GitCli,
        path: &str,
        side: Side,
        wanted: HunkHeader,
        to_index: bool,
    ) -> Result<(), Error> {
        let v = self.versions(path, side)?;
        let (Some(old), Some(new)) = (v.old, v.new) else {
            return Err(Error::Refused(format!(
                "{path} is new or deleted: stage or unstage the whole file"
            )));
        };
        if is_binary(&old) || is_binary(&new) {
            return Err(Error::Refused(format!(
                "{path} is binary: stage or unstage the whole file"
            )));
        }
        let (old, new) = (String::from_utf8_lossy(&old), String::from_utf8_lossy(&new));
        let hunk = hunks_keeping_eol(&old, &new)
            .into_iter()
            .find(|h| HunkHeader::from(h) == wanted)
            .ok_or_else(|| Error::Stale(path.to_owned()))?;

        let patch = patch(path, &hunk);
        let mut args = vec!["apply", "--whitespace=nowarn"];
        if to_index {
            args.push("--cached");
        }
        if side == Side::Staged || !to_index {
            args.push("--reverse");
        }
        args.push("-");
        git.run(self.root(), &args, Some(patch.as_bytes()))
            .map(drop)
    }

    fn versions(&self, path: &str, side: Side) -> Result<Versions, Error> {
        let repo = self.gix();
        let index = repo.index_or_empty().map_err(read_err)?;
        let entry = index.entry_by_path(BStr::new(path));
        if entry.is_some_and(|e| e.stage_raw() != 0) {
            return Err(Error::Refused(format!(
                "{path} is conflicted: resolve it, then stage the whole file"
            )));
        }
        let index_blob = entry
            .map(|e| {
                repo.find_blob(e.id)
                    .map(|b| b.data.clone())
                    .map_err(read_err)
            })
            .transpose()?;

        match side {
            Side::Staged => {
                let head = match repo.head_tree() {
                    Ok(tree) => match tree.lookup_entry_by_path(path).map_err(read_err)? {
                        Some(e) if e.mode().is_blob() => Some(
                            repo.find_blob(e.object_id())
                                .map_err(read_err)?
                                .data
                                .clone(),
                        ),
                        _ => None,
                    },
                    // Unborn HEAD: nothing committed yet.
                    Err(_) if repo.head().map_err(read_err)?.is_unborn() => None,
                    Err(e) => return Err(read_err(e)),
                };
                Ok(Versions {
                    old: head,
                    new: index_blob,
                })
            }
            Side::Unstaged => Ok(Versions {
                old: index_blob,
                new: self.worktree_as_git(&repo, path)?,
            }),
        }
    }

    /// The work tree file converted to its git form, `None` when it's gone.
    fn worktree_as_git(
        &self,
        repo: &gix::Repository,
        path: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
        let file = match std::fs::File::open(self.root().join(path)) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(read_err(e)),
        };
        let (mut pipeline, index) = repo.filter_pipeline(None).map_err(read_err)?;
        let outcome = pipeline
            .convert_to_git(file, Path::new(path), &index)
            .map_err(read_err)?;
        let mut out = Vec::new();
        use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
        match outcome {
            ToGitOutcome::Unchanged(mut r) => r.read_to_end(&mut out).map(drop),
            ToGitOutcome::Process(mut r) => r.read_to_end(&mut out).map(drop),
            ToGitOutcome::Buffer(b) => {
                out.extend_from_slice(b);
                Ok(())
            }
        }
        .map_err(read_err)?;
        Ok(Some(out))
    }
}

/// A one-hunk patch git can apply; lines keep their exact endings.
fn patch(path: &str, hunk: &Hunk) -> String {
    let range = |start: u32, len: u32| format!("{start},{len}");
    let mut out = format!(
        "--- {}\n+++ {}\n@@ -{} +{} @@\n",
        quote(&format!("a/{path}")),
        quote(&format!("b/{path}")),
        range(hunk.old_start, hunk.old_lines),
        range(hunk.new_start, hunk.new_lines)
    );
    for line in &hunk.lines {
        out.push(match line.kind {
            LineKind::Context => ' ',
            LineKind::Added => '+',
            LineKind::Removed => '-',
        });
        out.push_str(&line.text);
        if !line.text.ends_with('\n') {
            out.push_str("\n\\ No newline at end of file\n");
        }
    }
    out
}

/// Git's C-style quoting for paths with characters a patch header can't hold raw.
fn quote(path: &str) -> String {
    if !path
        .chars()
        .any(|c| matches!(c, '"' | '\\' | '\t' | '\n') || c.is_control())
    {
        return path.to_owned();
    }
    let mut out = String::from("\"");
    for c in path.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
