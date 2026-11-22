use std::collections::HashSet;
use std::fs;

use gix::ObjectId;
use gix::object::tree::diff::ChangeDetached;
use serde::Serialize;

use crate::cli::{GitCli, command_error, failure_message};
use crate::diff::{CommitDetail, FileChange, FileDiff, Person, blob_at, file_diff};
use crate::repo::{Change, EntryKind, Error, Repo, read_err};

/// One entry from `refs/stash` (`stash@{index}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub index: usize,
    pub id: String,
    pub short_id: String,
    pub message: String,
    pub branch: String,
    pub time: i64,
    pub parent_id: String,
}

impl Repo {
    /// Stashes in stack order (`stash@{0}` first). Zero git processes.
    ///
    /// # Errors
    /// [`Error::Read`] when `refs/stash` or its reflog can't be read.
    pub fn stashes(&self) -> Result<Vec<StashEntry>, Error> {
        let repo = self.gix();
        let Some(mut stash_ref) = repo.try_find_reference("refs/stash").map_err(read_err)? else {
            return Ok(Vec::new());
        };
        let log_path = repo.common_dir().join("logs/refs/stash");
        let log = match fs::read_to_string(&log_path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(read_err(e)),
        };

        let mut entries = Vec::new();
        if let Some(text) = log {
            for (index, line) in text
                .lines()
                .rev()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
            {
                if let Some(entry) = parse_reflog_line(&repo, index, line)? {
                    entries.push(entry);
                }
            }
            return Ok(entries);
        }

        // Fallback if reflog is missing despite refs/stash existing.
        let tip = stash_ref.peel_to_id().map_err(read_err)?.detach();
        let commit = repo.find_commit(tip).map_err(read_err)?;
        let parent_id = commit
            .parent_ids()
            .next()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let raw_msg = commit.message().map_err(read_err)?.summary().to_string();
        let time = commit
            .author()
            .map_err(read_err)?
            .time()
            .map_err(read_err)?
            .seconds;
        let (branch, message) = split_stash_message(&raw_msg);
        Ok(vec![StashEntry {
            index: 0,
            id: tip.to_string(),
            short_id: tip.to_hex_with_len(7).to_string(),
            message,
            branch,
            time,
            parent_id,
        }])
    }

