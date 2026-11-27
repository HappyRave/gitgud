use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};
use crate::snapshot::Snapshots;

/// Entries are kept for at least this long...
const KEEP_SECONDS: i64 = 30 * 24 * 60 * 60;
/// ...or at least this many, whichever keeps more (spec §3.3).
const KEEP_ENTRIES: usize = 2000;

/// The per-repo undo log for actions that move refs or the index (spec §3.3):
/// before and after each action it keeps HEAD, the branch tip and a copy of the
/// `.git/index` file. No git process runs to record an entry.
pub struct OpLog {
    dir: PathBuf,
    git_dir: PathBuf,
}

/// One recorded action, newest last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    /// Seconds since the Unix epoch.
    pub time: i64,
    /// What the user did, e.g. `Stage a.txt`.
    pub action: String,
    /// Undone entries can be redone until a new action is recorded.
    pub undone: bool,
    /// Work-tree paths a destructive action changed; undo restores exactly these.
    #[serde(default)]
    pub paths: Vec<String>,
    before: State,
    after: State,
}

/// What the op-log panel shows of an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpEntry {
    pub id: u64,
    pub time: i64,
    pub action: String,
    pub undone: bool,
    /// Files a destructive action changed, which "Restore these files" puts back.
    pub paths: Vec<String>,
}

/// Disk footprint and prunable count of a repository's op log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpLogStats {
    pub entry_count: usize,
    pub total_bytes: u64,
    pub prunable_count: usize,
    pub reclaimable_bytes: u64,
}

/// An operation recorded in git's reflog outside gitgud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    pub id: String,
    pub short_id: String,
    pub selector: String,
    pub action: String,
    pub time: i64,
}

impl From<&Entry> for OpEntry {
    fn from(e: &Entry) -> Self {
        Self {
            id: e.id,
            time: e.time,
            action: e.action.clone(),
            undone: e.undone,
            paths: e.paths.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    head: HeadState,
    /// File in the op-log folder holding the index; `None` when there was none.
    index: Option<String>,
    /// Snapshot tree of the work tree, for destructive actions only.
    #[serde(default)]
    worktree: Option<String>,
    /// git's files for a merge in progress (`MERGE_HEAD`...), so undo can put a
    /// merge back into, or out of, progress.
    #[serde(default)]
    merge: Vec<(String, String)>,
    /// During a rebase HEAD is detached: the branch being rebased (full ref) and its tip.
    #[serde(default)]
    rebasing: Option<(String, Option<String>)>,
    /// Every local branch (full ref) and its tip, sorted, so actions that move
    /// other branches (`rebase --update-refs`, rename, create) undo them too.
    #[serde(default)]
    branches: Vec<(String, String)>,
    /// `refs/stash` tip OID, saved when present.
    #[serde(default)]
    stash_ref: Option<String>,
    /// Contents of `.git/logs/refs/stash` so stash stack entries survive undo/redo.
    #[serde(default)]
    stash_log: Option<String>,
}

/// Branches an entry moved, created or deleted.
fn moved_branches(entry: &Entry) -> Vec<String> {
    let mut names: Vec<String> = entry
        .before
        .branches
        .iter()
        .chain(&entry.after.branches)
        .map(|(n, _)| n.clone())
        .collect();
    names.sort();
    names.dedup();
    names.retain(|n| tip_in(&entry.before, n) != tip_in(&entry.after, n));
    names
}

fn tip_in<'a>(state: &'a State, name: &str) -> Option<&'a str> {
    state
        .branches
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t.as_str())
}

/// The files that make a merge or sequencer op "in progress", relative to the git dir.
const MERGE_FILES: [&str; 8] = [
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "gitgud/merge.json",
    "gitgud/rebase.json",
    "gitgud/sequencer.json",
];

/// git's folder for a rebase in progress; its files are kept like [`MERGE_FILES`].
const REBASE_DIR: &str = "rebase-merge";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HeadState {
    /// HEAD on a branch (full ref name); `tip` is `None` before its first commit.
    Branch {
        name: String,
        tip: Option<String>,
    },
    Detached {
        id: String,
    },
}

impl OpLog {
    pub fn for_repo(repo: &Repo) -> Self {
        let git_dir = repo.gix().git_dir().to_owned();
        Self {
            dir: git_dir.join("gitgud").join("oplog"),
            git_dir,
        }
    }

    /// Runs `op` and records it as `action`, if it succeeds.
    ///
    /// # Errors
    /// `op`'s error (nothing is recorded), or [`Error::Read`] when the op-log
    /// folder can't be written.
    pub fn record<T>(
        &self,
        repo: &Repo,
        action: &str,
        op: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.record_inner(repo, action, None, op)
    }

    /// As [`OpLog::record`], for an action that can lose work-tree content in
    /// `paths` (discard, ...): the work tree is snapshotted before and after, so
    /// undo can put those files back.
    ///
    /// # Errors
    /// As [`OpLog::record`], or a snapshot's git error (then `op` doesn't run).
    pub fn record_destructive<T>(
        &self,
        repo: &Repo,
        git: &GitCli,
        action: &str,
        paths: &[String],
        op: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.record_inner(repo, action, Some((git, paths)), op)
    }

