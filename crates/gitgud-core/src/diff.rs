use gix::ObjectId;
use gix::diff::blob::{Algorithm, InternedInput, diff_with_slider_heuristics};
use gix::object::tree::diff::ChangeDetached;
use serde::{Deserialize, Serialize};

use crate::lfs::{lfs_is_hydrated, parse_lfs_pointer};
use crate::repo::{Change, Error, Repo, read_err};

/// Lines of context around each change, as `git diff` defaults to.
pub const CONTEXT: u32 = 3;
/// Git's binary check: a NUL byte in the first 8000 bytes.
const BINARY_SNIFF: usize = 8000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    /// The message after the subject line, trimmed; empty when there is none.
    pub body: String,
    pub author: Person,
    pub committer: Person,
    pub parents: Vec<String>,
    /// Changes against the first parent (or an empty tree for a root commit),
    /// without rename detection, sorted by path.
    pub files: Vec<FileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Person {
    pub name: String,
    pub email: String,
    /// Seconds since the Unix epoch.
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    pub change: Change,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinarySide {
    /// Content size in bytes (the LFS pointer's `size`, or raw blob length).
    pub size: u64,
    pub lfs: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    pub hydrated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryMeta {
    pub before: Option<BinarySide>,
    pub after: Option<BinarySide>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_meta: Option<BinaryMeta>,
}

/// A unified-diff hunk; starts are 1-based like `git diff`'s `@@` header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub kind: LineKind,
    /// 1-based line number in the old file; `None` for added lines.
    pub old: Option<u32>,
    /// 1-based line number in the new file; `None` for removed lines.
    pub new: Option<u32>,
    /// Without the line ending.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

impl Repo {
    /// Message, people, parents and changed files of commit `id` (full hex).
    ///
    /// # Errors
    /// [`Error::Read`] when `id` isn't a commit here.
    pub fn commit_detail(&self, id: &str) -> Result<CommitDetail, Error> {
        let repo = self.gix();
        let commit = find_commit(&repo, id)?;
        let message = commit.message().map_err(read_err)?;
        let author = commit.author().map_err(read_err)?;
        let committer = commit.committer().map_err(read_err)?;
        let parents: Vec<ObjectId> = commit.parent_ids().map(gix::Id::detach).collect();

        let new_tree = commit.tree().map_err(read_err)?;
        let old_tree = match parents.first() {
            Some(p) => repo
                .find_commit(*p)
                .map_err(read_err)?
                .tree()
                .map_err(read_err)?,
            None => repo.empty_tree(),
        };
        let changes = repo
            .diff_tree_to_tree(
                &old_tree,
                &new_tree,
                gix::diff::Options::default().with_rewrites(None),
            )
            .map_err(read_err)?;
        let mut files: Vec<FileChange> = changes.iter().filter_map(file_change).collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(CommitDetail {
            id: commit.id.to_string(),
            short_id: commit.id.to_hex_with_len(7).to_string(),
            subject: message.summary().to_string(),
            body: message
                .body
                .map(|b| b.to_string().trim().to_owned())
                .unwrap_or_default(),
            author: person(&author)?,
            committer: person(&committer)?,
            parents: parents.iter().map(ToString::to_string).collect(),
            files,
        })
    }

    /// The text diff of `path` in commit `id` against its first parent.
    ///
    /// # Errors
    /// [`Error::Read`] when `id` isn't a commit or `path` didn't change in it.
    pub fn commit_file_diff(&self, id: &str, path: &str) -> Result<FileDiff, Error> {
        self.commit_file_diff_with(id, path, false)
    }

    pub(crate) fn commit_file_versions(
        &self,
        id: &str,
        path: &str,
    ) -> Result<crate::worktree::Versions, Error> {
        let repo = self.gix();
        let commit = find_commit(&repo, id)?;
        let new = blob_at(&repo, &commit, path)?;
        let old = match commit.parent_ids().next() {
            Some(p) => blob_at(&repo, &p.object().map_err(read_err)?.into_commit(), path)?,
            None => None,
        };
        if old.is_none() && new.is_none() {
            return Err(Error::Read(format!(
                "{path} is not in commit {id} or its parent"
            )));
        }
        Ok(crate::worktree::Versions { old, new })
    }

    /// As [`Repo::commit_file_diff`], optionally ignoring whitespace differences.
    ///
    /// # Errors
    /// [`Error::Read`] when `id` isn't a commit or `path` didn't change in it.
    pub fn commit_file_diff_with(
        &self,
        id: &str,
        path: &str,
        ignore_whitespace: bool,
    ) -> Result<FileDiff, Error> {
        let v = self.commit_file_versions(id, path)?;
        Ok(file_diff_in_repo(
            Some(self),
            path,
            v.old.as_deref(),
            v.new.as_deref(),
            ignore_whitespace,
        ))
    }
}

pub(crate) fn is_binary_or_lfs(data: &[u8]) -> bool {
    is_binary(data) || parse_lfs_pointer(data).is_some()
}

pub(crate) fn binary_side_info(repo: Option<&Repo>, bytes: &[u8]) -> BinarySide {
    if let Some((oid, size)) = parse_lfs_pointer(bytes) {
        let hydrated = repo.is_some_and(|r| lfs_is_hydrated(r, &oid));
        return BinarySide {
            size,
            lfs: true,
            oid: Some(oid),
            hydrated,
        };
    }
    BinarySide {
        size: bytes.len() as u64,
        lfs: false,
        oid: None,
        hydrated: true,
    }
}

pub(crate) fn file_diff_in_repo(
    repo: Option<&Repo>,
    path: &str,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
    ignore_whitespace: bool,
) -> FileDiff {
    if old.is_some_and(is_binary_or_lfs) || new.is_some_and(is_binary_or_lfs) {
        return FileDiff {
            path: path.to_owned(),
            binary: true,
            hunks: Vec::new(),
            binary_meta: Some(BinaryMeta {
                before: old.map(|b| binary_side_info(repo, b)),
                after: new.map(|b| binary_side_info(repo, b)),
            }),
        };
    }
    FileDiff {
        path: path.to_owned(),
        binary: false,
        hunks: text_hunks_with(
            &String::from_utf8_lossy(old.unwrap_or_default()),
            &String::from_utf8_lossy(new.unwrap_or_default()),
            ignore_whitespace,
        ),
        binary_meta: None,
    }
}

pub(crate) fn find_commit<'r>(
    repo: &'r gix::Repository,
    id: &str,
) -> Result<gix::Commit<'r>, Error> {
    let oid = ObjectId::from_hex(id.as_bytes())
        .map_err(|_| Error::Read(format!("{id:?} is not a commit id")))?;
    repo.find_commit(oid).map_err(read_err)
}

