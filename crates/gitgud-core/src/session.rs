use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::ai::{self, AiProviderStatus};
use crate::blame::{BlameLine, FileCommit};
use crate::cli::GitCli;
use crate::config;
use crate::diff::is_binary;
use crate::git;
use crate::graph::{Graph, Row};
use crate::history::{Action, HistoryPlan, PlanPrediction, Step};
use crate::lfs::{self, LfsCacheStats, LfsInfo, LfsLock};
use crate::merge::{MergeOutcome, MergeState, Side};
use crate::oplog::{OpEntry, OpLog, OpLogStats, ReflogEntry, label_amend, label_commit};
use crate::ops::{CommitOptions, ResetMode};
use crate::proposals::{self, ProposalSummary, WorktreeInfo};
use crate::rebase::{RebaseOutcome, RebaseState, RebaseStep};
use crate::remote::{PullOutcome, PullStrategy};
use crate::repo::{self, Error, Repo};
use crate::sequencer::{SequencerKind, SequencerState, SequencerStep};
use crate::stacks::BranchStack;
use crate::tags::TagInfo;
use crate::task::{Cancel, Progress};
use crate::unity::{self, MetaIssue, UnityInfo};
use crate::unity_yaml::{
    self, UnityConflictTable, UnityIntegrityIssue, UnityPropertyPick, UnityRepoSetupStatus,
    UnityYamlDiff,
};
use crate::watch::RepoWatcher;
use crate::worktree::LineSelect;

/// Local operation timing sample for diagnostics (no external telemetry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingSample {
    pub op: String,
    pub elapsed_ms: u64,
    pub at: i64,
}

const MAX_TIMING_SAMPLES: usize = 50;

