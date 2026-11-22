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
}

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
        let mut entries = self.load()?;
        entries.retain(|e| !e.undone);
        let id = entries.last().map_or(1, |e| e.id + 1);
        let git = destructive.map(|(g, _)| g);
        let before = self.capture(repo, git, id, "before")?;
        let value = match op() {
            Ok(v) => v,
            Err(e) => {
                self.remove_files(&before);
                return Err(e);
            }
        };
        let after = self.capture(repo, git, id, "after")?;
        entries.push(Entry {
            id,
            time: now(),
            action: action.to_owned(),
            undone: false,
            paths: destructive.map(|(_, p)| p.to_vec()).unwrap_or_default(),
            before,
            after,
        });
        self.prune(&mut entries);
        self.save(&entries)?;
        Ok(value)
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
        let mut entries = self.load()?;
        let Some(pos) = entries.iter().rposition(|e| !e.undone) else {
            return Err(Error::Refused("nothing to undo".into()));
        };
        let entry = &entries[pos];
        self.ensure_at(repo, git, &entry.after, &entry.paths, "undo", &entry.action)?;
        self.restore(
            repo,
            git,
            &entry.before,
            &entry.paths,
            &format!("gitgud: undo {}", entry.action),
        )?;
        entries[pos].undone = true;
        let action = entries[pos].action.clone();
        self.save(&entries)?;
        Ok(action)
    }

    /// Re-applies the oldest undone action. Returns its label.
    ///
    /// # Errors
    /// As [`OpLog::undo`].
    pub fn redo(&self, repo: &Repo, git: &GitCli) -> Result<String, Error> {
        let mut entries = self.load()?;
        let Some(pos) = entries.iter().position(|e| e.undone) else {
            return Err(Error::Refused("nothing to redo".into()));
        };
        let entry = &entries[pos];
        self.ensure_at(
            repo,
            git,
            &entry.before,
            &entry.paths,
            "redo",
            &entry.action,
        )?;
        self.restore(
            repo,
            git,
            &entry.after,
            &entry.paths,
            &format!("gitgud: redo {}", entry.action),
        )?;
        entries[pos].undone = false;
        let action = entries[pos].action.clone();
        self.save(&entries)?;
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
        Ok(State {
            head: head_state(repo)?,
            index: copy,
            worktree,
        })
    }

    /// Refuses when HEAD, the staged content, or (for destructive actions) the
    /// touched files differ from `expected`: something happened outside gitgud
    /// that the op log didn't see, and undoing over it would lose it.
    fn ensure_at(
        &self,
        repo: &Repo,
        git: &GitCli,
        expected: &State,
        paths: &[String],
        verb: &str,
        action: &str,
    ) -> Result<(), Error> {
        let refused = || {
            Error::Refused(format!(
                "can't {verb} \"{action}\": the repository changed outside gitgud since"
            ))
        };
        let here = head_state(repo)?;
        let same_index = index_entries(&self.git_dir.join("index"))?
            == match &expected.index {
                Some(name) => index_entries(&self.dir.join(name))?,
                None => Vec::new(),
            };
        if here != expected.head || !same_index {
            return Err(refused());
        }
        if let (Some(tree), false) = (&expected.worktree, paths.is_empty()) {
            let now = Snapshots::for_repo(repo).take(repo, git)?;
            for path in paths {
                if blob_in(repo, &now, path)? != blob_in(repo, tree, path)? {
                    return Err(refused());
                }
            }
        }
        Ok(())
    }

    fn restore(
        &self,
        repo: &Repo,
        git: &GitCli,
        to: &State,
        paths: &[String],
        message: &str,
    ) -> Result<(), Error> {
        self.restore_refs_and_index(repo, git, to, message)?;
        if let (Some(tree), false) = (&to.worktree, paths.is_empty()) {
            restore_files(repo, git, tree, paths)?;
        }
        Ok(())
    }

    fn restore_refs_and_index(
        &self,
        repo: &Repo,
        git: &GitCli,
        to: &State,
        message: &str,
    ) -> Result<(), Error> {
        let root = repo.root();
        let here = head_state(repo)?;
        match &to.head {
            HeadState::Branch { name, tip } => {
                let current_tip = match &here {
                    HeadState::Branch { name: n, tip } if n == name => tip.clone(),
                    _ => branch_tip(repo, name)?,
                };
                match (tip, &current_tip) {
                    (Some(t), Some(c)) if t != c => {
                        git.run(root, &["update-ref", "-m", message, name, t, c], None)?;
                    }
                    (Some(t), None) => {
                        git.run(root, &["update-ref", "-m", message, name, t, ""], None)?;
                    }
                    (None, Some(c)) => {
                        git.run(root, &["update-ref", "-d", name, c], None)?;
                    }
                    _ => {}
                }
                if !matches!(&here, HeadState::Branch { name: n, .. } if n == name) {
                    git.run(root, &["symbolic-ref", "-m", message, "HEAD", name], None)?;
                }
            }
            HeadState::Detached { id } => {
                if here != to.head {
                    git.run(
                        root,
                        &["update-ref", "--no-deref", "-m", message, "HEAD", id],
                        None,
                    )?;
                }
            }
        }
        self.restore_index(to)
    }

    /// Writes the saved index back under git's own `index.lock`, so a git running
    /// at the same time fails instead of racing us.
    fn restore_index(&self, to: &State) -> Result<(), Error> {
        let index = self.git_dir.join("index");
        let lock = self.git_dir.join("index.lock");
        let mut file = match fs::OpenOptions::new()
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
        let written = match &to.index {
            Some(name) => fs::read(self.dir.join(name))
                .and_then(|bytes| file.write_all(&bytes))
                .and_then(|()| file.sync_all()),
            None => Ok(()),
        };
        drop(file);
        let done = written.and_then(|()| match &to.index {
            Some(_) => fs::rename(&lock, &index),
            None => fs::remove_file(&lock).and_then(|()| match fs::remove_file(&index) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }),
        });
        if done.is_err() {
            let _ = fs::remove_file(&lock);
        }
        done.map_err(read_err)
    }

    fn prune(&self, entries: &mut Vec<Entry>) {
        let cutoff = now() - KEEP_SECONDS;
        while entries.len() > KEEP_ENTRIES && entries[0].time < cutoff {
            let old = entries.remove(0);
            self.remove_files(&old.before);
            self.remove_files(&old.after);
        }
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
    let subject = message.lines().next().unwrap_or("").trim();
    let short: String = subject.chars().take(60).collect();
    let cut = if short.len() < subject.len() {
        "…"
    } else {
        ""
    };
    format!("Commit \"{short}{cut}\"")
}

/// The blob `path` has in snapshot `tree`, `None` when the file wasn't there.
fn blob_in(repo: &Repo, tree: &str, path: &str) -> Result<Option<String>, Error> {
    let gix = repo.gix();
    let id = gix::ObjectId::from_hex(tree.as_bytes()).map_err(read_err)?;
    let tree = gix.find_tree(id).map_err(read_err)?;
    Ok(tree
        .lookup_entry_by_path(path)
        .map_err(read_err)?
        .map(|e| e.object_id().to_string()))
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