fn person(sig: &gix::actor::SignatureRef<'_>) -> Result<Person, Error> {
    Ok(Person {
        name: sig.name.to_string(),
        email: sig.email.to_string(),
        time: sig.time().map_err(read_err)?.seconds,
    })
}

pub(crate) fn file_change(change: &ChangeDetached) -> Option<FileChange> {
    let (location, kind) = match change {
        ChangeDetached::Addition {
            location,
            entry_mode,
            ..
        } if !entry_mode.is_tree() => (location, Change::Added),
        ChangeDetached::Deletion {
            location,
            entry_mode,
            ..
        } if !entry_mode.is_tree() => (location, Change::Deleted),
        ChangeDetached::Modification {
            location,
            previous_entry_mode,
            entry_mode,
            ..
        } if !entry_mode.is_tree() || !previous_entry_mode.is_tree() => {
            let kind = if previous_entry_mode.kind() == entry_mode.kind() {
                Change::Modified
            } else {
                Change::TypeChange
            };
            (location, kind)
        }
        _ => return None,
    };
    Some(FileChange {
        path: location.to_string(),
        change: kind,
    })
}

pub(crate) fn blob_at(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    path: &str,
) -> Result<Option<Vec<u8>>, Error> {
    let tree = commit.tree().map_err(read_err)?;
    let Some(entry) = tree.lookup_entry_by_path(path).map_err(read_err)? else {
        return Ok(None);
    };
    if !entry.mode().is_blob() {
        return Ok(None);
    }
    let blob = repo.find_blob(entry.object_id()).map_err(read_err)?;
    Ok(Some(blob.data.clone()))
}

pub(crate) fn is_binary(data: &[u8]) -> bool {
    data[..data.len().min(BINARY_SNIFF)].contains(&0)
}

/// Unified hunks with [`CONTEXT`] lines, merging changes closer than twice that,
/// like `git diff --diff-algorithm=histogram`. Line texts have no line ending.
pub fn text_hunks(old: &str, new: &str) -> Vec<Hunk> {
    text_hunks_with(old, new, false)
}

/// As [`text_hunks`], optionally ignoring whitespace differences.
#[must_use]
pub fn text_hunks_with(old: &str, new: &str, ignore_whitespace: bool) -> Vec<Hunk> {
    hunks(old, new, false, ignore_whitespace)
}

/// As [`text_hunks`], but every line keeps its exact ending (`\n`, `\r\n`, or
/// none on a last line without one), so the hunk can be written back as a patch.
pub(crate) fn hunks_keeping_eol(old: &str, new: &str) -> Vec<Hunk> {
    hunks(old, new, true, false)
}