    fn record_inner<T>(
        &self,
        repo: &Repo,
        action: &str,
        destructive: Option<(&GitCli, &[String])>,
        op: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        // Undone entries are dropped by a new action, but only once the log that
        // forgets them is saved: an action that fails leaves redo as it was.
        let (undone, mut entries): (Vec<Entry>, Vec<Entry>) =
            self.load()?.into_iter().partition(|e| e.undone);
        let id = entries
            .iter()
            .chain(&undone)
            .map(|e| e.id)
            .max()
            .map_or(1, |n| n + 1);
        let git = destructive.map(|(g, _)| g);
        let before = self.capture(repo, git, id, "before")?;
        let (value, label) = match op() {
            Ok(v) => (Ok(v), action.to_owned()),
            Err(e) => (Err(e), format!("{action} (failed)")),
        };
        let after = match self.capture(repo, git, id, "after") {
            Ok(after) => after,
            Err(capture) => {
                self.remove_files(&before);
                return value.and(Err(capture));
            }
        };
        // A failed action that changed nothing isn't worth an entry; one that
        // stopped halfway is, so Ctrl+Z can take back what it did.
        if value.is_err() && self.same_state(&before, &after)? {
            self.remove_files(&before);
            self.remove_files(&after);
            return value;
        }
        entries.push(Entry {
            id,
            time: now(),
            action: label,
            undone: false,
            paths: destructive.map(|(_, p)| p.to_vec()).unwrap_or_default(),
            before,
            after,
        });
        let pruned_snapshot = self.prune(&mut entries);
        self.save(&entries)?;
        let mut dropped_snapshot = false;
        for e in &undone {
            self.remove_files(&e.before);
            self.remove_files(&e.after);
            dropped_snapshot |= e.before.worktree.is_some() || e.after.worktree.is_some();
        }
        if (dropped_snapshot || pruned_snapshot)
            && let Some(g) = git
        {
            Snapshots::sync_kept(repo, g, &kept_snapshot_trees(&entries))?;
        }
        value
    }

    /// Whether nothing the op log tracks differs between `a` and `b`.
    fn same_state(&self, a: &State, b: &State) -> Result<bool, Error> {
        let index = |s: &State| match &s.index {
            Some(name) => index_entries(&self.dir.join(name)),
            None => Ok(Vec::new()),
        };
        Ok(a.head == b.head
            && a.branches == b.branches
            && a.stash_ref == b.stash_ref
            && a.stash_log == b.stash_log
            && a.merge == b.merge
            && a.rebasing == b.rebasing
            && a.worktree == b.worktree
            && index(a)? == index(b)?)
    }

    /// Every entry, oldest first.
    ///
    /// # Errors
    /// [`Error::Read`] when the log can't be read.
    pub fn entries(&self) -> Result<Vec<Entry>, Error> {
        self.load()
    }

