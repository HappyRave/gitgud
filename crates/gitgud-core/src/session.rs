use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde::Serialize;

use crate::cli::GitCli;
use crate::config;
use crate::git;
use crate::graph::{Graph, Row};
use crate::history::{Action, HistoryPlan, PlanPrediction, Step};
use crate::merge::{MergeOutcome, MergeState, Side};
use crate::oplog::{OpEntry, OpLog, label_amend, label_commit};
use crate::ops::{CommitOptions, ResetMode};
use crate::rebase::{RebaseOutcome, RebaseState, RebaseStep};
use crate::remote::{PullOutcome, PullStrategy};
use crate::repo::{self, Error, Repo};
use crate::sequencer::{SequencerKind, SequencerState, SequencerStep};
use crate::task::{Cancel, Progress};
use crate::watch::RepoWatcher;
use crate::worktree::LineSelect;

/// The repositories the app has open, by id, and the git CLI it writes with.
#[derive(Default)]
pub struct Session {
    next_id: AtomicU32,
    open: Mutex<HashMap<u32, OpenRepo>>,
    watchers: Mutex<HashMap<u32, RepoWatcher>>,
    /// The running network task of each repo, so it can be cancelled.
    tasks: Mutex<HashMap<u32, Cancel>>,
    /// The graph of each repo with its history plan drawn in.
    previews: Mutex<HashMap<u32, Arc<Graph>>>,
    git: OnceLock<GitCli>,
}

/// A history plan's prediction and the size of its graph preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPreview {
    pub prediction: PlanPrediction,
    /// Rows of the preview graph.
    pub rows: usize,
}

/// A repository with its graph. Cheap to clone; a refresh swaps in a new graph
/// while readers keep the old one.
#[derive(Clone)]
pub struct OpenRepo {
    pub repo: Arc<Repo>,
    pub graph: Arc<Graph>,
    pub graph_version: u32,
    config_stamp: Option<(std::time::SystemTime, u64)>,
    /// One write at a time per repo: the op log records before and after states.
    writing: Arc<Mutex<()>>,
}

fn config_stamp(repo: &Repo) -> Option<(std::time::SystemTime, u64)> {
    let meta = std::fs::metadata(repo.gix().common_dir().join("config")).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// The outcome of an undo or redo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reverted {
    pub opened: Opened,
    /// The action that was undone or redone.
    pub action: String,
}

/// The outcome of a pull operation alongside the refreshed repo state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    pub opened: Opened,
    pub outcome: PullOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub id: u32,
    pub root: PathBuf,
    /// Graph rows, the working-copy row included.
    pub rows: usize,
    /// Bumped each time the graph is rebuilt, so views know to refetch rows.
    pub graph_version: u32,
}

impl Session {
    /// Opens the repository containing `path`, walks its history and lays out its
    /// graph. A working-copy row is added when there are uncommitted changes.
    ///
    /// # Errors
    /// See [`repo::open`], [`Repo::status`] and [`Graph::build`].
    pub fn open(&self, path: impl AsRef<Path>) -> Result<Opened, Error> {
        let repo = repo::open(path)?;
        let dirty = !repo.status()?.entries.is_empty();
        let graph = Graph::build(&repo, dirty)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let open = OpenRepo {
            config_stamp: config_stamp(&repo),
            repo: Arc::new(repo),
            graph: Arc::new(graph),
            graph_version: 0,
            writing: Arc::default(),
        };
        let opened = opened(id, &open);
        self.lock().insert(id, open);
        Ok(opened)
    }

    /// Re-reads a repository after something changed it. The graph is rebuilt only
    /// when a ref or HEAD moved, or the working copy turned clean or dirty.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or a read error.
    pub fn refresh(&self, id: u32) -> Result<Opened, Error> {
        self.refresh_inner(id, false)
    }