    /// Files and metadata of `stash@{index}`, including `-u` untracked files.
    ///
    /// # Errors
    /// [`Error::Refused`] when `index` doesn't exist; [`Error::Read`] otherwise.
    pub fn stash_detail(&self, index: usize) -> Result<CommitDetail, Error> {
        let entry = self
            .stashes()?
            .into_iter()
            .nth(index)
            .ok_or_else(|| Error::Refused(format!("stash@{{{index}}} does not exist")))?;
        let repo = self.gix();
        let oid = ObjectId::from_hex(entry.id.as_bytes()).map_err(read_err)?;
        let commit = repo.find_commit(oid).map_err(read_err)?;
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
        let mut files: Vec<FileChange> = changes.iter().filter_map(stash_file_change).collect();

        if let Some(untracked_id) = parents.get(2) {
            let empty = repo.empty_tree();
            let untracked_tree = repo
                .find_commit(*untracked_id)
                .map_err(read_err)?
                .tree()
                .map_err(read_err)?;
            let extra = repo
                .diff_tree_to_tree(
                    &empty,
                    &untracked_tree,
                    gix::diff::Options::default().with_rewrites(None),
                )
                .map_err(read_err)?;
            let seen: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();
            for change in extra.iter().filter_map(stash_file_change) {
                if !seen.contains(&change.path) {
                    files.push(change);
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(CommitDetail {
            id: entry.id,
            short_id: entry.short_id,
            subject: entry.message,
            body: String::new(),
            author: Person {
                name: author.name.to_string(),
                email: author.email.to_string(),
                time: entry.time,
            },
            committer: Person {
                name: committer.name.to_string(),
                email: committer.email.to_string(),
                time: entry.time,
            },
            parents: parents
                .first()
                .map(|p| vec![p.to_string()])
                .unwrap_or_default(),
            files,
        })
    }

    /// Diff of `path` inside `stash@{index}` (tracked or untracked `-u` file).
    ///
    /// # Errors
    /// [`Error::Refused`] when `index` doesn't exist; [`Error::Read`] when `path`
    /// is not in that stash.
    pub fn stash_file_diff(&self, index: usize, path: &str) -> Result<FileDiff, Error> {
        let entry = self
            .stashes()?
            .into_iter()
            .nth(index)
            .ok_or_else(|| Error::Refused(format!("stash@{{{index}}} does not exist")))?;
        let repo = self.gix();
        let oid = ObjectId::from_hex(entry.id.as_bytes()).map_err(read_err)?;
        let commit = repo.find_commit(oid).map_err(read_err)?;
        let parents: Vec<ObjectId> = commit.parent_ids().map(gix::Id::detach).collect();

        let new = blob_at(&repo, &commit, path)?;
        let old = match parents.first() {
            Some(p) => blob_at(&repo, &repo.find_commit(*p).map_err(read_err)?, path)?,
            None => None,
        };
        if old.is_some() || new.is_some() {
            return Ok(file_diff(
                path,
                &old.unwrap_or_default(),
                &new.unwrap_or_default(),
            ));
        }
        if let Some(untracked_id) = parents.get(2) {
            let untracked_commit = repo.find_commit(*untracked_id).map_err(read_err)?;
            if let Some(blob) = blob_at(&repo, &untracked_commit, path)? {
                return Ok(file_diff(path, &[], &blob));
            }
        }
        Err(Error::Read(format!("{path} is not in stash@{{{index}}}")))
    }

    /// Paths in the working copy that [`Repo::stash_save`] will reset or remove.
    ///
    /// # Errors
    /// [`Error::Refused`] when there is nothing to stash.
    pub fn stash_save_paths(&self, include_untracked: bool) -> Result<Vec<String>, Error> {
        let paths: Vec<String> = self
            .status()?
            .entries
            .into_iter()
            .filter(|e| include_untracked || e.kind != EntryKind::Untracked)
            .map(|e| e.path)
            .collect();
        if paths.is_empty() {
            return Err(Error::Refused("no local changes to stash".into()));
        }
        Ok(paths)
    }

    /// Paths touched by `stash@{index}`, so [`Repo::stash_apply`] / [`Repo::stash_pop`]
    /// can snapshot them before writing.
    ///
    /// # Errors
    /// See [`Repo::stash_detail`].
    pub fn stash_paths(&self, index: usize) -> Result<Vec<String>, Error> {
        Ok(self
            .stash_detail(index)?
            .files
            .into_iter()
            .map(|f| f.path)
            .collect())
    }

    /// Saves working-copy changes into a new stash.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn stash_save(
        &self,
        git: &GitCli,
        message: Option<&str>,
        include_untracked: bool,
    ) -> Result<(), Error> {
        let mut args = vec!["stash", "push", "-q"];
        if include_untracked {
            args.push("--include-untracked");
        }
        let msg_flag = message
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(|m| format!("--message={m}"));
        if let Some(flag) = &msg_flag {
            args.push(flag);
        }
        git.run(self.root(), &args, None).map(drop)
    }

    /// Applies `stash@{index}` to the working copy. Returns `Ok` even when the
    /// apply left conflict stages in the index so the op log can undo it.
    ///
    /// # Errors
    /// [`Error::Command`] when git refused before modifying anything.
    pub fn stash_apply(&self, git: &GitCli, index: usize) -> Result<(), Error> {
        let spec = format!("stash@{{{index}}}");
        self.run_stash_apply(git, &["stash", "apply", "-q", &spec])
    }

    /// Applies `stash@{index}` and removes it on a clean apply.
    ///
    /// # Errors
    /// [`Error::Command`] when git refused before modifying anything.
    pub fn stash_pop(&self, git: &GitCli, index: usize) -> Result<(), Error> {
        let spec = format!("stash@{{{index}}}");
        self.run_stash_apply(git, &["stash", "pop", "-q", &spec])
    }

    /// Removes `stash@{index}` from the stash list.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn stash_drop(&self, git: &GitCli, index: usize) -> Result<(), Error> {
        let spec = format!("stash@{{{index}}}");
        git.run(self.root(), &["stash", "drop", "-q", &spec], None)
            .map(drop)
    }

    fn run_stash_apply(&self, git: &GitCli, args: &[&str]) -> Result<(), Error> {
        let (code, out, err) = git.run_status(self.root(), args, None)?;
        if code == 0 {
            return Ok(());
        }
        if !self.conflicted_paths()?.is_empty() {
            return Ok(());
        }
        Err(command_error(args, failure_message(&out, &err)))
    }
}

fn parse_reflog_line(
    repo: &gix::Repository,
    index: usize,
    line: &str,
) -> Result<Option<StashEntry>, Error> {
    let (meta, raw_msg) = match line.split_once('\t') {
        Some((m, msg)) => (m, msg),
        None => (line, ""),
    };
    let mut parts = meta.split_whitespace();
    let _old = parts.next();
    let Some(new_hex) = parts.next() else {
        return Ok(None);
    };
    let rest: Vec<&str> = parts.collect();
    let parsed_time = rest
        .len()
        .checked_sub(2)
        .and_then(|i| rest[i].parse::<i64>().ok());

    let Ok(oid) = ObjectId::from_hex(new_hex.as_bytes()) else {
        return Ok(None);
    };
    let Ok(commit) = repo.find_commit(oid) else {
        return Ok(None);
    };
    let parent_id = commit
        .parent_ids()
        .next()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let time = match parsed_time {
        Some(t) => t,
        None => {
            commit
                .author()
                .map_err(read_err)?
                .time()
                .map_err(read_err)?
                .seconds
        }
    };
    let (branch, message) = split_stash_message(raw_msg);
    Ok(Some(StashEntry {
        index,
        id: oid.to_string(),
        short_id: oid.to_hex_with_len(7).to_string(),
        message,
        branch,
        time,
        parent_id,
    }))
}

fn split_stash_message(raw: &str) -> (String, String) {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("On ")
        && let Some((branch, msg)) = rest.split_once(": ")
    {
        return (branch.to_owned(), msg.to_owned());
    }
    if let Some(rest) = raw.strip_prefix("WIP on ")
        && let Some((branch, _)) = rest.split_once(": ")
    {
        return (branch.to_owned(), raw.to_owned());
    }
    (String::new(), raw.to_owned())
}

fn stash_file_change(change: &ChangeDetached) -> Option<FileChange> {
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