    /// Puts back the files a destructive entry changed, as they were just before
    /// it, without undoing anything recorded after it.
    ///
    /// # Errors
    /// [`Error::Refused`] when the entry is unknown or changed no files.
    pub fn restore_entry_files(
        &self,
        repo: &Repo,
        git: &GitCli,
        entry_id: u64,
    ) -> Result<(), Error> {
        let entries = self.load()?;
        let entry = entries
            .iter()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| Error::Refused(format!("op-log entry {entry_id} is gone")))?;
        match (&entry.before.worktree, entry.paths.is_empty()) {
            (Some(tree), false) => restore_files(repo, git, tree, &entry.paths),
            _ => Err(Error::Refused(format!(
                "\"{}\" changed no files to restore",
                entry.action
            ))),
        }
    }

    /// Puts the refs and index back to before the newest action that isn't undone.
    /// Returns that action's label.
    ///
    /// # Errors
    /// [`Error::Refused`] when there's nothing to undo, the repo changed outside
    /// gitgud since (then undoing could lose that change), or another git holds
    /// the index lock; [`Error::Command`] when git can't move a ref.
    pub fn undo(&self, repo: &Repo, git: &GitCli) -> Result<String, Error> {
        self.step(repo, git, true)
    }

    /// Re-applies the oldest undone action. Returns its label.
    ///
    /// # Errors
    /// As [`OpLog::undo`].
    pub fn redo(&self, repo: &Repo, git: &GitCli) -> Result<String, Error> {
        self.step(repo, git, false)
    }

    /// Undoes (`undo`) or redoes one entry. Everything that can refuse runs before
    /// anything moves: the index lock is taken first and held until the index is
    /// written, and the refs move in one transaction, so a refusal changes nothing.
    fn step(&self, repo: &Repo, git: &GitCli, undo: bool) -> Result<String, Error> {
        let mut entries = self.load()?;
        let found = if undo {
            entries.iter().rposition(|e| !e.undone)
        } else {
            entries.iter().position(|e| e.undone)
        };
        let (verb, done) = if undo {
            ("undo", "undone")
        } else {
            ("redo", "redone")
        };
        let pos = found.ok_or_else(|| Error::Refused(format!("nothing to {verb}")))?;
        let entry = &entries[pos];
        let (from, to) = if undo {
            (&entry.after, &entry.before)
        } else {
            (&entry.before, &entry.after)
        };
        let moved = moved_branches(entry);
        let stash_moved = stash_changed(entry);
        let lock = IndexLock::take(&self.git_dir)?;
        self.ensure_at(repo, git, entry, from, &moved, stash_moved, verb)?;
        let index = to
            .index
            .as_ref()
            .map(|name| fs::read(self.dir.join(name)).map_err(read_err))
            .transpose()?;
        let message = format!("gitgud: {verb} {}", entry.action);
        move_refs(repo, git, to, &moved, stash_moved, &message)?;
        if stash_moved {
            restore_stash_log(repo, to)?;
        }
        self.restore_merge_files(to)?;
        lock.commit(index.as_deref())?;

        let action = entry.action.clone();
        let paths = entry.paths.clone();
        let tree = to.worktree.clone();
        entries[pos].undone = undo;
        self.save(&entries)?;
        if let (Some(tree), false) = (tree, paths.is_empty()) {
            restore_files(repo, git, &tree, &paths).map_err(|e| {
                Error::Refused(format!(
                    "\"{action}\" was {done}, but its files couldn't be put back: {e}"
                ))
            })?;
        }
        Ok(action)
    }

    fn capture(
        &self,
        repo: &Repo,
        snapshot: Option<&GitCli>,
        id: u64,
        when: &str,
    ) -> Result<State, Error> {
        fs::create_dir_all(&self.dir).map_err(read_err)?;
        let index = self.git_dir.join("index");
        let copy = if index.exists() {
            let name = format!("{id}-{when}.index");
            fs::copy(&index, self.dir.join(&name)).map_err(read_err)?;
            Some(name)
        } else {
            None
        };
        let worktree = snapshot
            .map(|git| Snapshots::for_repo(repo).take(repo, git))
            .transpose()?;
        let merge = self.merge_files()?;
        let rebasing = match merge.iter().find(|(n, _)| n == "rebase-merge/head-name") {
            Some((_, name)) => {
                let name = name.trim().to_owned();
                let tip = branch_tip(repo, &name)?;
                Some((name, tip))
            }
            None => None,
        };
        Ok(State {
            head: head_state(repo)?,
            index: copy,
            worktree,
            merge,
            rebasing,
            branches: branch_tips(repo)?,
            stash_ref: branch_tip(repo, "refs/stash")?,
            stash_log: read_optional(&repo.gix().common_dir().join("logs/refs/stash"))?,
        })
    }

    fn merge_files(&self) -> Result<Vec<(String, String)>, Error> {
        let mut found = Vec::new();
        for name in MERGE_FILES {
            match fs::read_to_string(self.git_dir.join(name)) {
                Ok(text) => found.push((name.to_owned(), text)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(read_err(e)),
            }
        }
        let dir = match fs::read_dir(self.git_dir.join(REBASE_DIR)) {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
            Err(e) => return Err(read_err(e)),
        };
        let mut files = Vec::new();
        for entry in dir {
            let entry = entry.map_err(read_err)?;
            if entry.file_type().map_err(read_err)?.is_file() {
                let text = fs::read_to_string(entry.path()).map_err(read_err)?;
                files.push((
                    format!("{REBASE_DIR}/{}", entry.file_name().to_string_lossy()),
                    text,
                ));
            }
        }
        files.sort();
        found.extend(files);
        Ok(found)
    }

    fn restore_merge_files(&self, to: &State) -> Result<(), Error> {
        for name in MERGE_FILES {
            let path = self.git_dir.join(name);
            match to.merge.iter().find(|(n, _)| n == name) {
                Some((_, text)) => fs::write(&path, text).map_err(read_err)?,
                None => match fs::remove_file(&path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
                    _ => {}
                },
            }
        }
        let dir = self.git_dir.join(REBASE_DIR);
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
            _ => {}
        }
        let prefix = format!("{REBASE_DIR}/");
        let rebase: Vec<_> = to
            .merge
            .iter()
            .filter_map(|(n, t)| n.strip_prefix(&prefix).map(|f| (f, t)))
            .collect();
        if !rebase.is_empty() {
            fs::create_dir_all(&dir).map_err(read_err)?;
            for (file, text) in rebase {
                fs::write(dir.join(file), text).map_err(read_err)?;
            }
        }
        Ok(())
    }

    /// Refuses when HEAD, the staged content, or (for destructive actions) the
    /// touched files differ from `expected`, one side of `entry`: something
    /// happened outside gitgud that the op log didn't see, and undoing over it
    /// would lose it.
    #[allow(clippy::too_many_arguments)]
    fn ensure_at(
        &self,
        repo: &Repo,
        git: &GitCli,
        entry: &Entry,
        expected: &State,
        moved: &[String],
        stash_moved: bool,
        verb: &str,
    ) -> Result<(), Error> {
        let (paths, action) = (&entry.paths, &entry.action);
        let refused = || {
            Error::Refused(format!(
                "can't {verb} \"{action}\": the repository changed outside gitgud since"
            ))
        };
        let here = head_state(repo)?;
        let same_index = index_entries(&self.git_dir.join("index"))?
            == match &expected.index {
                Some(name) => {
                    let copy = self.dir.join(name);
                    if !copy.exists() {
                        return Err(Error::Read(format!(
                            "the op log's copy of the index ({name}) is missing"
                        )));
                    }
                    index_entries(&copy)?
                }
                None => Vec::new(),
            };
        let files = self.merge_files()?;
        let marker = |files: &[(String, String)]| -> Vec<(String, String)> {
            files
                .iter()
                .filter(|(n, _)| {
                    n == "MERGE_HEAD"
                        || n == "CHERRY_PICK_HEAD"
                        || n == "REVERT_HEAD"
                        || n == "rebase-merge/msgnum"
                        || n == "rebase-merge/orig-head"
                })
                .map(|(n, t)| (n.clone(), t.trim().to_owned()))
                .collect()
        };
        let same_merge = marker(&files) == marker(&expected.merge);
        let same_rebasing = match &expected.rebasing {
            Some((name, tip)) => branch_tip(repo, name)? == *tip,
            None => true,
        };
        let mut same_branches = true;
        for name in moved {
            same_branches &= branch_tip(repo, name)?.as_deref() == tip_in(expected, name);
        }
        let same_stash = if stash_moved {
            branch_tip(repo, "refs/stash")? == expected.stash_ref
                && read_optional(&repo.gix().common_dir().join("logs/refs/stash"))?
                    == expected.stash_log
        } else {
            true
        };
        if here != expected.head
            || !same_index
            || !same_merge
            || !same_rebasing
            || !same_branches
            || !same_stash
        {
            return Err(refused());
        }
        if let (Some(tree), false) = (&expected.worktree, paths.is_empty()) {
            let now = worktree_blobs(repo, git, paths)?;
            for (path, now) in paths.iter().zip(now) {
                if now != blob_in(repo, tree, path)? {
                    return Err(refused());
                }
            }
        }
        Ok(())
    }

    /// Lists recent entries in `.git/gitgud/oplog` and computes storage + prune preview.
    ///
    /// # Errors
    /// [`Error::Read`] when reading log files fails.
    pub fn stats(&self, repo: &Repo) -> Result<OpLogStats, Error> {
        self.stats_with_limits(repo, KEEP_ENTRIES, KEEP_SECONDS)
    }

    /// Computes op-log storage and prune preview under custom retention limits.
    ///
    /// # Errors
    /// [`Error::Read`] when reading log files fails.
    pub fn stats_with_limits(
        &self,
        repo: &Repo,
        keep_min: usize,
        max_age_secs: i64,
    ) -> Result<OpLogStats, Error> {
        let entries = self.load()?;
        Ok(self.compute_stats(repo, &entries, keep_min, max_age_secs))
    }

    /// Prunes entries exceeding both `keep_min` and `max_age_secs` (defaults: 2000, 30 days),
    /// unlinks dropped snapshot trees from [`crate::snapshot::SNAPSHOT_REF`], runs auto-gc,
    /// and returns updated storage stats.
    ///
    /// # Errors
    /// [`Error::Read`] or [`Error::Command`] on disk/git error.
    pub fn prune_now(
        &self,
        repo: &Repo,
        git: &GitCli,
        keep_min: usize,
        max_age_secs: i64,
    ) -> Result<OpLogStats, Error> {
        let mut entries = self.load()?;
        let cutoff = now() - max_age_secs;
        let mut changed = false;
        while entries.len() > keep_min && entries[0].time < cutoff {
            let old = entries.remove(0);
            self.remove_files(&old.before);
            self.remove_files(&old.after);
            changed = true;
        }
        if changed {
            fs::create_dir_all(&self.dir).map_err(read_err)?;
            self.save(&entries)?;
        }
        let synced = Snapshots::sync_kept(repo, git, &kept_snapshot_trees(&entries))?;
        if changed || synced {
            let _ = git.run(repo.root(), &["gc", "--auto"], None);
        }
        Ok(self.compute_stats(repo, &entries, keep_min, max_age_secs))
    }

    fn compute_stats(
        &self,
        repo: &Repo,
        entries: &[Entry],
        keep_min: usize,
        max_age_secs: i64,
    ) -> OpLogStats {
        let mut total_bytes = 0u64;
        if let Ok(read) = fs::read_dir(&self.dir) {
            for e in read.flatten() {
                if let Ok(m) = e.metadata()
                    && m.is_file()
                {
                    total_bytes += m.len();
                }
            }
        }
        let cutoff = now() - max_age_secs;
        let mut prune_end = 0usize;
        let mut reclaimable_bytes = 0u64;
        for (idx, e) in entries.iter().enumerate() {
            if entries.len() - idx > keep_min && e.time < cutoff {
                prune_end = idx + 1;
                reclaimable_bytes +=
                    self.state_file_size(&e.before) + self.state_file_size(&e.after);
            } else {
                break;
            }
        }
        let (lfs_total, lfs_reclaimable) = oplog_only_lfs_bytes(repo, entries, prune_end);
        OpLogStats {
            entry_count: entries.len(),
            total_bytes: total_bytes + lfs_total,
            prunable_count: prune_end,
            reclaimable_bytes: reclaimable_bytes + lfs_reclaimable,
        }
    }

    fn state_file_size(&self, state: &State) -> u64 {
        state
            .index
            .as_ref()
            .and_then(|name| fs::metadata(self.dir.join(name)).ok())
            .map_or(0, |m| m.len())
    }

    fn prune(&self, entries: &mut Vec<Entry>) -> bool {
        let cutoff = now() - KEEP_SECONDS;
        let mut dropped_snapshot = false;
        while entries.len() > KEEP_ENTRIES && entries[0].time < cutoff {
            let old = entries.remove(0);
            dropped_snapshot |= old.before.worktree.is_some() || old.after.worktree.is_some();
            self.remove_files(&old.before);
            self.remove_files(&old.after);
        }
        dropped_snapshot
    }

    fn remove_files(&self, state: &State) {
        if let Some(name) = &state.index {
            let _ = fs::remove_file(self.dir.join(name));
        }
    }

    fn load(&self) -> Result<Vec<Entry>, Error> {
        match fs::read(self.dir.join("log.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(read_err),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(read_err(e)),
        }
    }

    fn save(&self, entries: &[Entry]) -> Result<(), Error> {
        let tmp = self.dir.join("log.json.tmp");
        fs::write(&tmp, serde_json::to_vec(entries).map_err(read_err)?).map_err(read_err)?;
        fs::rename(&tmp, self.dir.join("log.json")).map_err(read_err)
    }
}

