use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use gix::bstr::BStr;
use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::diff::{
    FileDiff, Hunk, LineKind, file_diff_in_repo, hunks_keeping_eol, is_binary_or_lfs,
};
use crate::lfs::parse_lfs_pointer;
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

/// Identifies specific changed line indices inside one hunk for line-level staging/discard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSelect {
    pub hunk: HunkHeader,
    pub lines: Vec<usize>,
}

/// Both versions of a path for one side, `None` where the path doesn't exist.
pub(crate) struct Versions {
    pub(crate) old: Option<Vec<u8>>,
    pub(crate) new: Option<Vec<u8>>,
}

impl Repo {
    /// The diff of `path` on one side of the working copy. The work tree side is
    /// compared after git's to-git filters (autocrlf, eol), as `git diff` does.
    ///
    /// # Errors
    /// [`Error::Read`] when the path is conflicted or can't be read.
    pub fn worktree_file_diff(&self, path: &str, side: Side) -> Result<FileDiff, Error> {
        self.worktree_file_diff_with(path, side, false)
    }

    /// As [`Repo::worktree_file_diff`], optionally ignoring whitespace differences.
    ///
    /// # Errors
    /// [`Error::Read`] when the path is conflicted or can't be read.
    pub fn worktree_file_diff_with(
        &self,
        path: &str,
        side: Side,
        ignore_whitespace: bool,
    ) -> Result<FileDiff, Error> {
        let v = self.versions(path, side)?;
        let mut diff = file_diff_in_repo(
            Some(self),
            path,
            v.old.as_deref(),
            v.new.as_deref(),
            ignore_whitespace,
        );
        if side == Side::Unstaged
            && let Some(meta) = diff.binary_meta.as_mut()
            && let Some(after) = meta.after.as_mut()
            && after.lfs
            && !after.hydrated
            && worktree_has_non_pointer(self.root(), path)
        {
            after.hydrated = true;
        }
        Ok(diff)
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

    /// Stages only the selected line indices within one hunk of `path`.
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn stage_lines(&self, git: &GitCli, path: &str, select: &LineSelect) -> Result<(), Error> {
        self.apply_lines_to(git, path, Side::Unstaged, select, true)
    }

    /// Unstages one hunk of the staged diff of `path`.
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn unstage_hunk(&self, git: &GitCli, path: &str, hunk: HunkHeader) -> Result<(), Error> {
        self.apply_hunk(git, path, Side::Staged, hunk)
    }

    /// Unstages only the selected line indices within one hunk of `path`.
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn unstage_lines(
        &self,
        git: &GitCli,
        path: &str,
        select: &LineSelect,
    ) -> Result<(), Error> {
        self.apply_lines_to(git, path, Side::Staged, select, true)
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

    /// Discards only the selected line indices within one hunk of `path`. Destructive: record it
    /// with [`crate::oplog::OpLog::record_destructive`].
    ///
    /// # Errors
    /// As [`Repo::stage_hunk`].
    pub fn discard_lines(
        &self,
        git: &GitCli,
        path: &str,
        select: &LineSelect,
    ) -> Result<(), Error> {
        self.apply_lines_to(git, path, Side::Unstaged, select, false)
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
        if is_binary_or_lfs(&old) || is_binary_or_lfs(&new) {
            return Err(Error::Refused(format!(
                "{path} is binary: stage or unstage the whole file"
            )));
        }
        let (old_s, new_s) = (String::from_utf8_lossy(&old), String::from_utf8_lossy(&new));
        let hunk = hunks_keeping_eol(&old_s, &new_s)
            .into_iter()
            .find(|h| HunkHeader::from(h) == wanted)
            .ok_or_else(|| Error::Stale(path.to_owned()))?;

        let raw = raw_lines(path, &hunk, &old, &new)?;
        let patch = patch(path, &hunk, &raw, false);
        let mut args = vec!["apply", "--whitespace=nowarn"];
        if to_index {
            args.push("--cached");
        }
        if side == Side::Staged || !to_index {
            args.push("--reverse");
        }
        args.push("-");
        git.run(self.root(), &args, Some(&patch)).map(drop)
    }

    fn apply_lines_to(
        &self,
        git: &GitCli,
        path: &str,
        side: Side,
        select: &LineSelect,
        to_index: bool,
    ) -> Result<(), Error> {
        let v = self.versions(path, side)?;
        let (Some(old), Some(new)) = (v.old, v.new) else {
            return Err(Error::Refused(format!(
                "{path} is new or deleted: stage or unstage the whole file"
            )));
        };
        if is_binary_or_lfs(&old) || is_binary_or_lfs(&new) {
            return Err(Error::Refused(format!(
                "{path} is binary: stage or unstage the whole file"
            )));
        }
        let (old_s, new_s) = (String::from_utf8_lossy(&old), String::from_utf8_lossy(&new));
        let hunk = hunks_keeping_eol(&old_s, &new_s)
            .into_iter()
            .find(|h| HunkHeader::from(h) == select.hunk)
            .ok_or_else(|| Error::Stale(path.to_owned()))?;

        let raw = raw_lines(path, &hunk, &old, &new)?;
        let chosen: BTreeSet<usize> = select.lines.iter().copied().collect();
        let forward = side == Side::Unstaged && to_index;
        let patch = patch_lines(path, &hunk, &raw, &chosen, forward)?;
        let mut args = vec!["apply", "--whitespace=nowarn"];
        if to_index {
            args.push("--cached");
        }
        args.push("-");
        git.run(self.root(), &args, Some(&patch)).map(drop)
    }

    pub(crate) fn versions(&self, path: &str, side: Side) -> Result<Versions, Error> {
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

    /// Returns every hunk (preserving exact line endings) along with its unified patch for `path` on `side`.
    pub(crate) fn side_hunks_with_patches(
        &self,
        path: &str,
        side: Side,
    ) -> Result<Vec<(Hunk, Vec<u8>)>, Error> {
        let v = self.versions(path, side)?;
        let old = v.old.unwrap_or_default();
        let new = v.new.unwrap_or_default();
        if is_binary_or_lfs(&old) || is_binary_or_lfs(&new) {
            return Ok(Vec::new());
        }
        let (old_s, new_s) = (String::from_utf8_lossy(&old), String::from_utf8_lossy(&new));
        let is_new = old.is_empty();
        hunks_keeping_eol(&old_s, &new_s)
            .into_iter()
            .map(|h| {
                let raw = raw_lines(path, &h, &old, &new)?;
                let patch = patch(path, &h, &raw, is_new);
                Ok((h, patch))
            })
            .collect()
    }

    /// The work tree file converted to its git form, `None` when it's gone.
    pub(crate) fn worktree_as_git(
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

pub(crate) fn worktree_has_non_pointer(root: &Path, path: &str) -> bool {
    let Ok(mut file) = std::fs::File::open(root.join(path)) else {
        return false;
    };
    let mut head = [0u8; 256];
    let Ok(n) = file.read(&mut head) else {
        return false;
    };
    parse_lfs_pointer(&head[..n]).is_none()
}

/// Each line of `hunk` as its exact bytes in `old` / `new`. Hunks are found on
/// text decoded lossily, so non-UTF-8 bytes would otherwise come back as U+FFFD.
fn raw_lines<'a>(
    path: &str,
    hunk: &Hunk,
    old: &'a [u8],
    new: &'a [u8],
) -> Result<Vec<&'a [u8]>, Error> {
    let start = |start: u32, len: u32| {
        if len == 0 {
            start as usize
        } else {
            start.saturating_sub(1) as usize
        }
    };
    let mut old_lines = old
        .split_inclusive(|b| *b == b'\n')
        .skip(start(hunk.old_start, hunk.old_lines));
    let mut new_lines = new
        .split_inclusive(|b| *b == b'\n')
        .skip(start(hunk.new_start, hunk.new_lines));
    let mut raw = Vec::with_capacity(hunk.lines.len());
    for line in &hunk.lines {
        let bytes = match line.kind {
            LineKind::Context => {
                new_lines.next();
                old_lines.next()
            }
            LineKind::Removed => old_lines.next(),
            LineKind::Added => new_lines.next(),
        };
        match bytes {
            Some(b) if String::from_utf8_lossy(b) == line.text => raw.push(b),
            _ => return Err(Error::Stale(path.to_owned())),
        }
    }
    Ok(raw)
}

fn push_line(out: &mut Vec<u8>, prefix: u8, bytes: &[u8]) {
    out.push(prefix);
    out.extend_from_slice(bytes);
    if !bytes.ends_with(b"\n") {
        out.extend_from_slice(b"\n\\ No newline at end of file\n");
    }
}

/// A one-hunk patch git can apply, from `raw` (the hunk's lines as exact bytes,
/// see [`raw_lines`]); `new_file` patches against `/dev/null`.
pub(crate) fn patch(path: &str, hunk: &Hunk, raw: &[&[u8]], new_file: bool) -> Vec<u8> {
    let old_name = if new_file {
        "/dev/null".to_owned()
    } else {
        quote(&format!("a/{path}"))
    };
    let mut out = format!(
        "--- {old_name}\n+++ {}\n@@ -{},{} +{},{} @@\n",
        quote(&format!("b/{path}")),
        hunk.old_start,
        hunk.old_lines,
        hunk.new_start,
        hunk.new_lines
    )
    .into_bytes();
    for (line, bytes) in hunk.lines.iter().zip(raw) {
        let prefix = match line.kind {
            LineKind::Context => b' ',
            LineKind::Added => b'+',
            LineKind::Removed => b'-',
        };
        push_line(&mut out, prefix, bytes);
    }
    out
}

/// Builds a partial patch applying only the selected line indices within `hunk`.
/// When `forward` is `true`, the patch transforms `old` toward `new` (for `stage_lines`).
/// When `forward` is `false`, the patch transforms `new` directly back toward `old`
/// (for `unstage_lines` and `discard_lines` without `--reverse`).
fn patch_lines(
    path: &str,
    hunk: &Hunk,
    raw: &[&[u8]],
    selected: &BTreeSet<usize>,
    forward: bool,
) -> Result<Vec<u8>, Error> {
    let mut body = Vec::new();
    let mut before_count = 0u32;
    let mut after_count = 0u32;
    let mut applied_any = false;

    for (idx, line) in hunk.lines.iter().enumerate() {
        let is_selected = selected.contains(&idx);
        let emitted = match (forward, line.kind) {
            (_, LineKind::Context) => {
                before_count += 1;
                after_count += 1;
                Some(' ')
            }
            (true, LineKind::Removed) => {
                if is_selected {
                    applied_any = true;
                    before_count += 1;
                    Some('-')
                } else {
                    before_count += 1;
                    after_count += 1;
                    Some(' ')
                }
            }
            (true, LineKind::Added) => {
                if is_selected {
                    applied_any = true;
                    after_count += 1;
                    Some('+')
                } else {
                    None
                }
            }
            (false, LineKind::Added) => {
                if is_selected {
                    applied_any = true;
                    before_count += 1;
                    Some('-')
                } else {
                    before_count += 1;
                    after_count += 1;
                    Some(' ')
                }
            }
            (false, LineKind::Removed) => {
                if is_selected {
                    applied_any = true;
                    after_count += 1;
                    Some('+')
                } else {
                    None
                }
            }
        };
        if let Some(prefix) = emitted {
            push_line(&mut body, prefix as u8, raw[idx]);
        }
    }

    if !applied_any {
        return Err(Error::Refused("no changed lines selected".into()));
    }

    let before_start = if forward {
        hunk.old_start
    } else {
        hunk.new_start
    };
    let after_start = if after_count == 0 {
        before_start.saturating_sub(1)
    } else if before_start == 0 {
        1
    } else {
        before_start
    };
    let mut out = format!(
        "--- {}\n+++ {}\n@@ -{before_start},{before_count} +{after_start},{after_count} @@\n",
        quote(&format!("a/{path}")),
        quote(&format!("b/{path}")),
    )
    .into_bytes();
    out.extend_from_slice(&body);
    Ok(out)
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