type FetchLocks = Mutex<HashMap<(u32, String), Arc<Mutex<()>>>>;

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
    /// Last silent background refresh timestamp per repo id.
    last_bg_refresh: Mutex<HashMap<u32, Instant>>,
    /// Per-(repo_id, oid) locks so concurrent image requests never race on the same LFS fetch.
    lfs_fetches: FetchLocks,
    /// Recent local operation timings (oldest evicted past 50).
    timings: Mutex<VecDeque<TimingSample>>,
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
        let started = Instant::now();
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
        warm_guid_index(&open.repo);
        self.lock().insert(id, open);
        self.record_timing("open", started);
        Ok(opened)
    }

    /// Reads working-copy status of repo `id` and records a local latency sample.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn repo_status(&self, id: u32) -> Result<crate::repo::Status, Error> {
        let started = Instant::now();
        let status = self.get(id)?.repo.status()?;
        self.record_timing("status", started);
        Ok(status)
    }

    /// Reads a slice of commit graph rows for repo `id` and records a local latency sample.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn graph_rows(&self, id: u32, start: usize, count: usize) -> Result<Vec<Row>, Error> {
        let started = Instant::now();
        let open = self.get(id)?;
        let rows = open.graph.rows(&open.repo, start, count)?;
        self.record_timing("graph_rows", started);
        Ok(rows)
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
        let started = Instant::now();
        let open = self.get(id)?;
        unity_yaml::forget_guid_index(&open.repo);
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
        self.record_timing("refresh", started);
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
        self.last_bg_refresh
            .lock()
            .expect("bg refresh lock poisoned")
            .remove(&id);
        self.lfs_fetches
            .lock()
            .expect("lfs fetches lock poisoned")
            .retain(|(repo_id, _), _| *repo_id != id);
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
        let watched = repo.clone();
        let watcher = RepoWatcher::start(repo, move || {
            unity_yaml::forget_guid_index(&watched);
            on_change();
        })?;
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
        let started = Instant::now();
        self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo).record(&open.repo, action, || op(&open.repo, git))
        })?;
        self.record_timing(action, started);
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
        let started = Instant::now();
        self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo)
                .record_destructive(&open.repo, git, action, paths, || op(&open.repo, git))
        })?;
        self.record_timing(action, started);
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

    /// Stages `paths`, auto-pairing matching `.meta` <-> asset changes present in status.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Read`], or git's error.
    pub fn stage(&self, id: u32, paths: &[String]) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let status = open.repo.status()?;
        let (expanded, paired) = unity::expand_meta_pairs(&status, paths);
        let action = label_paths_meta("Stage", paths, &expanded, &paired);
        self.write(id, &action, |repo, git| repo.stage(git, &expanded))
    }

    /// Unstages `paths`, auto-pairing matching `.meta` <-> asset changes present in status.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Read`], or git's error.
    pub fn unstage(&self, id: u32, paths: &[String]) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let status = open.repo.status()?;
        let (expanded, paired) = unity::expand_meta_pairs(&status, paths);
        let action = label_paths_meta("Unstage", paths, &expanded, &paired);
        self.write(id, &action, |repo, git| repo.unstage(git, &expanded))
    }

    /// Discards `paths`, auto-pairing matching `.meta` <-> asset changes present in status; undoable.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Read`], or git's error.
    pub fn discard(&self, id: u32, paths: &[String]) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let status = open.repo.status()?;
        let (expanded, paired) = unity::expand_meta_pairs(&status, paths);
        let action = label_paths_meta("Discard", paths, &expanded, &paired);
        self.write_destructive(id, &action, &expanded, |repo, git| {
            repo.discard(git, &expanded)
        })
    }

    /// Detects Unity project info and Smart Merge configuration for repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn unity_info(&self, id: u32) -> Result<UnityInfo, Error> {
        let open = self.get(id)?;
        unity::detect_unity(self.git()?, &open.repo)
    }

    /// Configures Unity Smart Merge in local `.git/config` and `.git/info/attributes` only.
    ///
    /// # Errors
    /// See [`unity::configure_smart_merge`].
    pub fn unity_configure_smart_merge(
        &self,
        id: u32,
        tool_path: Option<&str>,
    ) -> Result<UnityInfo, Error> {
        let open = self.get(id)?;
        let info = unity::configure_smart_merge(self.git()?, &open.repo, tool_path)?;
        self.refresh(id)?;
        Ok(info)
    }

    /// Checks changed Unity `.meta` files for missing, orphan, colliding GUID, or case issues.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn unity_meta_issues(&self, id: u32) -> Result<Vec<MetaIssue>, Error> {
        let open = self.get(id)?;
        let status = open.repo.status()?;
        Ok(unity::check_meta_issues(&open.repo, &status))
    }

    /// Re-runs Unity Smart Merge over conflicted Unity YAML files; undoable via Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::remerge_unity_files`].
    pub fn unity_remerge_conflicts(&self, id: u32) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let paths = open.repo.conflicted_paths()?;
        self.write_destructive(id, "Run Unity Smart Merge", &paths, |repo, git| {
            repo.remerge_unity_files(git).map(drop)
        })
    }

    /// Confirms or unconfirms Unity Smart Merge property changes on `path`; undoable via Ctrl+Z.
    ///
    /// # Errors
    /// See [`Repo::confirm_unity_verify`].
    pub fn unity_confirm_verify(
        &self,
        id: u32,
        path: &str,
        file_id: Option<&str>,
        property: Option<&str>,
        confirmed: bool,
    ) -> Result<Opened, Error> {
        let action = if confirmed {
            format!("Verify Smart Merge in {path}")
        } else {
            format!("Unconfirm Smart Merge in {path}")
        };
        self.write(id, &action, |repo, _| {
            repo.confirm_unity_verify(path, file_id, property, confirmed)
        })
    }

    /// Returns whether Unity Editor lockfile is present AND `range` touches `Assets/`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn unity_check_open_assets(&self, id: u32, range: &str) -> Result<bool, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let info = unity::detect_unity(git, &open.repo)?;
        if !info.editor_open {
            return Ok(false);
        }
        unity::range_touches_assets(git, open.repo.root(), range)
    }

    /// Git LFS info and host lock support (Bitbucket Cloud disables locks).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn lfs_info(&self, id: u32) -> Result<LfsInfo, Error> {
        let open = self.get(id)?;
        lfs::lfs_info(self.git()?, &open.repo)
    }

    /// Lists Git LFS locks on the remote.
    ///
    /// # Errors
    /// See [`lfs::lfs_locks_list`].
    pub fn lfs_locks(&self, id: u32) -> Result<Vec<LfsLock>, Error> {
        let open = self.get(id)?;
        lfs::lfs_locks_list(self.git()?, &open.repo)
    }

    /// Locks `path` with `git lfs lock`.
    ///
    /// # Errors
    /// See [`lfs::lfs_lock`].
    pub fn lfs_lock(&self, id: u32, path: &str) -> Result<(), Error> {
        let open = self.get(id)?;
        lfs::lfs_lock(self.git()?, &open.repo, path)
    }

    /// Unlocks `path` with `git lfs unlock`.
    ///
    /// # Errors
    /// See [`lfs::lfs_unlock`].
    pub fn lfs_unlock(&self, id: u32, path: &str, force: bool) -> Result<(), Error> {
        let open = self.get(id)?;
        lfs::lfs_unlock(self.git()?, &open.repo, path, force)
    }

    /// Lists active locks held by us on files touched in commits about to be pushed.
    ///
    /// # Errors
    /// See [`lfs::locks_in_push`].
    pub fn lfs_locks_in_push(
        &self,
        id: u32,
        remote: &str,
        remote_branch: &str,
        branch: &str,
    ) -> Result<Vec<LfsLock>, Error> {
        let open = self.get(id)?;
        lfs::locks_in_push(self.git()?, &open.repo, remote, remote_branch, branch)
    }

    /// Quiet non-interactive background fetch when repo is idle and stale beyond `min_age_secs`.
    /// Returns `Ok(true)` if fetched and refreshed, `Ok(false)` when skipped or offline.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] if closed.
    pub fn background_refresh(&self, id: u32, min_age_secs: u64) -> Result<bool, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        if open.repo.primary_remote_url().is_none() {
            return Ok(false);
        }
        if open.repo.merge_state(git).ok().flatten().is_some()
            || open.repo.rebase_state(git).ok().flatten().is_some()
            || open.repo.sequencer_state(git).ok().flatten().is_some()
        {
            return Ok(false);
        }
        {
            let guard = self
                .last_bg_refresh
                .lock()
                .expect("bg refresh lock poisoned");
            if let Some(prev) = guard.get(&id)
                && prev.elapsed() < Duration::from_secs(min_age_secs)
            {
                return Ok(false);
            }
        }
        if open.writing.try_lock().is_err() {
            return Ok(false);
        }
        let ok = git
            .run_background(open.repo.root(), &["fetch", "--all", "--prune"])
            .is_ok();
        self.last_bg_refresh
            .lock()
            .expect("bg refresh lock poisoned")
            .insert(id, Instant::now());
        if ok {
            let _ = self.refresh_inner(id, true);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists tags in repo `id`, newest first.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn tags(&self, id: u32) -> Result<Vec<TagInfo>, Error> {
        let open = self.get(id)?;
        open.repo.tags()
    }

    /// Creates a lightweight or annotated tag in repo `id` and refreshes the open view.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn create_tag(
        &self,
        id: u32,
        name: &str,
        target: Option<&str>,
        message: Option<&str>,
    ) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| {
            open.repo.create_tag(git, name, target, message)
        })?;
        self.refresh(id)
    }

    /// Deletes local tag `name` in repo `id` and refreshes the open view.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn delete_tag(&self, id: u32, name: &str) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| open.repo.delete_tag(git, name))?;
        self.refresh(id)
    }

    /// Pushes tag `name` to `remote`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn push_tag(&self, id: u32, remote: &str, name: &str) -> Result<(), Error> {
        let open = self.get(id)?;
        open.repo.push_tag(self.git()?, remote, name)
    }

    /// Deletes tag `name` from `remote`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn delete_remote_tag(&self, id: u32, remote: &str, name: &str) -> Result<(), Error> {
        let open = self.get(id)?;
        open.repo.delete_remote_tag(self.git()?, remote, name)
    }

    /// Lists recent commits touching `path` (following renames).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn file_history(
        &self,
        id: u32,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Vec<FileCommit>, Error> {
        let open = self.get(id)?;
        open.repo
            .file_history(self.git()?, path, limit.unwrap_or(200))
    }

    /// Returns line-by-line blame annotations for `path` at `rev` (or worktree HEAD).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn blame_file(
        &self,
        id: u32,
        path: &str,
        rev: Option<&str>,
    ) -> Result<Vec<BlameLine>, Error> {
        let open = self.get(id)?;
        open.repo.blame_file(self.git()?, path, rev)
    }

    /// Returns op log entry counts, byte footprint, and prunable summary for repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn op_log_stats(&self, id: u32) -> Result<OpLogStats, Error> {
        let open = self.get(id)?;
        OpLog::for_repo(&open.repo).stats(&open.repo)
    }

    /// Prunes op log entries older than 30 days beyond the most recent 2,000 entries.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn op_log_prune(&self, id: u32) -> Result<OpLogStats, Error> {
        self.exclusive(id, |open, git| {
            OpLog::for_repo(&open.repo).prune_now(&open.repo, git, 2000, 30 * 24 * 60 * 60)
        })
    }

    /// Lists recent git reflog entries on HEAD (commits made inside or outside gitgud).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn outside_reflog(&self, id: u32, limit: Option<usize>) -> Result<Vec<ReflogEntry>, Error> {
        let open = self.get(id)?;
        open.repo.outside_reflog(self.git()?, limit.unwrap_or(100))
    }

    /// Returns whether `.git/objects/info/commit-graph` (or chain) exists.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`].
    pub fn has_commit_graph(&self, id: u32) -> Result<bool, Error> {
        let open = self.get(id)?;
        Ok(open.repo.has_commit_graph())
    }

    /// Writes `git commit-graph write --reachable --changed-paths` for faster graph loads.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn write_commit_graph(&self, id: u32) -> Result<(), Error> {
        self.exclusive(id, |open, git| open.repo.write_commit_graph(git))
    }

    /// See [`Repo::cache_stats`].
    ///
    /// # Errors
    /// [`Error::UnknownRepo`].
    pub fn cache_stats(&self, id: u32) -> Result<crate::caches::CacheStats, Error> {
        Ok(self.get(id)?.repo.cache_stats())
    }

    /// See [`Repo::clear_unity_caches`]; returns the stats after.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or as [`Repo::clear_unity_caches`].
    pub fn clear_unity_caches(&self, id: u32) -> Result<crate::caches::CacheStats, Error> {
        let repo = self.get(id)?.repo;
        repo.clear_unity_caches()?;
        Ok(repo.cache_stats())
    }

    /// Structural property diff of a working-copy Unity YAML file (staged or unstaged).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Read`].
    pub fn worktree_unity_diff(
        &self,
        id: u32,
        path: &str,
        side: crate::worktree::Side,
    ) -> Result<UnityYamlDiff, Error> {
        let open = self.get(id)?;
        unity_yaml::worktree_unity_diff(&open.repo, path, side)
    }

    /// Structural property diff of a Unity YAML file in a historical commit.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Read`].
    pub fn commit_unity_diff(
        &self,
        id: u32,
        commit: &str,
        path: &str,
    ) -> Result<UnityYamlDiff, Error> {
        let open = self.get(id)?;
        unity_yaml::commit_unity_diff(&open.repo, commit, path)
    }

    /// Builds an R4-A 3-way property conflict table for conflicted Unity YAML `path`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Refused`] when binary, deleted on one side, or non-Unity YAML.
    pub fn unity_conflict_table(&self, id: u32, path: &str) -> Result<UnityConflictTable, Error> {
        if !unity_yaml::is_unity_yaml_path(path) {
            return Err(Error::Refused(format!("{path} is not a Unity YAML file")));
        }
        let open = self.get(id)?;
        let [base, yours, theirs] = open.repo.stages(path)?;
        let (Some(yours), Some(theirs)) = (yours, theirs) else {
            return Err(Error::Refused(format!(
                "{path} was deleted on one side: pick a side"
            )));
        };
        let base = base.unwrap_or_default();
        if is_binary(&yours) || is_binary(&theirs) || is_binary(&base) {
            return Err(Error::Refused(format!("{path} is binary: pick a side")));
        }
        let b = String::from_utf8_lossy(&base);
        let o = String::from_utf8_lossy(&yours);
        let t = String::from_utf8_lossy(&theirs);
        let guids = unity_yaml::repo_guid_index(&open.repo);
        unity_yaml::build_conflict_table(path, &b, &o, &t, &guids)
    }

    /// Applies R4-A property picks on conflicted Unity YAML `path`, writes, stages, and records undoable resolve.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or git write failure.
    pub fn resolve_unity_conflict(
        &self,
        id: u32,
        path: &str,
        picks: &[UnityPropertyPick],
    ) -> Result<Opened, Error> {
        let open = self.get(id)?;
        let [base, yours, theirs] = open.repo.stages(path)?;
        let (Some(yours), Some(theirs)) = (yours, theirs) else {
            return Err(Error::Refused(format!(
                "{path} was deleted on one side: pick a side"
            )));
        };
        let base = base.unwrap_or_default();
        if is_binary(&yours) || is_binary(&theirs) || is_binary(&base) {
            return Err(Error::Refused(format!("{path} is binary: pick a side")));
        }
        let b = String::from_utf8_lossy(&base);
        let o = String::from_utf8_lossy(&yours);
        let t = String::from_utf8_lossy(&theirs);
        let merged = unity_yaml::apply_unity_conflict_picks(&b, &o, &t, picks)?;
        self.resolve_with(id, path, &merged, "property table")
    }

    /// Runs post-merge Unity YAML integrity verification over `paths` in repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`].
    pub fn unity_post_merge_check(
        &self,
        id: u32,
        paths: &[String],
    ) -> Result<Vec<UnityIntegrityIssue>, Error> {
        let open = self.get(id)?;
        Ok(unity_yaml::check_repo_unity_integrity(&open.repo, paths))
    }

    /// Inspects `.gitignore`, `.gitattributes`, and Unity EditorSettings `Force Text` status.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn unity_repo_setup_status(&self, id: u32) -> Result<UnityRepoSetupStatus, Error> {
        let open = self.get(id)?;
        unity_yaml::unity_repo_setup_status(&open.repo)
    }

    /// Writes missing standard Unity `.gitignore` and/or `.gitattributes` entries.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn unity_apply_repo_setup(
        &self,
        id: u32,
        write_gitignore: bool,
        write_gitattributes: bool,
    ) -> Result<UnityRepoSetupStatus, Error> {
        self.exclusive(id, |open, _| {
            unity_yaml::apply_unity_repo_setup(&open.repo, write_gitignore, write_gitattributes)
        })
    }

    /// Lists all agent proposals in repo `id`, newest first.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn proposals_list(&self, id: u32) -> Result<Vec<ProposalSummary>, Error> {
        let open = self.get(id)?;
        proposals::list_proposals(&open.repo)
    }

    /// Reads the diff for one file inside an agent proposal commit.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Read`].
    pub fn proposal_file_diff(
        &self,
        id: u32,
        proposal_id: &str,
        path: &str,
    ) -> Result<crate::diff::FileDiff, Error> {
        let open = self.get(id)?;
        proposals::proposal_file_diff(&open.repo, proposal_id, path)
    }

    /// Approves an agent proposal (applying & committing onto current HEAD via `git commit`, undoable via Ctrl+Z).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn proposal_approve(
        &self,
        id: u32,
        proposal_id: &str,
        message: Option<&str>,
        selected_paths: &[String],
    ) -> Result<Opened, Error> {
        let prop = {
            let open = self.get(id)?;
            proposals::get_proposal(&open.repo, proposal_id)?
        };
        let paths: Vec<String> = if selected_paths.is_empty() {
            prop.files.clone()
        } else {
            selected_paths.to_vec()
        };
        let label = format!("Approve proposal {}", prop.short_commit);
        self.write_destructive(id, &label, &paths, |repo, git| {
            proposals::approve_proposal(repo, git, proposal_id, message, selected_paths).map(drop)
        })
    }

    /// Requests changes on a proposal with a review comment and returns the updated summary.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Refused`].
    pub fn proposal_request_changes(
        &self,
        id: u32,
        proposal_id: &str,
        comment: &str,
        path: Option<&str>,
    ) -> Result<ProposalSummary, Error> {
        let open = self.get(id)?;
        proposals::request_changes(&open.repo, proposal_id, comment, path)
    }

    /// Rejects an agent proposal, removes its hidden ref, and refreshes the graph.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn proposal_reject(
        &self,
        id: u32,
        proposal_id: &str,
        reason: Option<&str>,
    ) -> Result<Opened, Error> {
        self.exclusive(id, |open, git| {
            proposals::reject_proposal(&open.repo, git, proposal_id, reason).map(drop)
        })?;
        self.refresh(id)
    }

    /// Detects stacked branch chains (depth >= 2) rooted on trunk in repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn branch_stacks(&self, id: u32) -> Result<Vec<BranchStack>, Error> {
        let open = self.get(id)?;
        open.repo.branch_stacks()
    }

    /// Rebases a stacked branch chain ending at `top_branch` onto `onto` using `--update-refs` (undoable via Ctrl+Z).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn restack(&self, id: u32, top_branch: &str, onto: &str) -> Result<Opened, Error> {
        let paths = {
            let open = self.get(id)?;
            let git = self.git()?;
            open.repo.restack_paths(git, top_branch, onto)?
        };
        let label = format!("Restack {top_branch} onto {onto}");
        self.write_destructive(id, &label, &paths, |repo, git| {
            repo.restack(git, top_branch, onto).map(drop)
        })
    }

    /// Returns the host PR / compare URL for `branch` targeting optional `base`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn pr_url_with_base(
        &self,
        id: u32,
        branch: &str,
        base: Option<&str>,
    ) -> Result<Option<String>, Error> {
        let open = self.get(id)?;
        open.repo.pr_url_with_base(branch, base)
    }

    /// Measures local Git LFS cache footprint and dry-run prunable objects in repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Read`], or [`Error::Command`].
    pub fn lfs_cache_stats(&self, id: u32) -> Result<LfsCacheStats, Error> {
        let open = self.get(id)?;
        lfs::lfs_cache_stats(self.git()?, &open.repo)
    }

    /// Prunes unreferenced local Git LFS objects in repo `id` and returns updated cache stats.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn lfs_prune(&self, id: u32) -> Result<LfsCacheStats, Error> {
        self.exclusive(id, |open, git| lfs::lfs_prune(git, &open.repo))
    }

    /// Downloads one unhydrated LFS object (`oid`) into repo `id`'s local cache, serializing
    /// concurrent requests for the same `(id, oid)` without blocking unrelated operations.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn lfs_fetch_object(
        &self,
        id: u32,
        oid: &str,
        size: u64,
        path: Option<&str>,
    ) -> Result<bool, Error> {
        let open = self.get(id)?;
        let git = self.git()?;
        let normalized = oid.trim().to_ascii_lowercase();
        let gate = {
            let mut map = self.lfs_fetches.lock().expect("lfs fetches lock poisoned");
            Arc::clone(
                map.entry((id, normalized.clone()))
                    .or_insert_with(|| Arc::new(Mutex::new(()))),
            )
        };
        let _guard = gate.lock().expect("lfs oid fetch lock poisoned");
        lfs::lfs_fetch_object(git, &open.repo, &normalized, size, path)
    }

    /// Reads and decodes image `path` at `version` for repo `id`. When `auto_fetch` is `true`,
    /// unhydrated LFS pointers up to 25 MiB are downloaded automatically on first preview.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], [`Error::Read`], or [`Error::Command`].
    pub fn image_bytes(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
        auto_fetch: bool,
    ) -> Result<Vec<u8>, Error> {
        let repo = self.get(id)?.repo.clone();
        if !auto_fetch {
            return repo.image_bytes(path, version);
        }
        if let Some((oid, size)) = repo.image_lfs_pointer(path, version)?
            && size <= lfs::AUTO_FETCH_MAX_BYTES
            && !lfs::lfs_is_hydrated(&repo, &oid)
            && repo.primary_remote_url().is_some()
        {
            self.lfs_fetch_object(id, &oid, size, Some(path))?;
        }
        repo.image_bytes_with_fetch(None, path, version, lfs::AUTO_FETCH_MAX_BYTES)
    }

    /// Model `path` at `version`, LFS objects up to the auto-fetch limit fetched first.
    ///
    /// # Errors
    /// As [`Repo::model_bytes`].
    pub fn model_bytes(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<Vec<u8>, Error> {
        let repo = self.get(id)?.repo.clone();
        if crate::model::model_kind(path).is_some()
            && let Some((oid, size)) = lfs::parse_lfs_pointer(&repo.file_at(path, version)?)
            && size <= lfs::AUTO_FETCH_MAX_BYTES
            && !lfs::lfs_is_hydrated(&repo, &oid)
            && repo.primary_remote_url().is_some()
        {
            self.lfs_fetch_object(id, &oid, size, Some(path))?;
        }
        repo.model_bytes(None, path, version, lfs::AUTO_FETCH_MAX_BYTES)
    }

    /// `path` at `version` as stored, LFS objects up to the auto-fetch limit fetched first.
    ///
    /// # Errors
    /// As [`Repo::file_bytes`].
    pub fn file_bytes(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<Vec<u8>, Error> {
        let repo = self.get(id)?.repo.clone();
        if let Some((oid, size)) = repo.lfs_pointer_at(path, version)?
            && size <= lfs::AUTO_FETCH_MAX_BYTES
            && !lfs::lfs_is_hydrated(&repo, &oid)
            && repo.primary_remote_url().is_some()
        {
            self.lfs_fetch_object(id, &oid, size, Some(path))?;
        }
        repo.file_bytes(None, path, version, lfs::AUTO_FETCH_MAX_BYTES)
    }

    /// See [`Repo::font_info`]; LFS objects up to the auto-fetch limit are fetched first.
    ///
    /// # Errors
    /// As [`Session::file_bytes`], or when the file can't be read as a font.
    pub fn font_info(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
        face: u32,
    ) -> Result<crate::font::FontInfo, Error> {
        let bytes = self.file_bytes(id, path, version)?;
        crate::font::read_font(&bytes, face).map_err(|e| Error::Refused(format!("{path}: {e}")))
    }

    /// See [`Repo::unity_asset_info`].
    ///
    /// # Errors
    /// As [`Repo::unity_asset_info`].
    pub fn unity_asset_info(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<crate::preview::UnityAssetInfo, Error> {
        self.get(id)?.repo.unity_asset_info(path, version)
    }

    /// Writes `path` at `version` to a read-only temp copy for opening in its default
    /// app; see [`crate::preview::write_open_copy`].
    ///
    /// # Errors
    /// As [`Session::file_bytes`], or when the copy can't be written.
    pub fn open_copy(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
        label: &str,
    ) -> Result<std::path::PathBuf, Error> {
        let bytes = self.file_bytes(id, path, version)?;
        crate::preview::write_open_copy(path, label, &bytes)
    }

    /// The file model `near` refers to as `reference` at `version`, found by
    /// [`Repo::model_reference`]: images decoded as [`Session::image_bytes`]
    /// does, anything else (a glTF `.bin`) as stored.
    ///
    /// # Errors
    /// [`Error::Refused`] when no such file is in that version; as [`Session::image_bytes`] otherwise.
    pub fn model_ref_bytes(
        &self,
        id: u32,
        near: &str,
        reference: &str,
        version: &crate::image::Version,
    ) -> Result<Vec<u8>, Error> {
        let repo = self.get(id)?.repo.clone();
        let path = repo
            .model_reference(near, reference, version)?
            .ok_or_else(|| Error::Refused(format!("{reference} is not in that version")))?;
        if crate::image::image_type(&path).is_some() {
            return self.image_bytes(id, &path, version, true);
        }
        let bytes = repo.file_at(&path, version)?;
        repo.lfs_resolve_with_fetch(None, &path, bytes, 0)
    }

    /// See [`Repo::model_info`].
    ///
    /// # Errors
    /// As [`Repo::model_info`].
    pub fn model_info(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<crate::model::ModelInfo, Error> {
        let repo = self.get(id)?.repo.clone();
        let git = self.git().ok();
        repo.model_info(git, path, version)
    }

    /// See [`Repo::material_view`].
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or as [`Repo::material_view`].
    pub fn material_view(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<crate::model::MaterialView, Error> {
        let repo = self.get(id)?.repo.clone();
        repo.material_view(self.git().ok(), path, version)
    }

    /// See [`Repo::material_users`].
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], or as [`Repo::material_users`].
    pub fn material_users(
        &self,
        id: u32,
        path: &str,
        version: &crate::image::Version,
    ) -> Result<Vec<String>, Error> {
        let repo = self.get(id)?.repo.clone();
        repo.material_users(self.git()?, path, version)
    }

    /// Detects available local AI CLIs and env key presence (never reads key values).
    #[must_use]
    pub fn ai_providers(&self) -> AiProviderStatus {
        ai::detect_ai_providers()
    }

    /// Drafts a commit message from staged changes using an available local AI CLI.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn ai_draft_commit_message(
        &self,
        id: u32,
        provider: Option<&str>,
    ) -> Result<String, Error> {
        let open = self.get(id)?;
        ai::draft_commit_message(self.git()?, &open.repo, provider)
    }

    /// Writes agent instructions (`claude`, `codex`, `cursor`, `generic`) into repo `id`.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn agent_setup(&self, id: u32, target: &str, with_hooks: bool) -> Result<String, Error> {
        let open = self.get(id)?;
        let root = open.repo.root();
        let rel = proposals::write_agent_setup(root, target)?;
        if with_hooks && target == "claude" {
            let hook_rel = proposals::write_claude_stop_hook(root)?;
            return Ok(format!("{rel} + {hook_rel}"));
        }
        Ok(rel)
    }

    /// Returns recent local operation timings, newest first (capped at 50).
    #[must_use]
    pub fn recent_timings(&self) -> Vec<TimingSample> {
        self.timings
            .lock()
            .expect("timings lock poisoned")
            .iter()
            .rev()
            .cloned()
            .collect()
    }

    /// Commits still reachable from `upstream` that are no longer ancestors of `HEAD` after a rewrite.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Read`].
    pub fn stale_origin_commits(&self, id: u32, upstream: &str) -> Result<Vec<String>, Error> {
        let open = self.get(id)?;
        open.repo.commits_only_in(upstream, "HEAD", 200)
    }

    /// Refreshes index stat metadata (`git update-index -q --refresh`) and reloads status.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn refresh_index(&self, id: u32) -> Result<Opened, Error> {
        let started = Instant::now();
        self.exclusive(id, |open, git| open.repo.refresh_index(git))?;
        self.record_timing("refresh_index", started);
        self.refresh(id)
    }

    /// Lists linked git worktrees attached to repo `id` (with dirty file counts).
    ///
    /// # Errors
    /// [`Error::UnknownRepo`] or [`Error::Command`].
    pub fn worktrees_list(&self, id: u32) -> Result<Vec<WorktreeInfo>, Error> {
        let open = self.get(id)?;
        proposals::list_worktrees(self.git()?, &open.repo)
    }

    /// Captures uncommitted edits in linked `worktree_path` into a proposal on repo `id` and refreshes the graph.
    ///
    /// # Errors
    /// [`Error::UnknownRepo`], [`Error::Refused`], or [`Error::Command`].
    pub fn propose_from_worktree(
        &self,
        id: u32,
        worktree_path: &Path,
        message: &str,
        why: Option<&str>,
        agent: Option<&str>,
    ) -> Result<ProposalSummary, Error> {
        let summary = self.exclusive(id, |open, git| {
            proposals::propose_from_worktree(&open.repo, git, worktree_path, message, why, agent)
        })?;
        self.refresh(id)?;
        Ok(summary)
    }

    fn record_timing(&self, op: &str, started: Instant) {
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let at = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        )
        .unwrap_or(0);
        let mut timings = self.timings.lock().expect("timings lock poisoned");
        if timings.len() >= MAX_TIMING_SAMPLES {
            timings.pop_front();
        }
        timings.push_back(TimingSample {
            op: op.to_owned(),
            elapsed_ms,
            at,
        });
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

fn label_paths_meta(
    verb: &str,
    original: &[String],
    expanded: &[String],
    paired: &[String],
) -> String {
    if paired.is_empty() {
        return match original {
            [one] => format!("{verb} {one}"),
            many => format!("{verb} {} files", many.len()),
        };
    }
    if original.len() == 1 && paired.len() == 1 {
        format!("{verb} {} + {}", original[0], paired[0])
    } else {
        format!("{verb} {} files (+ .meta)", expanded.len())
    }
}

/// Builds a Unity repo's GUID index in the background, so the first 3D preview
/// or structural diff doesn't wait for it.
fn warm_guid_index(repo: &Arc<repo::Repo>) {
    if crate::unity::find_unity_root(repo.root()).is_none() {
        return;
    }
    let repo = repo.clone();
    std::thread::spawn(move || {
        unity_yaml::repo_guid_index(&repo);
    });
}

fn opened(id: u32, open: &OpenRepo) -> Opened {
    Opened {
        id,
        root: open.repo.root().to_owned(),
        rows: open.graph.len(),
        graph_version: open.graph_version,
    }
}