/// An op-log label for an action on files: `Stage a.txt`, `Stage 3 files`.
pub fn label_paths(verb: &str, paths: &[String]) -> String {
    match paths {
        [one] => format!("{verb} {one}"),
        many => format!("{verb} {} files", many.len()),
    }
}

/// An op-log label for a commit: its subject, cut at 60 characters.
pub fn label_commit(message: &str) -> String {
    label_with_subject("Commit", message)
}

/// An op-log label for amending a commit: its subject, cut at 60 characters.
pub fn label_amend(message: &str) -> String {
    label_with_subject("Amend", message)
}

impl Repo {
    /// Recent git reflog moves made outside gitgud (terminal, IDE, coding agents).
    ///
    /// # Errors
    /// [`Error::Command`] when `git reflog` fails.
    pub fn outside_reflog(&self, git: &GitCli, limit: usize) -> Result<Vec<ReflogEntry>, Error> {
        let repo = self.gix();
        if repo.head().map_err(read_err)?.is_unborn() {
            return Ok(Vec::new());
        }
        let n = limit.clamp(1, 200).to_string();
        let out = git.run(
            self.root(),
            &[
                "reflog",
                "--date=unix",
                "--format=%H%x1f%h%x1f%gd%x1f%gs%x1f%ct",
                "-n",
                &n,
            ],
            None,
        )?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let cols: Vec<&str> = line.split('\x1f').collect();
                if cols.len() < 5 {
                    return None;
                }
                if cols[3].contains("gitgud") {
                    return None;
                }
                Some(ReflogEntry {
                    id: cols[0].to_owned(),
                    short_id: cols[1].to_owned(),
                    selector: cols[2].to_owned(),
                    action: cols[3].to_owned(),
                    time: cols[4].trim().parse().unwrap_or(0),
                })
            })
            .collect())
    }
}