fn hunks(old: &str, new: &str, keep_eol: bool, ignore_whitespace: bool) -> Vec<Hunk> {
    let norm_pair = if ignore_whitespace {
        let old_raw: Vec<&str> = old.split_inclusive('\n').collect();
        let new_raw: Vec<&str> = new.split_inclusive('\n').collect();
        let norm_old = normalize_ws(&old_raw);
        let norm_new = normalize_ws(&new_raw);
        Some((old_raw, new_raw, norm_old, norm_new))
    } else {
        None
    };

    let input = match &norm_pair {
        Some((_, _, norm_old, norm_new)) => {
            InternedInput::new(norm_old.as_str(), norm_new.as_str())
        }
        None => InternedInput::new(old, new),
    };
    let diff = diff_with_slider_heuristics(Algorithm::Histogram, &input);
    let changes: Vec<_> = diff.hunks().collect();
    let raw_before = |idx: usize| -> &str {
        let raw = match &norm_pair {
            Some((old_raw, _, _, _)) => old_raw[idx],
            None => input.interner[input.before[idx]],
        };
        if keep_eol { raw } else { strip_eol(raw) }
    };
    let raw_after = |idx: usize| -> &str {
        let raw = match &norm_pair {
            Some((_, new_raw, _, _)) => new_raw[idx],
            None => input.interner[input.after[idx]],
        };
        if keep_eol { raw } else { strip_eol(raw) }
    };
    let (old_len, new_len) = (len_u32(input.before.len()), len_u32(input.after.len()));

    let mut hunks = Vec::new();
    let mut i = 0;
    while i < changes.len() {
        let mut j = i;
        while j + 1 < changes.len()
            && changes[j + 1].before.start - changes[j].before.end <= 2 * CONTEXT
        {
            j += 1;
        }
        let group = &changes[i..=j];
        let lead = group[0].before.start.min(CONTEXT);
        let (old_from, new_from) = (group[0].before.start - lead, group[0].after.start - lead);
        let trail = (old_len - group[j - i].before.end).min(CONTEXT);
        let (old_to, new_to) = (
            group[j - i].before.end + trail,
            group[j - i].after.end + trail,
        );

        let mut lines = Vec::new();
        let (mut o, mut n) = (old_from, new_from);
        for change in group {
            while o < change.before.start {
                lines.push(line(
                    LineKind::Context,
                    Some(o),
                    Some(n),
                    raw_before(o as usize),
                ));
                o += 1;
                n += 1;
            }
            for k in change.before.clone() {
                lines.push(line(
                    LineKind::Removed,
                    Some(k),
                    None,
                    raw_before(k as usize),
                ));
            }
            for k in change.after.clone() {
                lines.push(line(LineKind::Added, None, Some(k), raw_after(k as usize)));
            }
            o = change.before.end;
            n = change.after.end;
        }
        while o < old_to {
            lines.push(line(
                LineKind::Context,
                Some(o),
                Some(n),
                raw_before(o as usize),
            ));
            o += 1;
            n += 1;
        }
        debug_assert!(new_to <= new_len);
        let (old_lines, new_lines) = (old_to - old_from, new_to - new_from);
        hunks.push(Hunk {
            // git numbers an empty side by the line before it.
            old_start: if old_lines == 0 {
                old_from
            } else {
                old_from + 1
            },
            old_lines,
            new_start: if new_lines == 0 {
                new_from
            } else {
                new_from + 1
            },
            new_lines,
            lines,
        });
        i = j + 1;
    }
    hunks
}

fn normalize_ws(lines: &[&str]) -> String {
    let mut out = String::new();
    for &l in lines {
        let mut first = true;
        for word in l.split_whitespace() {
            if !first {
                out.push(' ');
            }
            first = false;
            out.push_str(word);
        }
        out.push('\n');
    }
    out
}

fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
    Line {
        kind,
        old: old.map(|o| o + 1),
        new: new.map(|n| n + 1),
        text: text.to_owned(),
    }
}

fn strip_eol(s: &str) -> &str {
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).expect("file has more than u32::MAX lines")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_whitespace_only_changes_while_preserving_original_text() {
        let old = "fn main() {\n    let x = 1;\n}\n";
        let new = "fn main() {\n        let x = 1;\n}\n";
        assert_eq!(text_hunks_with(old, new, false).len(), 1);
        assert!(text_hunks_with(old, new, true).is_empty());

        let mixed = "fn main() {\n        let x = 2;\n}\n";
        let hunks = text_hunks_with(old, mixed, true);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].lines[2].text, "        let x = 2;");
    }
}