    fn refresh_inner(&self, id: u32, force_reload: bool) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let stamp = config_stamp(&open.repo);
        let reload = force_reload || stamp != open.config_stamp;
        let repo = if reload {
            Arc::new(repo::open(open.repo.root())?)
        } else {
            open.repo.clone()
        };
        let dirty = !repo.status()?.entries.is_empty();
        if open.graph.is_current(&repo, dirty)? {
            if reload {
                let updated = OpenRepo {
                    repo,
                    graph: open.graph.clone(),
                    graph_version: open.graph_version,
                    config_stamp: stamp,
                    writing: open.writing.clone(),
                };
                let res = opened(id, &updated);
                self.lock().insert(id, updated);
                return Ok(res);
            }
            return Ok(opened(id, &open));
        }
        let fresh = OpenRepo {
            config_stamp: config_stamp(&repo),
            repo: repo.clone(),
            graph: Arc::new(Graph::build(&repo, dirty)?),
            graph_version: open.graph_version + 1,
            writing: open.writing.clone(),
        };
        let opened = opened(id, &fresh);
        self.lock().insert(id, fresh);
        Ok(opened)
    }

    /// # Errors
    /// [`Error::UnknownRepo`] when `id` was never opened or is closed.
    pub fn get(&self, id: u32) -> Result<OpenRepo, Error> {
        self.lock().get(&id).cloned().ok_or(Error::UnknownRepo(id))
    }

    /// Closes the repository and stops watching it.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] when `id` was never opened or is already closed.
    pub fn close(&self, id: u32) -> Result<(), Error> {
        self.watchers().remove(&id);
        self.previews
            .lock()
            .expect("preview lock poisoned")
            .remove(&id);
        self.lock()
            .remove(&id)
            .map(drop)
            .ok_or(Error::UnknownRepo(id))
    }

    /// Calls `on_change` after outside changes to the work tree or refs, debounced.
    /// Changes made through [`Session::write`] are not reported.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or [`Error::Read`] when the watch can't be set up.
    pub fn watch(&self, id: u32, on_change: impl Fn() + Send + 'static) -> Result<(), Error> {
        let repo = self.get(id)?.repo;
        let watcher = RepoWatcher::start(repo, on_change)?;
        self.watchers().insert(id, watcher);
        Ok(())
    }

    /// Runs a git write on repository `id` with the watcher paused, records it in
    /// the op log as `action` (Ctrl+Z undoes it), then refreshes.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Git`] when git is unusable, or the write's error.
    pub fn write(
        &self,
        id: u32,
        action: &str,
        op: impl FnOnce(&Repo, &GitCli) -> Result<(), Error>,
    ) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo).record(&open.repo, action, || op(&open.repo, git))
        })?;
        self.refresh(id)
    }

    /// As [`Session::write`], for an action that can lose work-tree content in
    /// `paths`: the work tree is snapshotted first, so Ctrl+Z gets it back.
    ///
    /// # Errors
    /// As [`Session::write`], or a snapshot error (then `op` doesn't run).
    pub fn write_destructive(
        &self,
        id: u32,
        action: &str,
        paths: &[String],
        op: impl FnOnce(&Repo, &GitCli) -> Result<(), Error>,
    ) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo)
                .record_destructive(&open.repo, git, action, paths, || op(&open.repo, git))
        })?;
        self.refresh(id)
    }

    /// Checks out local branch `name`, snapshotting the files it rewrites so
    /// Ctrl+Z switches back with them as they were.
    ///
    /// # Errors
    /// [`Error::Refused`] for an unknown branch, or git's refusal (local
    /// changes that would be overwritten); then nothing changed.
    pub fn checkout(&self, id: u32, name: &str) -> Result<Opened, Error> {
        let paths = self.get(id)?.repo.checkout_paths(name)?;
        let action = format!("Check out {name}");
        self.write_destructive(id, &action, &paths, |repo, git| repo.checkout(git, name))
    }

    /// Merges `branch` into the current branch. Clean merges commit; conflicted
    /// ones stop (see [`Session::merge_state`]). Undoable either way.
    ///
    /// # Errors
    /// [`Error::Refused`] while a merge is in progress, or git's error.
    pub fn merge(&self, id: u32, branch: &str) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.merge_paths(self.git()?, branch)?;
        let action = format!("Merge {branch}");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.merge(git, branch).map(drop)
        })
    }

    /// The merge in progress in repo `id`, if any.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or a read error.
    pub fn merge_state(&self, id: u32) -> Result<Option<MergeState>, Error> {
        self.get(id)?.repo.merge_state(self.git()?)
    }

    /// Resolves conflicted `path` with `content`, recorded as `how` ("use both"...).
    ///
    /// # Errors
    /// A write error; then the file stays conflicted.
    pub fn resolve_with(
        &self,
        id: u32,
        path: &str,
        content: &str,
        how: &str,
    ) -> Result<Opened, Error> {
        let action = format!("Resolve {path} ({how})");
        let paths = [path.to_owned()];
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.resolve_with(git, path, content)
        })
    }

    /// Resolves conflicted `path` by keeping one side whole.
    ///
    /// # Errors
    /// A write error; then the file stays conflicted.
    pub fn resolve_side(&self, id: u32, path: &str, side: Side) -> Result<Opened, Error> {
        let which = match side {
            Side::Yours => "keep yours",
            Side::Theirs => "keep theirs",
        };
        let action = format!("Resolve {path} ({which})");
        let paths = [path.to_owned()];
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.resolve_side(git, path, side)
        })
    }

    /// Aborts the merge in progress; undoable like any discard.
    ///
    /// # Errors
    /// git's error.
    pub fn merge_abort(&self, id: u32) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.merge_paths(self.git()?, "MERGE_HEAD")?;
        self.write_destructive(id, "Abort merge", &paths, |repo, git| repo.merge_abort(git))
    }

    /// Commits the resolved merge.
    ///
    /// # Errors
    /// [`Error::Refused`] while files are conflicted, or git's error (a hook).
    pub fn merge_commit(&self, id: u32, message: &str) -> Result<Opened, Error> {
        self.write(id, "Commit merge", |repo, git| {
            repo.merge_commit(git, message)
        })
    }

    /// Rebases the current branch onto `onto`; stops on conflicts. Undoable.
    ///
    /// # Errors
    /// [`Error::Refused`] while another operation is in progress, or git's error.
    pub fn rebase(&self, id: u32, onto: &str) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.rebase_paths(self.git()?, onto)?;
        let action = format!("Rebase onto {onto}");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.rebase(git, onto).map(drop)
        })
    }

    /// The rebase in progress in repo `id`, if any.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or a read error.
    pub fn rebase_state(&self, id: u32) -> Result<Option<RebaseState>, Error> {
        self.get(id)?.repo.rebase_state(self.git()?)
    }

    /// Continues, skips the current commit of, or aborts the rebase in progress.
    ///
    /// # Errors
    /// [`Error::Refused`] while files are conflicted (continue), or git's error.
    pub fn rebase_step(&self, id: u32, step: RebaseStep) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.rebase_paths_in_progress(self.git()?)?;
        let action = match step {
            RebaseStep::Continue => "Continue rebase",
            RebaseStep::Skip => "Skip commit in rebase",
            RebaseStep::Abort => "Abort rebase",
        };
        self.write_destructive(id, action, &paths, |repo, git| match step {
            RebaseStep::Continue => repo.rebase_continue(git).map(drop),
            RebaseStep::Skip => repo.rebase_skip(git).map(drop),
            RebaseStep::Abort => repo.rebase_abort(git),
        })
    }

    /// The history plan of repo `id` from commit `from` up to HEAD.
    ///
    /// # Errors
    /// See [`Repo::history_plan`].
    pub fn history_plan(&self, id: u32, from: &str) -> Result<HistoryPlan, Error> {
        self.get(id)?.repo.history_plan(self.git()?, from)
    }

    /// Predicts `steps` replayed onto `onto` and lays out the graph with the
    /// result drawn in, for [`Session::preview_rows`]. Touches nothing.
    ///
    /// # Errors
    /// See [`Repo::predict_plan`].
    pub fn plan_preview(
        &self,
        id: u32,
        onto: &str,
        base: &str,
        steps: &[Step],
    ) -> Result<PlanPreview, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let prediction = open.repo.predict_plan(git, onto, base, steps)?;
        let ghosts = open.repo.plan_ghosts(steps, &prediction)?;
        let oid = |rev: &str| -> Result<gix::ObjectId, Error> {
            let hex = git.run(
                open.repo.root(),
                &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
                None,
            )?;
            gix::ObjectId::from_hex(hex.trim().as_bytes()).map_err(repo::read_err)
        };
        let replaced = steps
            .iter()
            .map(|s| oid(&s.id))
            .collect::<Result<Vec<_>, _>>()?;
        let graph = open
            .graph
            .preview(oid("HEAD")?, oid(onto)?, ghosts, &replaced)?;
        let rows = graph.len();
        self.previews
            .lock()
            .expect("preview lock poisoned")
            .insert(id, Arc::new(graph));
        Ok(PlanPreview { prediction, rows })
    }

    /// Rows of the last [`Session::plan_preview`] of repo `id`.
    ///
    /// # Errors
    /// [`Error::Refused`] when there is no preview, or a read error.
    pub fn preview_rows(&self, id: u32, start: usize, count: usize) -> Result<Vec<Row>, Error> {
        let open = self.get(id)?;
        let graph = self
            .previews
            .lock()
            .expect("preview lock poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Refused("no plan is being previewed".into()))?;
        graph.rows(&open.repo, start, count)
    }

    /// Runs a history plan: `git rebase -i` onto `onto`. Stops on a conflict or
    /// an Edit step. One op-log entry, so Ctrl+Z undoes the whole rebase.
    ///
    /// # Errors
    /// See [`Repo::run_plan`].
    pub fn run_plan(
        &self,
        id: u32,
        onto: &str,
        base: &str,
        steps: &[Step],
        update_refs: bool,
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let branch = open.repo.ensure_can_edit()?;
        let moved = open.repo.resolves_to(git, onto, base)?;
        let paths = open.repo.rebase_paths(git, onto)?;
        let action = Repo::plan_label(&branch, (!moved).then_some(onto), steps);
        self.previews
            .lock()
            .expect("preview lock poisoned")
            .remove(&id);
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.run_plan(git, onto, base, steps, update_refs).map(drop)
        })
    }

    /// Rewords the HEAD commit; undoable.
    ///
    /// # Errors
    /// See [`Repo::reword_head`].
    pub fn reword_head(&self, id: u32, message: &str) -> Result<Opened, Error> {
        let subject = message.lines().next().unwrap_or_default();
        let action = format!("Reword \"{subject}\"");
        self.write(id, &action, |repo, git| repo.reword_head(git, message))
    }

    /// Folds `commit` into its parent or drops it, at once; undoable.
    ///
    /// # Errors
    /// See [`Repo::quick_steps`] and [`Repo::run_plan`].
    pub fn quick_edit(&self, id: u32, commit: &str, action: Action) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let (base, steps) = open.repo.quick_steps(git, commit, action)?;
        let subject = open.repo.subject(git, commit)?;
        let label = match action {
            Action::Fixup => format!("Fixup \"{subject}\" into its parent"),
            _ => format!("Drop \"{subject}\""),
        };
        let paths = open.repo.rebase_paths(git, &base)?;
        self.write_destructive(id, &label, &paths, |repo, git| {
            repo.run_plan(git, &base, &base, &steps, false).map(drop)
        })
    }

    /// The op log of repo `id`, newest first.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or [`Error::Read`] when the log can't be read.
    pub fn op_log(&self, id: u32) -> Result<Vec<OpEntry>, Error> {
        let open = self.get(id)?;
        let entries = OpLog::for_repo(&open.repo).entries()?;
        Ok(entries.iter().rev().map(OpEntry::from).collect())
    }

    /// Returns the repo to how it was just before entry `entry_id`, by undoing
    /// it and everything after it, one by one (each step checked as Ctrl+Z is).
    /// Redo still walks forward again.
    ///
    /// # Errors
    /// [`Error::Refused`] when the entry is unknown or already undone, or a step
    /// is refused (the steps before it stay undone).
    pub fn restore_to(&self, id: u32, entry_id: u64) -> Result<Reverted, Error> {
        let action = self.exclusive(id, |open, git| {
            let log = OpLog::for_repo(&open.repo);
            let target = log
                .entries()?
                .into_iter()
                .find(|e| e.id == entry_id)
                .ok_or_else(|| Error::Refused(format!("op-log entry {entry_id} is gone")))?;
            if target.undone {
                return Err(Error::Refused(format!(
                    "\"{}\" is already undone",
                    target.action
                )));
            }
            while log.entries()?.iter().any(|e| e.id == entry_id && !e.undone) {
                log.undo(&open.repo, git)?;
            }
            Ok(target.action)
        })?;
        Ok(Reverted {
            opened: self.refresh(id)?,
            action,
        })
    }

    /// Puts back the files a destructive entry (a discard) changed, as a new
    /// undoable action; later actions stay.
    ///
    /// # Errors
    /// [`Error::Refused`] when the entry changed no files, or a write error.
    pub fn restore_files(&self, id: u32, entry_id: u64) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let entry = OpLog::for_repo(&open.repo)
            .entries()?
            .into_iter()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| Error::Refused(format!("op-log entry {entry_id} is gone")))?;
        let action = format!("Restore files from \"{}\"", entry.action);
        self.write_destructive(id, &action, &entry.paths, |repo, git| {
            OpLog::for_repo(repo).restore_entry_files(repo, git, entry_id)
        })
    }

    /// Undoes the newest recorded action.
    ///
    /// # Errors
    /// See [`OpLog::undo`].
    pub fn undo(&self, id: u32) -> Result<Reverted, Error> {
        let action = self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo).undo(&open.repo, git)
        })?;
        Ok(Reverted {
            opened: self.refresh(id)?,
            action,
        })
    }

    /// Redoes the oldest undone action.
    ///
    /// # Errors
    /// See [`OpLog::redo`].
    pub fn redo(&self, id: u32) -> Result<Reverted, Error> {
        let action = self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo).redo(&open.repo, git)
        })?;
        Ok(Reverted {
            opened: self.refresh(id)?,
            action,
        })
    }

    /// Creates branch `name` at `at` (or HEAD when `None`), optionally checking
    /// it out. Undoable with Ctrl+Z.
    ///
    /// # Errors
    /// [`Error::Command`] or [`Error::Refused`] when the branch can't be created.
    pub fn create_branch(
        &self,
        id: u32,
        name: &str,
        at: Option<&str>,
        checkout: bool,
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let trimmed = name.trim();
        let action = format!("Create branch {trimmed}");
        let paths = if checkout {
            match at {
                Some(target) => open.repo.checkout_rev_paths(git, target)?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        if paths.is_empty() {
            self.write(id, &action, |repo, git| {
                repo.create_branch(git, trimmed, at, checkout)
            })
        } else {
            self.write_destructive(id, &action, &paths, |repo, git| {
                repo.create_branch(git, trimmed, at, checkout)
            })
        }
    }

    /// Renames local branch `old` to `new`; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::rename_branch`].
    pub fn rename_branch(&self, id: u32, old: &str, new: &str) -> Result<Opened, Error> {
        let trimmed = new.trim();
        let action = format!("Rename {old} to {trimmed}");
        self.write(id, &action, |repo, git| {
            repo.rename_branch(git, old, trimmed)
        })
    }

    /// Deletes local branch `name`; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::delete_branch`].
    pub fn delete_branch(&self, id: u32, name: &str, force: bool) -> Result<Opened, Error> {
        let action = format!("Delete branch {name}");
        self.write(id, &action, |repo, git| {
            repo.delete_branch(git, name, force)
        })
    }

    /// Sets or clears the upstream tracking branch of `branch`.
    ///
    /// # Errors
    /// See [`Repo::set_upstream`].
    pub fn set_upstream(
        &self,
        id: u32,
        branch: &str,
        upstream: Option<&str>,
    ) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| {
            open.repo.set_upstream(git, branch, upstream)
        })?;
        self.refresh_inner(id, true)
    }

    /// Creates and checks out local branch `local_name` tracking `remote_branch`;
    /// undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::checkout_remote`].
    pub fn checkout_remote(
        &self,
        id: u32,
        remote_branch: &str,
        local_name: &str,
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let trimmed = local_name.trim();
        let paths = open.repo.checkout_rev_paths(git, remote_branch)?;
        let action = format!("Check out {trimmed} ({remote_branch})");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.checkout_remote(git, remote_branch, trimmed)
        })
    }

    /// Commits or amends with `message` and `options`; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::commit_with`].
    pub fn commit_with(
        &self,
        id: u32,
        message: &str,
        options: CommitOptions,
    ) -> Result<Opened, Error> {
        let action = if options.amend {
            label_amend(message)
        } else {
            label_commit(message)
        };
        self.write(id, &action, |repo, git| {
            repo.commit_with(git, message, options)
        })
    }

    /// Resets the current branch to `target` (`soft`, `mixed`, or `hard`).
    /// Hard reset snapshots dirty/changed work-tree files so Ctrl+Z undoes it.
    ///
    /// # Errors
    /// See [`Repo::reset`].
    pub fn reset(&self, id: u32, target: &str, mode: ResetMode) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let short = &target[..target.len().min(7)];
        let kind = match mode {
            ResetMode::Soft => "soft",
            ResetMode::Mixed => "mixed",
            ResetMode::Hard => "hard",
        };
        let action = format!("Reset ({kind}) to {short}");
        if mode == ResetMode::Hard {
            let paths = open.repo.reset_hard_paths(git, target)?;
            self.write_destructive(id, &action, &paths, |repo, git| {
                repo.reset(git, target, mode)
            })
        } else {
            self.write(id, &action, |repo, git| repo.reset(git, target, mode))
        }
    }

    /// Cherry-picks `commit` onto HEAD; stops on conflicts. Undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::cherry_pick`].
    pub fn cherry_pick(&self, id: u32, commit: &str) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let paths = open.repo.sequencer_paths(git, commit)?;
        let short = &commit[..commit.len().min(7)];
        let action = format!("Cherry-pick {short}");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.cherry_pick(git, commit).map(drop)
        })
    }

    /// Reverts `commit` onto HEAD; stops on conflicts. Undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::revert_commit`].
    pub fn revert_commit(&self, id: u32, commit: &str) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let paths = open.repo.sequencer_paths(git, commit)?;
        let short = &commit[..commit.len().min(7)];
        let action = format!("Revert {short}");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.revert_commit(git, commit).map(drop)
        })
    }

    /// The cherry-pick or revert stopped on conflicts in repo `id`, if any.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or a read error.
    pub fn sequencer_state(&self, id: u32) -> Result<Option<SequencerState>, Error> {
        self.get(id)?.repo.sequencer_state(self.git()?)
    }

    /// Continues or aborts the cherry-pick or revert in progress. Undoable.
    ///
    /// # Errors
    /// See [`Repo::sequencer_step`].
    pub fn sequencer_step(&self, id: u32, step: SequencerStep) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let (kind, oid) = open
            .repo
            .sequencer_head()?
            .ok_or_else(|| Error::Refused("no cherry-pick or revert is in progress".into()))?;
        let paths = open.repo.sequencer_paths(git, &oid)?;
        let action = match (kind, step) {
            (SequencerKind::CherryPick, SequencerStep::Continue) => "Continue cherry-pick",
            (SequencerKind::CherryPick, SequencerStep::Abort) => "Abort cherry-pick",
            (SequencerKind::Revert, SequencerStep::Continue) => "Continue revert",
            (SequencerKind::Revert, SequencerStep::Abort) => "Abort revert",
        };
        self.write_destructive(id, action, &paths, |repo, git| {
            repo.sequencer_step(git, step)
        })
    }

    /// Saves working-copy changes into a new stash; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::stash_save`].
    pub fn stash_save(
        &self,
        id: u32,
        message: Option<&str>,
        include_untracked: bool,
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.stash_save_paths(include_untracked)?;
        let action = match message.map(str::trim).filter(|m| !m.is_empty()) {
            Some(m) => format!("Stash \"{m}\""),
            None => "Stash changes".into(),
        };
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.stash_save(git, message, include_untracked)
        })
    }

    /// Applies `stash@{index}` to the working copy; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::stash_apply`].
    pub fn stash_apply(&self, id: u32, index: usize) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.stash_paths(index)?;
        let action = format!("Apply stash@{{{index}}}");
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.stash_apply(git, index)
        })
    }

    /// Applies `stash@{index}` and removes it on success; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::stash_pop`].
    pub fn stash_pop(&self, id: u32, index: usize) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.stash_paths(index)?;
        let action = format!("Pop stash@{{{index}}}");
        self.write_destructive(id, &action, &paths, |repo, git| repo.stash_pop(git, index))
    }

    /// Drops `stash@{index}`; undoable with Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::stash_drop`].
    pub fn stash_drop(&self, id: u32, index: usize) -> Result<Opened, Error> {
        let action = format!("Drop stash@{{{index}}}");
        self.write(id, &action, |repo, git| repo.stash_drop(git, index))
    }

    /// Fetches every remote of repo `id`, reporting progress, then refreshes.
    /// [`Session::cancel`] stops it. Not recorded in the op log: it only moves
    /// remote-tracking refs.
    ///
    /// # Errors
    /// [`Error::Refused`] when a remote task is already running,
    /// [`Error::Cancelled`], or git's error.
    pub fn fetch(&self, id: u32, mut on_progress: impl FnMut(Progress)) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        self.run_task(id, |cancel| open.repo.fetch(git, &mut on_progress, cancel))?;
        self.refresh(id)
    }

    /// Fetches the current branch's upstream remote and integrates changes using
    /// `strategy`. Honouring `pull.rebase` / `pull.ff` (defaulting to fast-forward
    /// only) when `strategy` is [`PullStrategy::Auto`]. Undoable locally.
    ///
    /// # Errors
    /// [`Error::Refused`], [`Error::Cancelled`], or git's error.
    pub fn pull(
        &self,
        id: u32,
        strategy: PullStrategy,
        remember: bool,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<PullResult, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        if open.repo.merge_state(git)?.is_some()
            || open.repo.rebase_state(git)?.is_some()
            || open.repo.sequencer_head()?.is_some()
        {
            return Err(Error::Refused(
                "finish or abort the operation in progress first".into(),
            ));
        }
        let target = open.repo.pull_target()?;
        self.run_task(id, |cancel| {
            open.repo
                .fetch_remote(git, target.remote.as_deref(), &mut on_progress, cancel)
        })?;
        if remember && matches!(strategy, PullStrategy::Merge | PullStrategy::Rebase) {
            open.repo
                .set_pull_rebase(git, strategy == PullStrategy::Rebase)?;
        }
        let effective = match strategy {
            PullStrategy::Auto => open.repo.configured_pull_strategy(git)?,
            other => other,
        };
        let (ahead, behind) = match open.repo.upstream_counts(&target.upstream_full) {
            Ok(counts) => counts,
            Err(e) => {
                let _ = self.refresh(id);
                return Err(e);
            }
        };
        if behind == 0 {
            return Ok(PullResult {
                opened: self.refresh(id)?,
                outcome: PullOutcome::UpToDate,
            });
        }
        if effective == PullStrategy::FfOnly && ahead > 0 {
            return Ok(PullResult {
                opened: self.refresh(id)?,
                outcome: PullOutcome::Diverged {
                    ahead,
                    behind,
                    upstream: target.upstream_short,
                },
            });
        }
        if ahead == 0 || effective == PullStrategy::FfOnly {
            let paths = open.repo.merge_paths(git, &target.upstream_short)?;
            let action = format!("Pull {}", target.upstream_short);
            let opened = match self.write_destructive(id, &action, &paths, |repo, git| {
                repo.fast_forward(git, &target.upstream_short)
            }) {
                Ok(o) => o,
                Err(e) => {
                    let _ = self.refresh(id);
                    return Err(e);
                }
            };
            return Ok(PullResult {
                opened,
                outcome: PullOutcome::FastForwarded,
            });
        }
        match effective {
            PullStrategy::Merge => {
                let paths = open.repo.merge_paths(git, &target.upstream_short)?;
                let action = format!("Pull (merge) {}", target.upstream_short);
                let mut outcome = PullOutcome::Merged;
                let opened = match self.write_destructive(id, &action, &paths, |repo, git| {
                    if repo.merge(git, &target.upstream_short)? == MergeOutcome::Conflicts {
                        outcome = PullOutcome::Conflicts;
                    }
                    Ok(())
                }) {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = self.refresh(id);
                        return Err(e);
                    }
                };
                Ok(PullResult { opened, outcome })
            }
            PullStrategy::Rebase => {
                let paths = open.repo.rebase_paths(git, &target.upstream_short)?;
                let action = format!("Pull (rebase) {}", target.upstream_short);
                let mut outcome = PullOutcome::Rebased;
                let opened = match self.write_destructive(id, &action, &paths, |repo, git| {
                    if repo.rebase(git, &target.upstream_short)? == RebaseOutcome::Stopped {
                        outcome = PullOutcome::Conflicts;
                    }
                    Ok(())
                }) {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = self.refresh(id);
                        return Err(e);
                    }
                };
                Ok(PullResult { opened, outcome })
            }
            PullStrategy::Auto | PullStrategy::FfOnly => unreachable!(),
        }
    }

    /// Pushes `branch` to `remote`'s `remote_branch`, reporting progress.
    /// Force pushes strictly use `--force-with-lease`. Not recorded in the op log
    /// because pushes cannot be undone remotely.
    ///
    /// # Errors
    /// [`Error::Refused`], [`Error::Cancelled`], or git's error.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &self,
        id: u32,
        remote: &str,
        branch: &str,
        remote_branch: &str,
        set_upstream: bool,
        force_with_lease: bool,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        self.run_task(id, |cancel| {
            open.repo.push(
                git,
                remote,
                branch,
                remote_branch,
                set_upstream,
                force_with_lease,
                &mut on_progress,
                cancel,
            )
        })?;
        self.refresh_inner(id, set_upstream)
    }

    /// Deletes `branch` on `remote`. Irreversible remotely.
    ///
    /// # Errors
    /// See [`Repo::delete_remote_branch`].
    pub fn delete_remote_branch(
        &self,
        id: u32,
        remote: &str,
        branch: &str,
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        open.repo
            .delete_remote_branch(self.git()?, remote, branch)?;
        self.refresh(id)
    }

    fn run_task<T>(
        &self,
        id: u32,
        f: impl FnOnce(&Cancel) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let cancel = Cancel::default();
        {
            let mut tasks = self.tasks.lock().expect("task lock poisoned");
            if tasks.contains_key(&id) {
                return Err(Error::Refused(
                    "a remote operation is already running".into(),
                ));
            }
            tasks.insert(id, cancel.clone());
        }
        let result = f(&cancel);
        self.tasks.lock().expect("task lock poisoned").remove(&id);
        result
    }

    /// Cancels repo `id`'s running remote task. Returns whether one was running.
    pub fn cancel(&self, id: u32) -> bool {
        match self.tasks.lock().expect("task lock poisoned").get(&id) {
            Some(cancel) => {
                cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Stages selected line indices within one hunk of `path`, undoable via Ctrl+Z.
    ///
    /// # Errors
    /// As [`Repo::stage_lines`].
    pub fn stage_lines(&self, id: u32, path: &str, select: &LineSelect) -> Result<Opened, Error> {
        let action = format!("Stage {} lines in {path}", select.lines.len());
        self.write(id, &action, |repo, git| repo.stage_lines(git, path, select))
    }

    /// Unstages selected line indices within one hunk of `path`, undoable via Ctrl+Z.
    ///
    /// # Errors
    /// As [`Repo::unstage_lines`].
    pub fn unstage_lines(&self, id: u32, path: &str, select: &LineSelect) -> Result<Opened, Error> {
        let action = format!("Unstage {} lines in {path}", select.lines.len());
        self.write(id, &action, |repo, git| {
            repo.unstage_lines(git, path, select)
        })
    }

    /// Discards selected line indices within one hunk of `path`, snapshotting first for Ctrl+Z.
    ///
    /// # Errors
    /// As [`Repo::discard_lines`].
    pub fn discard_lines(&self, id: u32, path: &str, select: &LineSelect) -> Result<Opened, Error> {
        let action = format!("Discard {} lines in {path}", select.lines.len());
        let paths = [path.to_owned()];
        self.write_destructive(id, &action, &paths, |repo, git| {
            repo.discard_lines(git, path, select)
        })
    }

    /// Updates local git identity for repo `id` and refreshes if config changed.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Command`], or [`Error::Read`].
    pub fn set_repo_identity(&self, id: u32, name: &str, email: &str) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        config::write_identity(git, Some(open.repo.root()), name, email)?;
        self.refresh(id)
    }

    /// Runs `f` as the only write on repo `id`, with its watcher paused.
    fn exclusive<T>(
        &self,
        id: u32,
        f: impl FnOnce(&OpenRepo, &GitCli) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let _one_writer = open.writing.lock().expect("write lock poisoned");
        let _paused = self.watchers().get(&id).map(RepoWatcher::pause);
        f(&open, git)
    }

    /// The git CLI, located on first use.
    ///
    /// # Errors
    /// [`Error::Git`] when no usable git is on PATH.
    pub fn git(&self) -> Result<&GitCli, Error> {
        if let Some(cli) = self.git.get() {
            return Ok(cli);
        }
        let cli = GitCli::new(&git::locate()?);
        Ok(self.git.get_or_init(|| cli))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<u32, OpenRepo>> {
        self.open.lock().expect("session lock poisoned")
    }

    fn watchers(&self) -> MutexGuard<'_, HashMap<u32, RepoWatcher>> {
        self.watchers.lock().expect("watcher lock poisoned")
    }
}

fn opened(id: u32, open: &OpenRepo) -> Opened {
    Opened {
        id,
        root: open.repo.root().to_owned(),
        rows: open.graph.len(),
        graph_version: open.graph_version,
    }
}