/// Splits `--amend` / commit messages cleanly into a label.
fn label_with_subject(verb: &str, message: &str) -> String {
    let subject = message.lines().next().unwrap_or("").trim();
    let short: String = subject.chars().take(60).collect();
    let cut = if short.len() < subject.len() {
        "…"
    } else {
        ""
    };
    format!("{verb} \"{short}{cut}\"")
}

/// The blob `path` has in snapshot `tree`, `None` when the file wasn't there.
/// The blob id each of `paths` would get in a snapshot (through git's clean
/// filters, as `add` does), `None` where the file is gone: one `hash-object`
/// for the files an undo checks, not a snapshot of the whole work tree.
fn worktree_blobs(
    repo: &Repo,
    git: &GitCli,
    paths: &[String],
) -> Result<Vec<Option<String>>, Error> {
    let present: Vec<&str> = paths
        .iter()
        .filter(|p| repo.root().join(p).is_file())
        .map(String::as_str)
        .collect();
    let mut ids = if present.is_empty() {
        Vec::new()
    } else {
        let mut list = present.join("\n");
        list.push('\n');
        git.run(
            repo.root(),
            &["hash-object", "--stdin-paths"],
            Some(list.as_bytes()),
        )?
        .lines()
        .map(str::to_owned)
        .collect()
    }
    .into_iter();
    Ok(paths
        .iter()
        .map(|p| {
            if present.contains(&p.as_str()) {
                ids.next()
            } else {
                None
            }
        })
        .collect())
}

fn blob_in(repo: &Repo, tree: &str, path: &str) -> Result<Option<String>, Error> {
    let gix = repo.gix();
    let id = gix::ObjectId::from_hex(tree.as_bytes()).map_err(read_err)?;
    let tree = gix.find_tree(id).map_err(read_err)?;
    Ok(tree
        .lookup_entry_by_path(path)
        .map_err(read_err)?
        .map(|e| e.object_id().to_string()))
}

/// git's own `.git/index.lock`, held while an undo or redo moves anything, so a
/// git running at the same time fails instead of racing us. Dropped without
/// [`IndexLock::commit`], it leaves the index as it was.
struct IndexLock {
    lock: PathBuf,
    index: PathBuf,
    file: Option<fs::File>,
}

impl IndexLock {
    fn take(git_dir: &Path) -> Result<Self, Error> {
        let lock = git_dir.join("index.lock");
        let file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Error::Refused(
                    "another git process is using the index (.git/index.lock exists)".into(),
                ));
            }
            Err(e) => return Err(read_err(e)),
        };
        Ok(Self {
            lock,
            index: git_dir.join("index"),
            file: Some(file),
        })
    }

    /// Replaces the index with `bytes`, or deletes it for `None`.
    fn commit(mut self, bytes: Option<&[u8]>) -> Result<(), Error> {
        let Some(mut file) = self.file.take() else {
            return Err(Error::Read("index lock committed twice".into()));
        };
        let written = match bytes {
            Some(bytes) => file.write_all(bytes).and_then(|()| file.sync_all()),
            None => Ok(()),
        };
        drop(file);
        written.map_err(read_err)?;
        match bytes {
            Some(_) => fs::rename(&self.lock, &self.index).map_err(read_err),
            None => match fs::remove_file(&self.index) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
                _ => Ok(()),
            },
        }
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        self.file = None;
        match fs::remove_file(&self.lock) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("gitgud: cannot remove {}: {e}", self.lock.display());
            }
            _ => {}
        }
    }
}

/// Moves HEAD's branch, the rebased branch, every branch the entry moved and
/// the stash to where `to` has them, in one `update-ref` transaction: either
/// all of them move or none does.
fn move_refs(
    repo: &Repo,
    git: &GitCli,
    to: &State,
    moved: &[String],
    stash_moved: bool,
    message: &str,
) -> Result<(), Error> {
    let mut tx = RefTransaction::default();
    let here = head_state(repo)?;
    let mut switch_to = None;
    let mut detach_at = None;
    match &to.head {
        HeadState::Branch { name, tip } => {
            let now = match &here {
                HeadState::Branch { name: n, tip } if n == name => tip.clone(),
                _ => branch_tip(repo, name)?,
            };
            tx.set(name, tip.as_deref(), now.as_deref(), false);
            if !matches!(&here, HeadState::Branch { name: n, .. } if n == name) {
                switch_to = Some(name);
            }
        }
        HeadState::Detached { id } => match &here {
            HeadState::Detached { id: now } => tx.set("HEAD", Some(id), Some(now), true),
            // git refuses to detach HEAD in the transaction that moves its branch.
            HeadState::Branch { .. } => detach_at = Some(id),
        },
    }
    if let Some((name, Some(tip))) = &to.rebasing {
        let now = branch_tip(repo, name)?;
        tx.set(name, Some(tip), now.as_deref(), false);
    }
    for name in moved {
        let now = branch_tip(repo, name)?;
        tx.set(name, tip_in(to, name), now.as_deref(), false);
    }
    if stash_moved {
        let now = branch_tip(repo, "refs/stash")?;
        tx.set("refs/stash", to.stash_ref.as_deref(), now.as_deref(), true);
    }
    if !tx.text.is_empty() {
        git.run(
            repo.root(),
            &["update-ref", "-m", message, "--stdin"],
            Some(tx.text.as_bytes()),
        )?;
    }
    if let Some(name) = switch_to {
        git.run(
            repo.root(),
            &["symbolic-ref", "-m", message, "HEAD", name],
            None,
        )?;
    }
    if let Some(id) = detach_at {
        git.run(
            repo.root(),
            &["update-ref", "--no-deref", "-m", message, "HEAD", id],
            None,
        )?;
    }
    Ok(())
}

/// `git update-ref --stdin` commands, each ref at most once (the first wins).
#[derive(Default)]
struct RefTransaction {
    text: String,
    queued: HashSet<String>,
}

impl RefTransaction {
    /// Moves `name` from `now` to `target` (`None`: absent), checking `now`.
    fn set(&mut self, name: &str, target: Option<&str>, now: Option<&str>, no_deref: bool) {
        if target == now || !self.queued.insert(name.to_owned()) {
            return;
        }
        if no_deref {
            self.text.push_str("option no-deref\n");
        }
        let line = match (target, now) {
            (Some(t), Some(n)) => format!("update {name} {t} {n}\n"),
            (Some(t), None) => format!("create {name} {t}\n"),
            (None, Some(n)) => format!("delete {name} {n}\n"),
            (None, None) => return,
        };
        self.text.push_str(&line);
    }
}

/// Puts `.git/logs/refs/stash` back, so the stash list matches `to`.
fn restore_stash_log(repo: &Repo, to: &State) -> Result<(), Error> {
    let log_path = repo.gix().common_dir().join("logs/refs/stash");
    match &to.stash_log {
        Some(log) => {
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent).map_err(read_err)?;
            }
            fs::write(&log_path, log).map_err(read_err)
        }
        None => match fs::remove_file(&log_path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
            _ => Ok(()),
        },
    }
}

/// Puts `paths` in the work tree back as snapshot `tree` had them: rewritten
/// through git (so eol and filters apply), or deleted where the tree has none.
fn restore_files(repo: &Repo, git: &GitCli, tree: &str, paths: &[String]) -> Result<(), Error> {
    let mut present = Vec::new();
    for path in paths {
        if blob_in(repo, tree, path)?.is_some() {
            present.push(path.as_str());
        } else {
            match fs::remove_file(repo.root().join(path)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
                _ => {}
            }
        }
    }
    if present.is_empty() {
        return Ok(());
    }
    let source = format!("--source={tree}");
    git.run(
        repo.root(),
        &[
            "--literal-pathspecs",
            "restore",
            &source,
            "--worktree",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        Some(present.join("\0").as_bytes()),
    )
    .map(drop)
}

fn head_state(repo: &Repo) -> Result<HeadState, Error> {
    let gix = repo.gix();
    let head = gix.head().map_err(read_err)?;
    Ok(match head.kind {
        gix::head::Kind::Symbolic(r) => HeadState::Branch {
            name: r.name.as_bstr().to_string(),
            tip: r.target.try_id().map(|id| id.to_hex().to_string()),
        },
        gix::head::Kind::Unborn(name) => HeadState::Branch {
            name: name.as_bstr().to_string(),
            tip: None,
        },
        gix::head::Kind::Detached { target, .. } => HeadState::Detached {
            id: target.to_string(),
        },
    })
}

fn branch_tip(repo: &Repo, name: &str) -> Result<Option<String>, Error> {
    let gix = repo.gix();
    Ok(gix
        .try_find_reference(name)
        .map_err(read_err)?
        .and_then(|r| r.target().try_id().map(|id| id.to_hex().to_string())))
}

fn branch_tips(repo: &Repo) -> Result<Vec<(String, String)>, Error> {
    let gix = repo.gix();
    let platform = gix.references().map_err(read_err)?;
    let mut tips = Vec::new();
    for r in platform.local_branches().map_err(read_err)? {
        let r = r.map_err(|e| Error::Read(e.to_string()))?;
        if let Some(id) = r.target().try_id() {
            tips.push((r.name().as_bstr().to_string(), id.to_hex().to_string()));
        }
    }
    tips.sort();
    Ok(tips)
}

/// What an index stages, ignoring the stat data `git status` rewrites.
fn index_entries(path: &Path) -> Result<Vec<(String, String, u32, u32)>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = gix::index::File::at(path, gix::hash::Kind::Sha1, false, Default::default())
        .map_err(read_err)?;
    Ok(file
        .entries()
        .iter()
        .map(|e| {
            (
                e.path(&file).to_string(),
                e.id.to_string(),
                e.mode.bits(),
                e.stage_raw(),
            )
        })
        .collect())
}

fn kept_snapshot_trees(entries: &[Entry]) -> Vec<String> {
    let mut kept = Vec::new();
    for entry in entries {
        if let Some(tree) = &entry.before.worktree {
            kept.push(tree.clone());
        }
        if let Some(tree) = &entry.after.worktree {
            kept.push(tree.clone());
        }
    }
    kept
}

const MAX_LFS_POINTER_BYTES: u64 = 512;

fn oplog_only_lfs_bytes(repo: &Repo, entries: &[Entry], prune_end: usize) -> (u64, u64) {
    let gix = &repo.gix();
    let lfs_objects_dir = gix.common_dir().join("lfs").join("objects");
    if !lfs_objects_dir.is_dir() {
        return (0, 0);
    }

    let mut blob_cache: HashMap<gix::ObjectId, Option<String>> = HashMap::new();
    let mut prunable_trees_seen = HashSet::new();
    let mut kept_trees_seen = HashSet::new();
    let mut prunable_oids = HashSet::new();
    let mut kept_oids = HashSet::new();

    for e in &entries[..prune_end] {
        collect_state_lfs_oids(
            gix,
            &e.before,
            &mut prunable_trees_seen,
            &mut blob_cache,
            &mut prunable_oids,
        );
        collect_state_lfs_oids(
            gix,
            &e.after,
            &mut prunable_trees_seen,
            &mut blob_cache,
            &mut prunable_oids,
        );
    }
    for e in &entries[prune_end..] {
        collect_state_lfs_oids(
            gix,
            &e.before,
            &mut kept_trees_seen,
            &mut blob_cache,
            &mut kept_oids,
        );
        collect_state_lfs_oids(
            gix,
            &e.after,
            &mut kept_trees_seen,
            &mut blob_cache,
            &mut kept_oids,
        );
    }

    let mut snapshot_sizes: HashMap<String, u64> = HashMap::new();
    for oid in prunable_oids.iter().chain(kept_oids.iter()) {
        if snapshot_sizes.contains_key(oid) || oid.len() < 4 {
            continue;
        }
        let obj_path = lfs_objects_dir.join(&oid[..2]).join(&oid[2..4]).join(oid);
        if let Ok(meta) = fs::metadata(obj_path)
            && meta.is_file()
        {
            snapshot_sizes.insert(oid.clone(), meta.len());
        }
    }
    if snapshot_sizes.is_empty() {
        return (0, 0);
    }

    // Walking the history is the slow part, and its answer only changes when a
    // ref, the index or the op log's objects do.
    let key = reachability_key(repo, &snapshot_sizes);
    let remaining = {
        let mut kept = repo
            .oplog_only_lfs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match kept.as_ref() {
            Some((k, set)) if *k == key => set.clone(),
            _ => {
                let wanted = snapshot_sizes.keys().cloned().collect();
                let set = unreachable_lfs(gix, wanted, &mut blob_cache);
                *kept = Some((key, set.clone()));
                set
            }
        }
    };

    let total_lfs: u64 = remaining
        .iter()
        .filter_map(|oid| snapshot_sizes.get(oid))
        .copied()
        .sum();
    let reclaimable_lfs: u64 = remaining
        .iter()
        .filter(|oid| prunable_oids.contains(*oid) && !kept_oids.contains(*oid))
        .filter_map(|oid| snapshot_sizes.get(oid))
        .copied()
        .sum();
    (total_lfs, reclaimable_lfs)
}

/// [`Repo::refs_key`], the index file's size and time, and `objects`.
fn reachability_key(repo: &Repo, objects: &HashMap<String, u64>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo.refs_key().hash(&mut h);
    if let Ok(meta) = fs::metadata(repo.gix().index_path()) {
        meta.len().hash(&mut h);
        meta.modified().ok().hash(&mut h);
    }
    let mut oids: Vec<&String> = objects.keys().collect();
    oids.sort();
    oids.hash(&mut h);
    h.finish()
}

/// Which of `wanted` (LFS oids) neither the index nor any ref's history reaches.
fn unreachable_lfs(
    gix: &gix::Repository,
    mut remaining: HashSet<String>,
    blob_cache: &mut HashMap<gix::ObjectId, Option<String>>,
) -> HashSet<String> {
    if let Ok(index) = gix.index_or_empty() {
        for entry in index.entries() {
            if let Some(oid) = blob_lfs_oid(gix, entry.id, blob_cache) {
                remaining.remove(&oid);
                if remaining.is_empty() {
                    return remaining;
                }
            }
        }
    }

    let mut commit_stack = Vec::new();
    let mut live_tree_stack = Vec::new();
    if let Ok(head_id) = gix.head_id() {
        commit_stack.push(head_id.detach());
    }
    if let Ok(refs) = gix.references()
        && let Ok(iter) = refs.all()
    {
        for mut r in iter.flatten() {
            if r.name().as_bstr().starts_with(b"refs/gitgud/") {
                continue;
            }
            let Ok(id) = r.peel_to_id() else {
                continue;
            };
            let oid = id.detach();
            let Ok(header) = gix.find_header(oid) else {
                continue;
            };
            match header.kind() {
                gix::object::Kind::Commit => commit_stack.push(oid),
                gix::object::Kind::Tree => live_tree_stack.push(oid),
                gix::object::Kind::Blob => {
                    if let Some(lfs_oid) = blob_lfs_oid(gix, oid, blob_cache) {
                        remaining.remove(&lfs_oid);
                    }
                }
                gix::object::Kind::Tag => {}
            }
        }
    }

    let mut visited_commits = HashSet::new();
    let mut live_trees_seen = HashSet::new();
    if !remaining.is_empty() && !live_tree_stack.is_empty() {
        walk_trees_for_lfs(
            gix,
            &mut live_tree_stack,
            &mut live_trees_seen,
            blob_cache,
            |lfs_oid| {
                remaining.remove(lfs_oid);
                !remaining.is_empty()
            },
        );
    }
    while let Some(commit_id) = commit_stack.pop() {
        if remaining.is_empty() {
            break;
        }
        if !visited_commits.insert(commit_id) {
            continue;
        }
        let Ok(commit) = gix.find_commit(commit_id) else {
            continue;
        };
        if let Ok(tree_id) = commit.tree_id() {
            live_tree_stack.push(tree_id.detach());
        }
        for parent_id in commit.parent_ids() {
            commit_stack.push(parent_id.detach());
        }
        walk_trees_for_lfs(
            gix,
            &mut live_tree_stack,
            &mut live_trees_seen,
            blob_cache,
            |lfs_oid| {
                remaining.remove(lfs_oid);
                !remaining.is_empty()
            },
        );
    }
    remaining
}

fn collect_state_lfs_oids(
    gix: &gix::Repository,
    state: &State,
    seen_trees: &mut HashSet<gix::ObjectId>,
    blob_cache: &mut HashMap<gix::ObjectId, Option<String>>,
    out: &mut HashSet<String>,
) {
    let Some(tree_hex) = &state.worktree else {
        return;
    };
    let Ok(tree_id) = gix::ObjectId::from_hex(tree_hex.as_bytes()) else {
        return;
    };
    let mut stack = vec![tree_id];
    walk_trees_for_lfs(gix, &mut stack, seen_trees, blob_cache, |oid| {
        out.insert(oid.to_owned());
        true
    });
}

fn walk_trees_for_lfs(
    gix: &gix::Repository,
    tree_stack: &mut Vec<gix::ObjectId>,
    seen_trees: &mut HashSet<gix::ObjectId>,
    blob_cache: &mut HashMap<gix::ObjectId, Option<String>>,
    mut on_lfs: impl FnMut(&str) -> bool,
) {
    while let Some(tree_id) = tree_stack.pop() {
        if !seen_trees.insert(tree_id) {
            continue;
        }
        let Ok(tree) = gix.find_tree(tree_id) else {
            continue;
        };
        for entry in tree.iter().flatten() {
            if entry.mode().is_tree() {
                tree_stack.push(entry.object_id());
            } else if entry.mode().is_blob()
                && let Some(oid) = blob_lfs_oid(gix, entry.object_id(), blob_cache)
                && !on_lfs(&oid)
            {
                return;
            }
        }
    }
}

fn blob_lfs_oid(
    gix: &gix::Repository,
    id: gix::ObjectId,
    blob_cache: &mut HashMap<gix::ObjectId, Option<String>>,
) -> Option<String> {
    if let Some(cached) = blob_cache.get(&id) {
        return cached.clone();
    }
    let header_ok = gix
        .find_header(id)
        .is_ok_and(|h| h.kind() == gix::object::Kind::Blob && h.size() <= MAX_LFS_POINTER_BYTES);
    let parsed = if header_ok {
        gix.find_blob(id)
            .ok()
            .and_then(|blob| crate::lfs::parse_lfs_pointer(&blob.data).map(|(oid, _)| oid))
    } else {
        None
    };
    blob_cache.insert(id, parsed.clone());
    parsed
}

fn stash_changed(entry: &Entry) -> bool {
    entry.before.stash_ref != entry.after.stash_ref
        || entry.before.stash_log != entry.after.stash_log
}

fn read_optional(path: &Path) -> Result<Option<String>, Error> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_err(e)),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, time: i64, dir: &Path) -> Entry {
        let name = format!("{id}-before.index");
        fs::write(dir.join(&name), b"x").unwrap();
        let state = |index| State {
            head: HeadState::Detached { id: "0".repeat(40) },
            index,
            worktree: None,
            merge: Vec::new(),
            rebasing: None,
            branches: Vec::new(),
            stash_ref: None,
            stash_log: None,
        };
        Entry {
            id,
            time,
            action: format!("action {id}"),
            undone: false,
            paths: Vec::new(),
            before: state(Some(name)),
            after: state(None),
        }
    }

    fn oplog() -> (tempfile::TempDir, OpLog) {
        let tmp = tempfile::tempdir().unwrap();
        let log = OpLog {
            dir: tmp.path().to_owned(),
            git_dir: tmp.path().to_owned(),
        };
        (tmp, log)
    }

    #[test]
    fn prunes_only_entries_that_are_both_old_and_beyond_the_count() {
        let (tmp, log) = oplog();
        let old = now() - KEEP_SECONDS - 60;
        let mut entries: Vec<Entry> = (0..KEEP_ENTRIES as u64 + 5)
            .map(|i| entry(i, old, tmp.path()))
            .collect();

        log.prune(&mut entries);

        assert_eq!(entries.len(), KEEP_ENTRIES);
        assert_eq!(entries[0].id, 5);
        assert!(
            !tmp.path().join("0-before.index").exists(),
            "pruned index copies are deleted"
        );
        assert!(tmp.path().join("5-before.index").exists());
    }

    #[test]
    fn labels_name_the_files_or_count_them() {
        assert_eq!(label_paths("Stage", &["a.txt".into()]), "Stage a.txt");
        assert_eq!(
            label_paths("Unstage", &["a".into(), "b".into()]),
            "Unstage 2 files"
        );
    }

    #[test]
    fn commit_labels_use_the_subject_cut_short() {
        assert_eq!(label_commit("Fix it\n\nbody"), "Commit \"Fix it\"");
        let long = "x".repeat(80);
        assert_eq!(
            label_commit(&long),
            format!("Commit \"{}…\"", "x".repeat(60))
        );
        assert_eq!(label_commit("ünïcødé"), "Commit \"ünïcødé\"");
    }

    #[test]
    fn keeps_recent_entries_past_the_count() {
        let (tmp, log) = oplog();
        let mut entries: Vec<Entry> = (0..KEEP_ENTRIES as u64 + 5)
            .map(|i| entry(i, now(), tmp.path()))
            .collect();

        log.prune(&mut entries);

        assert_eq!(entries.len(), KEEP_ENTRIES + 5);
    }
}
