use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gitgud_core::ai::AiProviderStatus;
use gitgud_core::blame::{BlameLine, FileCommit};
use gitgud_core::branches::Branches;
use gitgud_core::caches::CacheStats;
use gitgud_core::clone::{CloneProgress, clone_repo as core_clone_repo};
use gitgud_core::config::{self, IdentityConfig, ToolDiagnostics};
use gitgud_core::diff::{CommitDetail, FileDiff};
use gitgud_core::git::{self, GitInfo};
use gitgud_core::graph::Row;
use gitgud_core::history::{Action, HistoryPlan, Step};
use gitgud_core::lfs::{LfsCacheStats, LfsInfo, LfsLock};
use gitgud_core::library::{
    AddRepoEntry, DEFAULT_SCAN_DEPTH, LibrarySnapshot, LibraryStore, OpenTabState, RepoSummary,
    ScanResult, display_path, scan_folder as core_scan_folder,
};
use gitgud_core::merge::{ConflictSides, ConflictText, MergeState, Side as MergeSide};
use gitgud_core::oplog::{OpEntry, OpLogStats, ReflogEntry};
use gitgud_core::ops::{CommitOptions, ResetMode};
use gitgud_core::proposals::{ProposalSummary, WorktreeInfo};
use gitgud_core::rebase::{RebasePlan, RebaseState, RebaseStep};
use gitgud_core::remote::{PullStrategy, PushInfo};
use gitgud_core::repo::{self, Error, Status};
use gitgud_core::sequencer::{SequencerState, SequencerStep};
use gitgud_core::session::{Opened, PlanPreview, PullResult, Reverted, Session, TimingSample};
use gitgud_core::stacks::BranchStack;
use gitgud_core::stash::StashEntry;
use gitgud_core::tags::TagInfo;
use gitgud_core::task::{Cancel, Progress};
use gitgud_core::unity::{MetaIssue, UnityInfo};
use gitgud_core::unity_yaml::{
    UnityConflictTable, UnityIntegrityIssue, UnityPropertyPick, UnityRepoSetupStatus, UnityYamlDiff,
};
use gitgud_core::worktree::{HunkHeader, LineSelect, Side};
use tauri::{AppHandle, Builder, Emitter, Manager, Runtime, State};

mod images;

#[derive(Default)]
struct ActiveClone(Mutex<Option<Cancel>>);

struct ClearClone<'a>(&'a Mutex<Option<Cancel>>);

impl Drop for ClearClone<'_> {
    fn drop(&mut self) {
        if let Ok(mut g) = self.0.lock() {
            *g = None;
        }
    }
}

#[tauri::command]
async fn git_info() -> Result<GitInfo, git::Error> {
    git::locate()
}

/// The repository to open at start-up, from `GITGUD_OPEN`.
#[tauri::command]
async fn launch_path() -> Option<String> {
    std::env::var("GITGUD_OPEN").ok().filter(|p| !p.is_empty())
}

/// Emitted with the repo id when files or refs change outside gitgud.
const REPO_CHANGED: &str = "repo-changed";

#[tauri::command]
async fn open_repo<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    library: State<'_, LibraryStore>,
    path: String,
) -> Result<Opened, Error> {
    let opened = session.open(&path)?;
    if let Err(e) = library.touch_repo(opened.root.to_string_lossy().as_ref()) {
        eprintln!("gitgud: cannot update library last_opened: {e}");
    }
    let id = opened.id;
    session.watch(id, move || {
        if let Err(e) = app.emit(REPO_CHANGED, id) {
            eprintln!("gitgud: cannot emit {REPO_CHANGED}: {e}");
        }
    })?;
    Ok(opened)
}

#[tauri::command]
async fn close_repo(session: State<'_, Session>, id: u32) -> Result<(), Error> {
    session.close(id)
}

#[tauri::command]
async fn repo_status(session: State<'_, Session>, id: u32) -> Result<Status, Error> {
    session.repo_status(id)
}

#[tauri::command]
async fn graph_rows(
    session: State<'_, Session>,
    id: u32,
    start: usize,
    count: usize,
) -> Result<Vec<Row>, Error> {
    session.graph_rows(id, start, count)
}

#[tauri::command]
async fn graph_find(
    session: State<'_, Session>,
    id: u32,
    query: String,
) -> Result<Vec<usize>, Error> {
    let open = session.get(id)?;
    open.graph.find(&open.repo, &query)
}

#[tauri::command]
async fn commit_detail(
    session: State<'_, Session>,
    id: u32,
    commit: String,
) -> Result<CommitDetail, Error> {
    session.get(id)?.repo.commit_detail(&commit)
}

#[tauri::command]
async fn commit_file_diff(
    session: State<'_, Session>,
    id: u32,
    commit: String,
    path: String,
    ignore_whitespace: Option<bool>,
) -> Result<FileDiff, Error> {
    session
        .get(id)?
        .repo
        .commit_file_diff_with(&commit, &path, ignore_whitespace.unwrap_or(false))
}

#[tauri::command]
async fn worktree_file_diff(
    session: State<'_, Session>,
    id: u32,
    path: String,
    side: Side,
    ignore_whitespace: Option<bool>,
) -> Result<FileDiff, Error> {
    session
        .get(id)?
        .repo
        .worktree_file_diff_with(&path, side, ignore_whitespace.unwrap_or(false))
}

#[tauri::command]
async fn stage_hunk(
    session: State<'_, Session>,
    id: u32,
    path: String,
    hunk: HunkHeader,
) -> Result<Opened, Error> {
    let action = format!("Stage hunk in {path}");
    session.write(id, &action, |repo, git| repo.stage_hunk(git, &path, hunk))
}

#[tauri::command]
async fn unstage_hunk(
    session: State<'_, Session>,
    id: u32,
    path: String,
    hunk: HunkHeader,
) -> Result<Opened, Error> {
    let action = format!("Unstage hunk in {path}");
    session.write(id, &action, |repo, git| repo.unstage_hunk(git, &path, hunk))
}

#[tauri::command]
async fn stage_lines(
    session: State<'_, Session>,
    id: u32,
    path: String,
    select: LineSelect,
) -> Result<Opened, Error> {
    session.stage_lines(id, &path, &select)
}

#[tauri::command]
async fn unstage_lines(
    session: State<'_, Session>,
    id: u32,
    path: String,
    select: LineSelect,
) -> Result<Opened, Error> {
    session.unstage_lines(id, &path, &select)
}

#[tauri::command]
async fn discard_lines(
    session: State<'_, Session>,
    id: u32,
    path: String,
    select: LineSelect,
) -> Result<Opened, Error> {
    session.discard_lines(id, &path, &select)
}

#[tauri::command]
async fn discard(
    session: State<'_, Session>,
    id: u32,
    paths: Vec<String>,
) -> Result<Opened, Error> {
    session.discard(id, &paths)
}

#[tauri::command]
async fn discard_hunk(
    session: State<'_, Session>,
    id: u32,
    path: String,
    hunk: HunkHeader,
) -> Result<Opened, Error> {
    let action = format!("Discard hunk in {path}");
    let paths = [path.clone()];
    session.write_destructive(id, &action, &paths, |repo, git| {
        repo.discard_hunk(git, &path, hunk)
    })
}

#[tauri::command]
async fn refresh_repo(session: State<'_, Session>, id: u32) -> Result<Opened, Error> {
    session.refresh(id)
}

#[tauri::command]
async fn stage(session: State<'_, Session>, id: u32, paths: Vec<String>) -> Result<Opened, Error> {
    session.stage(id, &paths)
}

#[tauri::command]
async fn unstage(
    session: State<'_, Session>,
    id: u32,
    paths: Vec<String>,
) -> Result<Opened, Error> {
    session.unstage(id, &paths)
}

#[tauri::command]
async fn commit(
    session: State<'_, Session>,
    id: u32,
    message: String,
    amend: Option<bool>,
    signoff: Option<bool>,
    no_verify: Option<bool>,
) -> Result<Opened, Error> {
    let options = CommitOptions {
        amend: amend.unwrap_or(false),
        signoff: signoff.unwrap_or(false),
        no_verify: no_verify.unwrap_or(false),
    };
    session.commit_with(id, &message, options)
}

#[tauri::command]
async fn head_commit_message(
    session: State<'_, Session>,
    id: u32,
) -> Result<Option<String>, Error> {
    session.get(id)?.repo.head_commit_message()
}

#[tauri::command]
async fn reset(
    session: State<'_, Session>,
    id: u32,
    target: String,
    mode: ResetMode,
) -> Result<Opened, Error> {
    session.reset(id, &target, mode)
}

#[tauri::command]
async fn branches(session: State<'_, Session>, id: u32) -> Result<Branches, Error> {
    session.get(id)?.repo.branches()
}

#[tauri::command]
async fn checkout(session: State<'_, Session>, id: u32, branch: String) -> Result<Opened, Error> {
    session.checkout(id, &branch)
}

#[tauri::command]
async fn create_branch(
    session: State<'_, Session>,
    id: u32,
    name: String,
    at: Option<String>,
    checkout: bool,
) -> Result<Opened, Error> {
    session.create_branch(id, &name, at.as_deref(), checkout)
}

#[tauri::command]
async fn rename_branch(
    session: State<'_, Session>,
    id: u32,
    old: String,
    new: String,
) -> Result<Opened, Error> {
    session.rename_branch(id, &old, &new)
}

#[tauri::command]
async fn delete_branch(
    session: State<'_, Session>,
    id: u32,
    name: String,
    force: bool,
) -> Result<Opened, Error> {
    session.delete_branch(id, &name, force)
}

#[tauri::command]
async fn set_upstream(
    session: State<'_, Session>,
    id: u32,
    branch: String,
    upstream: Option<String>,
) -> Result<Opened, Error> {
    session.set_upstream(id, &branch, upstream.as_deref())
}

#[tauri::command]
async fn checkout_remote(
    session: State<'_, Session>,
    id: u32,
    remote_branch: String,
    local_name: String,
) -> Result<Opened, Error> {
    session.checkout_remote(id, &remote_branch, &local_name)
}

#[tauri::command]
async fn predict_merge(
    session: State<'_, Session>,
    id: u32,
    branch: String,
) -> Result<Vec<String>, Error> {
    session.get(id)?.repo.predict_merge(session.git()?, &branch)
}

#[tauri::command]
async fn merge(session: State<'_, Session>, id: u32, branch: String) -> Result<Opened, Error> {
    session.merge(id, &branch)
}

#[tauri::command]
async fn merge_state(session: State<'_, Session>, id: u32) -> Result<Option<MergeState>, Error> {
    session.merge_state(id)
}

#[tauri::command]
async fn conflict_text(
    session: State<'_, Session>,
    id: u32,
    path: String,
) -> Result<ConflictText, Error> {
    session.get(id)?.repo.conflict_text(session.git()?, &path)
}

#[tauri::command]
async fn conflict_sides(
    session: State<'_, Session>,
    id: u32,
    path: String,
) -> Result<ConflictSides, Error> {
    session.get(id)?.repo.conflict_sides(&path)
}

#[tauri::command]
async fn resolve_with(
    session: State<'_, Session>,
    id: u32,
    path: String,
    content: String,
    how: String,
) -> Result<Opened, Error> {
    session.resolve_with(id, &path, &content, &how)
}

#[tauri::command]
async fn resolve_side(
    session: State<'_, Session>,
    id: u32,
    path: String,
    side: MergeSide,
) -> Result<Opened, Error> {
    session.resolve_side(id, &path, side)
}

#[tauri::command]
async fn merge_abort(session: State<'_, Session>, id: u32) -> Result<Opened, Error> {
    session.merge_abort(id)
}

#[tauri::command]
async fn merge_commit(
    session: State<'_, Session>,
    id: u32,
    message: String,
) -> Result<Opened, Error> {
    session.merge_commit(id, &message)
}

#[tauri::command]
async fn predict_rebase(
    session: State<'_, Session>,
    id: u32,
    onto: String,
) -> Result<RebasePlan, Error> {
    session.get(id)?.repo.predict_rebase(session.git()?, &onto)
}

#[tauri::command]
async fn rebase(session: State<'_, Session>, id: u32, onto: String) -> Result<Opened, Error> {
    session.rebase(id, &onto)
}

#[tauri::command]
async fn rebase_state(session: State<'_, Session>, id: u32) -> Result<Option<RebaseState>, Error> {
    session.rebase_state(id)
}

#[tauri::command]
async fn rebase_step(
    session: State<'_, Session>,
    id: u32,
    step: RebaseStep,
) -> Result<Opened, Error> {
    session.rebase_step(id, step)
}

#[tauri::command]
async fn history_plan(
    session: State<'_, Session>,
    id: u32,
    from: String,
) -> Result<HistoryPlan, Error> {
    session.history_plan(id, &from)
}

#[tauri::command]
async fn plan_preview(
    session: State<'_, Session>,
    id: u32,
    onto: String,
    base: String,
    steps: Vec<Step>,
) -> Result<PlanPreview, Error> {
    session.plan_preview(id, &onto, &base, &steps)
}

#[tauri::command]
async fn preview_rows(
    session: State<'_, Session>,
    id: u32,
    start: usize,
    count: usize,
) -> Result<Vec<Row>, Error> {
    session.preview_rows(id, start, count)
}

#[tauri::command]
async fn predicted_file(
    session: State<'_, Session>,
    id: u32,
    tree: String,
    path: String,
) -> Result<String, Error> {
    session
        .get(id)?
        .repo
        .predicted_file(session.git()?, &tree, &path)
}

#[tauri::command]
async fn run_plan(
    session: State<'_, Session>,
    id: u32,
    onto: String,
    base: String,
    steps: Vec<Step>,
    update_refs: bool,
) -> Result<Opened, Error> {
    session.run_plan(id, &onto, &base, &steps, update_refs)
}

#[tauri::command]
async fn reword_head(
    session: State<'_, Session>,
    id: u32,
    message: String,
) -> Result<Opened, Error> {
    session.reword_head(id, &message)
}

#[tauri::command]
async fn quick_edit(
    session: State<'_, Session>,
    id: u32,
    commit: String,
    action: Action,
) -> Result<Opened, Error> {
    session.quick_edit(id, &commit, action)
}

#[tauri::command]
async fn op_log(session: State<'_, Session>, id: u32) -> Result<Vec<OpEntry>, Error> {
    session.op_log(id)
}

#[tauri::command]
async fn restore_to(session: State<'_, Session>, id: u32, entry: u64) -> Result<Reverted, Error> {
    session.restore_to(id, entry)
}

#[tauri::command]
async fn restore_files(session: State<'_, Session>, id: u32, entry: u64) -> Result<Opened, Error> {
    session.restore_files(id, entry)
}

/// Emitted while a fetch runs: `{ id, progress }`.
const FETCH_PROGRESS: &str = "fetch-progress";
/// Emitted while fetch, pull or push runs: `{ id, op, progress }`.
const TASK_PROGRESS: &str = "task-progress";

#[derive(Clone, serde::Serialize)]
struct FetchProgress {
    id: u32,
    progress: Progress,
}

#[derive(Clone, serde::Serialize)]
struct TaskProgress {
    id: u32,
    op: &'static str,
    progress: Progress,
}

fn emit_task_progress<R: Runtime>(
    app: &AppHandle<R>,
    id: u32,
    op: &'static str,
    progress: Progress,
) {
    if op == "fetch"
        && let Err(e) = app.emit(
            FETCH_PROGRESS,
            FetchProgress {
                id,
                progress: progress.clone(),
            },
        )
    {
        eprintln!("gitgud: cannot emit {FETCH_PROGRESS}: {e}");
    }
    if let Err(e) = app.emit(TASK_PROGRESS, TaskProgress { id, op, progress }) {
        eprintln!("gitgud: cannot emit {TASK_PROGRESS}: {e}");
    }
}

#[tauri::command]
async fn fetch<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    id: u32,
) -> Result<Opened, Error> {
    session.fetch(id, |progress| {
        emit_task_progress(&app, id, "fetch", progress)
    })
}

/// Returns whether a fetch was running.
#[tauri::command]
async fn cancel_fetch(session: State<'_, Session>, id: u32) -> Result<bool, Error> {
    Ok(session.cancel(id))
}

/// Cancels any running remote task (fetch, pull, push) on repo `id`.
#[tauri::command]
async fn cancel_task(session: State<'_, Session>, id: u32) -> Result<bool, Error> {
    Ok(session.cancel(id))
}

#[tauri::command]
async fn pull<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    id: u32,
    strategy: PullStrategy,
    remember: Option<bool>,
) -> Result<PullResult, Error> {
    session.pull(id, strategy, remember.unwrap_or(false), |progress| {
        emit_task_progress(&app, id, "pull", progress);
    })
}

#[tauri::command]
async fn push_info(
    session: State<'_, Session>,
    id: u32,
    remote: Option<String>,
) -> Result<PushInfo, Error> {
    session.get(id)?.repo.push_info(remote.as_deref())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn push<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    id: u32,
    remote: String,
    branch: String,
    remote_branch: Option<String>,
    set_upstream: bool,
    force_with_lease: bool,
) -> Result<Opened, Error> {
    let target = remote_branch.as_deref().unwrap_or(&branch);
    session.push(
        id,
        &remote,
        &branch,
        target,
        set_upstream,
        force_with_lease,
        |progress| emit_task_progress(&app, id, "push", progress),
    )
}

#[tauri::command]
async fn delete_remote_branch(
    session: State<'_, Session>,
    id: u32,
    remote: String,
    branch: String,
) -> Result<Opened, Error> {
    session.delete_remote_branch(id, &remote, &branch)
}

#[tauri::command]
async fn predict_cherry_pick(
    session: State<'_, Session>,
    id: u32,
    commit: String,
) -> Result<Vec<String>, Error> {
    session
        .get(id)?
        .repo
        .predict_cherry_pick(session.git()?, &commit)
}

#[tauri::command]
async fn predict_revert(
    session: State<'_, Session>,
    id: u32,
    commit: String,
) -> Result<Vec<String>, Error> {
    session
        .get(id)?
        .repo
        .predict_revert(session.git()?, &commit)
}

#[tauri::command]
async fn cherry_pick(
    session: State<'_, Session>,
    id: u32,
    commit: String,
) -> Result<Opened, Error> {
    session.cherry_pick(id, &commit)
}

#[tauri::command]
async fn revert_commit(
    session: State<'_, Session>,
    id: u32,
    commit: String,
) -> Result<Opened, Error> {
    session.revert_commit(id, &commit)
}

#[tauri::command]
async fn sequencer_state(
    session: State<'_, Session>,
    id: u32,
) -> Result<Option<SequencerState>, Error> {
    session.sequencer_state(id)
}

#[tauri::command]
async fn sequencer_step(
    session: State<'_, Session>,
    id: u32,
    step: SequencerStep,
) -> Result<Opened, Error> {
    session.sequencer_step(id, step)
}

#[tauri::command]
async fn stashes(session: State<'_, Session>, id: u32) -> Result<Vec<StashEntry>, Error> {
    session.get(id)?.repo.stashes()
}

#[tauri::command]
async fn stash_detail(
    session: State<'_, Session>,
    id: u32,
    index: usize,
) -> Result<CommitDetail, Error> {
    session.get(id)?.repo.stash_detail(index)
}

#[tauri::command]
async fn stash_file_diff(
    session: State<'_, Session>,
    id: u32,
    index: usize,
    path: String,
    ignore_whitespace: Option<bool>,
) -> Result<FileDiff, Error> {
    session
        .get(id)?
        .repo
        .stash_file_diff_with(index, &path, ignore_whitespace.unwrap_or(false))
}

#[tauri::command]
async fn config_get_identity(
    session: State<'_, Session>,
    id: Option<u32>,
) -> Result<IdentityConfig, Error> {
    let git = session.git()?;
    match id {
        Some(repo_id) => config::read_identity(git, Some(session.get(repo_id)?.repo.root())),
        None => config::read_identity(git, None),
    }
}

#[tauri::command]
async fn config_set_identity(
    session: State<'_, Session>,
    id: Option<u32>,
    name: String,
    email: String,
) -> Result<(), Error> {
    if let Some(repo_id) = id {
        session.set_repo_identity(repo_id, &name, &email).map(drop)
    } else {
        config::write_identity(session.git()?, None, &name, &email)
    }
}

#[tauri::command]
async fn tool_diagnostics(session: State<'_, Session>) -> Result<ToolDiagnostics, Error> {
    let info = git::locate()?;
    Ok(config::tool_diagnostics(session.git()?, &info))
}

#[tauri::command]
async fn stash_save(
    session: State<'_, Session>,
    id: u32,
    message: Option<String>,
    include_untracked: bool,
) -> Result<Opened, Error> {
    session.stash_save(id, message.as_deref(), include_untracked)
}

#[tauri::command]
async fn stash_apply(session: State<'_, Session>, id: u32, index: usize) -> Result<Opened, Error> {
    session.stash_apply(id, index)
}

#[tauri::command]
async fn stash_pop(session: State<'_, Session>, id: u32, index: usize) -> Result<Opened, Error> {
    session.stash_pop(id, index)
}

#[tauri::command]
async fn stash_drop(session: State<'_, Session>, id: u32, index: usize) -> Result<Opened, Error> {
    session.stash_drop(id, index)
}

#[tauri::command]
async fn pr_url(
    session: State<'_, Session>,
    id: u32,
    branch: String,
) -> Result<Option<String>, Error> {
    session.get(id)?.repo.pr_url(&branch)
}

fn validate_external_url(url: &str) -> Result<&str, Error> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(Error::Refused(
            "only http and https URLs can be opened".into(),
        ));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '"' | '\'' | '`'))
    {
        return Err(Error::Refused("URL contains invalid characters".into()));
    }
    Ok(trimmed)
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), Error> {
    let valid = validate_external_url(&url)?;
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", valid]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(valid);
        c
    };
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(valid);
        c
    };
    cmd.spawn()
        .map(drop)
        .map_err(|e| Error::Read(format!("cannot open browser: {e}")))
}

#[tauri::command]
async fn undo(session: State<'_, Session>, id: u32) -> Result<Reverted, Error> {
    session.undo(id)
}

#[tauri::command]
async fn redo(session: State<'_, Session>, id: u32) -> Result<Reverted, Error> {
    session.redo(id)
}

#[tauri::command]
async fn library_load(library: State<'_, LibraryStore>) -> Result<LibrarySnapshot, Error> {
    library.snapshot()
}

#[tauri::command]
async fn library_summaries(
    library: State<'_, LibraryStore>,
    paths: Vec<String>,
) -> Result<Vec<RepoSummary>, Error> {
    library.refresh_summaries(&paths)
}

#[tauri::command]
async fn library_add_repos(
    library: State<'_, LibraryStore>,
    entries: Vec<AddRepoEntry>,
) -> Result<LibrarySnapshot, Error> {
    library.add_repos(&entries)
}

#[tauri::command]
async fn library_remove_repo(
    library: State<'_, LibraryStore>,
    id: String,
) -> Result<LibrarySnapshot, Error> {
    library.remove_repo(&id)
}

#[tauri::command]
async fn library_locate_repo(
    library: State<'_, LibraryStore>,
    id: String,
    new_path: String,
) -> Result<LibrarySnapshot, Error> {
    library.locate_repo(&id, &new_path)
}

#[tauri::command]
async fn library_move_repo(
    library: State<'_, LibraryStore>,
    id: String,
    folder_id: Option<String>,
    order: usize,
) -> Result<LibrarySnapshot, Error> {
    library.move_repo(&id, folder_id, order)
}

#[tauri::command]
async fn library_pin_repo(
    library: State<'_, LibraryStore>,
    id: String,
    pinned: bool,
) -> Result<LibrarySnapshot, Error> {
    library.pin_repo(&id, pinned)
}

#[tauri::command]
async fn library_create_folder(
    library: State<'_, LibraryStore>,
    name: String,
    parent_id: Option<String>,
) -> Result<LibrarySnapshot, Error> {
    library.create_folder(&name, parent_id)
}

#[tauri::command]
async fn library_rename_folder(
    library: State<'_, LibraryStore>,
    id: String,
    name: String,
) -> Result<LibrarySnapshot, Error> {
    library.rename_folder(&id, &name)
}

#[tauri::command]
async fn library_delete_folder(
    library: State<'_, LibraryStore>,
    id: String,
) -> Result<LibrarySnapshot, Error> {
    library.delete_folder(&id)
}

#[tauri::command]
async fn library_move_folder(
    library: State<'_, LibraryStore>,
    id: String,
    parent_id: Option<String>,
    order: usize,
) -> Result<LibrarySnapshot, Error> {
    library.move_folder(&id, parent_id, order)
}

#[tauri::command]
async fn library_save_tabs(
    library: State<'_, LibraryStore>,
    tabs: OpenTabState,
) -> Result<LibrarySnapshot, Error> {
    library.save_tabs(tabs)
}

#[tauri::command]
async fn library_undo(library: State<'_, LibraryStore>) -> Result<LibrarySnapshot, Error> {
    library.undo()
}

#[tauri::command]
async fn library_redo(library: State<'_, LibraryStore>) -> Result<LibrarySnapshot, Error> {
    library.redo()
}

#[tauri::command]
async fn scan_folder(library: State<'_, LibraryStore>, root: String) -> Result<ScanResult, Error> {
    let existing = library.existing_paths()?;
    core_scan_folder(Path::new(&root), &existing, DEFAULT_SCAN_DEPTH)
}

const CLONE_PROGRESS: &str = "clone-progress";
const FETCH_MANY_PROGRESS: &str = "fetch-many-progress";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchManyProgress {
    done: usize,
    total: usize,
    path: String,
    error: Option<String>,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn clone_repo<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    library: State<'_, LibraryStore>,
    active: State<'_, ActiveClone>,
    url: String,
    dest: String,
    folder_path: Option<String>,
    long_paths: bool,
) -> Result<String, Error> {
    let cancel = {
        let mut g = active
            .0
            .lock()
            .map_err(|_| Error::Read("clone lock poisoned".into()))?;
        if g.is_some() {
            return Err(Error::Refused("a clone is already in progress".into()));
        }
        let c = Cancel::default();
        *g = Some(c.clone());
        c
    };
    let _clear = ClearClone(&active.0);
    let git = session.git()?;
    let cloned = core_clone_repo(
        git,
        &url,
        Path::new(&dest),
        long_paths,
        &cancel,
        &mut |p: CloneProgress| {
            if let Err(e) = app.emit(CLONE_PROGRESS, p) {
                eprintln!("gitgud: cannot emit {CLONE_PROGRESS}: {e}");
            }
        },
    )?;
    let path_str = display_path(&cloned);
    let _ = library.add_repos(&[AddRepoEntry {
        path: path_str.clone(),
        folder_path,
        name: None,
    }]);
    let _ = library.refresh_summaries(std::slice::from_ref(&path_str));
    Ok(path_str)
}

#[tauri::command]
async fn cancel_clone(active: State<'_, ActiveClone>) -> Result<bool, Error> {
    let guard = active
        .0
        .lock()
        .map_err(|_| Error::Read("clone lock poisoned".into()))?;
    if let Some(cancel) = guard.as_ref() {
        cancel.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn fetch_many<R: Runtime>(
    app: AppHandle<R>,
    session: State<'_, Session>,
    library: State<'_, LibraryStore>,
    paths: Vec<String>,
) -> Result<Vec<RepoSummary>, Error> {
    let total = paths.len();
    if total == 0 {
        return Ok(Vec::new());
    }
    let git = session.git()?;
    let done = AtomicUsize::new(0);
    let next = AtomicUsize::new(0);
    let workers = total.clamp(1, 4);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let idx = next.fetch_add(1, Ordering::Relaxed);
                    if idx >= total {
                        break;
                    }
                    let path = &paths[idx];
                    let err = match repo::open(path) {
                        Ok(r) => git
                            .run(r.root(), &["fetch", "--all", "--prune"], None)
                            .err()
                            .map(|e| e.to_string()),
                        Err(e) => Some(e.to_string()),
                    };
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if let Err(e) = app.emit(
                        FETCH_MANY_PROGRESS,
                        FetchManyProgress {
                            done: n,
                            total,
                            path: path.clone(),
                            error: err,
                        },
                    ) {
                        eprintln!("gitgud: cannot emit {FETCH_MANY_PROGRESS}: {e}");
                    }
                }
            });
        }
    });
    library.refresh_summaries(&paths)
}

#[tauri::command]
async fn unity_info(session: State<'_, Session>, id: u32) -> Result<UnityInfo, Error> {
    session.unity_info(id)
}

#[tauri::command]
async fn unity_configure_smart_merge(
    session: State<'_, Session>,
    id: u32,
    tool_path: Option<String>,
) -> Result<UnityInfo, Error> {
    session.unity_configure_smart_merge(id, tool_path.as_deref())
}

#[tauri::command]
async fn unity_meta_issues(session: State<'_, Session>, id: u32) -> Result<Vec<MetaIssue>, Error> {
    session.unity_meta_issues(id)
}

#[tauri::command]
async fn unity_remerge_conflicts(session: State<'_, Session>, id: u32) -> Result<Opened, Error> {
    session.unity_remerge_conflicts(id)
}

#[tauri::command]
async fn unity_confirm_verify(
    session: State<'_, Session>,
    id: u32,
    path: String,
    file_id: Option<String>,
    property: Option<String>,
    confirmed: bool,
) -> Result<Opened, Error> {
    session.unity_confirm_verify(
        id,
        &path,
        file_id.as_deref(),
        property.as_deref(),
        confirmed,
    )
}

#[tauri::command]
async fn unity_check_open_assets(
    session: State<'_, Session>,
    id: u32,
    range: String,
) -> Result<bool, Error> {
    session.unity_check_open_assets(id, &range)
}

#[tauri::command]
async fn lfs_info(session: State<'_, Session>, id: u32) -> Result<LfsInfo, Error> {
    session.lfs_info(id)
}

#[tauri::command]
async fn lfs_locks(
    session: State<'_, Session>,
    library: State<'_, LibraryStore>,
    id: u32,
) -> Result<Vec<LfsLock>, Error> {
    let locks = session.lfs_locks(id)?;
    if let Ok(open) = session.get(id) {
        let ours_count = locks.iter().filter(|l| l.ours).count();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let _ =
            library.update_cached_locks(&open.repo.root().to_string_lossy(), ours_count, Some(now));
    }
    Ok(locks)
}

#[tauri::command]
async fn lfs_lock(session: State<'_, Session>, id: u32, path: String) -> Result<(), Error> {
    session.lfs_lock(id, &path)
}

#[tauri::command]
async fn lfs_unlock(
    session: State<'_, Session>,
    id: u32,
    path: String,
    force: Option<bool>,
) -> Result<(), Error> {
    session.lfs_unlock(id, &path, force.unwrap_or(false))
}

#[tauri::command]
async fn lfs_locks_in_push(
    session: State<'_, Session>,
    id: u32,
    remote: String,
    remote_branch: String,
    branch: String,
) -> Result<Vec<LfsLock>, Error> {
    session.lfs_locks_in_push(id, &remote, &remote_branch, &branch)
}

#[tauri::command]
async fn tags(session: State<'_, Session>, id: u32) -> Result<Vec<TagInfo>, Error> {
    session.tags(id)
}

#[tauri::command]
async fn create_tag(
    session: State<'_, Session>,
    id: u32,
    name: String,
    target: Option<String>,
    message: Option<String>,
) -> Result<Opened, Error> {
    session.create_tag(id, &name, target.as_deref(), message.as_deref())
}

#[tauri::command]
async fn delete_tag(session: State<'_, Session>, id: u32, name: String) -> Result<Opened, Error> {
    session.delete_tag(id, &name)
}

#[tauri::command]
async fn push_tag(
    session: State<'_, Session>,
    id: u32,
    remote: String,
    name: String,
) -> Result<(), Error> {
    session.push_tag(id, &remote, &name)
}

#[tauri::command]
async fn delete_remote_tag(
    session: State<'_, Session>,
    id: u32,
    remote: String,
    name: String,
) -> Result<(), Error> {
    session.delete_remote_tag(id, &remote, &name)
}

#[tauri::command]
async fn file_history(
    session: State<'_, Session>,
    id: u32,
    path: String,
    limit: Option<usize>,
) -> Result<Vec<FileCommit>, Error> {
    session.file_history(id, &path, limit)
}

#[tauri::command]
async fn blame_file(
    session: State<'_, Session>,
    id: u32,
    path: String,
    rev: Option<String>,
) -> Result<Vec<BlameLine>, Error> {
    session.blame_file(id, &path, rev.as_deref())
}

#[tauri::command]
async fn op_log_stats(session: State<'_, Session>, id: u32) -> Result<OpLogStats, Error> {
    session.op_log_stats(id)
}

#[tauri::command]
async fn op_log_prune(session: State<'_, Session>, id: u32) -> Result<OpLogStats, Error> {
    session.op_log_prune(id)
}

#[tauri::command]
async fn outside_reflog(
    session: State<'_, Session>,
    id: u32,
    limit: Option<usize>,
) -> Result<Vec<ReflogEntry>, Error> {
    session.outside_reflog(id, limit)
}

#[tauri::command]
async fn has_commit_graph(session: State<'_, Session>, id: u32) -> Result<bool, Error> {
    session.has_commit_graph(id)
}

#[tauri::command]
async fn write_commit_graph(session: State<'_, Session>, id: u32) -> Result<(), Error> {
    session.write_commit_graph(id)
}

#[tauri::command]
async fn cache_stats(session: State<'_, Session>, id: u32) -> Result<CacheStats, Error> {
    session.cache_stats(id)
}

#[tauri::command]
async fn clear_unity_caches(session: State<'_, Session>, id: u32) -> Result<CacheStats, Error> {
    session.clear_unity_caches(id)
}

#[tauri::command]
async fn clear_preview_cache(session: State<'_, Session>, id: u32) -> Result<CacheStats, Error> {
    session.clear_preview_cache(id)
}

#[tauri::command]
async fn warm_textures(
    session: State<'_, Session>,
    id: u32,
    version: gitgud_core::image::Version,
    paths: Vec<String>,
    max: u32,
    bc: bool,
) -> Result<(), Error> {
    session.warm_textures(id, version, paths, max, bc)
}

#[tauri::command]
async fn worktree_unity_diff(
    session: State<'_, Session>,
    id: u32,
    path: String,
    side: Side,
) -> Result<UnityYamlDiff, Error> {
    session.worktree_unity_diff(id, &path, side)
}

#[tauri::command]
async fn commit_unity_diff(
    session: State<'_, Session>,
    id: u32,
    commit: String,
    path: String,
) -> Result<UnityYamlDiff, Error> {
    session.commit_unity_diff(id, &commit, &path)
}

#[tauri::command]
async fn model_info(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<gitgud_core::model::ModelInfo, Error> {
    session.model_info(id, &path, &version)
}

#[tauri::command]
async fn clip_curves(
    session: State<'_, Session>,
    id: u32,
    path: String,
    before: Option<gitgud_core::image::Version>,
    after: gitgud_core::image::Version,
) -> Result<gitgud_core::model::ClipCurves, Error> {
    session.clip_curves(id, &path, before.as_ref(), &after)
}

#[tauri::command]
async fn clip_users(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<Vec<String>, Error> {
    session.clip_users(id, &path, &version)
}

#[tauri::command]
async fn sprite_sheet(
    session: State<'_, Session>,
    id: u32,
    texture: String,
    before: Option<gitgud_core::image::Version>,
    after: gitgud_core::image::Version,
) -> Result<gitgud_core::model::SpriteSheet, Error> {
    session.sprite_sheet(id, &texture, before.as_ref(), &after)
}

#[tauri::command]
async fn video_info(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<gitgud_core::video::VideoInfo, Error> {
    session.video_info(id, &path, &version)
}

#[tauri::command]
async fn data_tables(
    session: State<'_, Session>,
    id: u32,
    path: String,
    before: Option<gitgud_core::image::Version>,
    after: gitgud_core::image::Version,
) -> Result<gitgud_core::model::DataTables, Error> {
    session.data_tables(id, &path, before.as_ref(), &after)
}

#[tauri::command]
async fn audio_info(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<gitgud_core::audio::AudioInfo, Error> {
    session.audio_info(id, &path, &version)
}

#[tauri::command]
async fn material_view(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<gitgud_core::model::MaterialView, Error> {
    session.material_view(id, &path, &version)
}

#[tauri::command]
async fn material_users(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<Vec<String>, Error> {
    session.material_users(id, &path, &version)
}

#[tauri::command]
async fn font_info(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
    face: Option<u32>,
) -> Result<gitgud_core::font::FontInfo, Error> {
    session.font_info(id, &path, &version, face.unwrap_or(0))
}

#[tauri::command]
async fn unity_asset_info(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
) -> Result<gitgud_core::preview::UnityAssetInfo, Error> {
    session.unity_asset_info(id, &path, &version)
}

/// Writes `path` at `version` to a temp copy and opens it in its default app.
#[tauri::command]
async fn open_version(
    session: State<'_, Session>,
    id: u32,
    path: String,
    version: gitgud_core::image::Version,
    label: String,
) -> Result<String, Error> {
    let copy = session.open_copy(id, &path, &version, &label)?;
    open_in_default_app(&copy)?;
    Ok(copy.to_string_lossy().into_owned())
}

fn open_in_default_app(file: &std::path::Path) -> Result<(), Error> {
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(file)
        .spawn()
        .map(drop)
        .map_err(|e| Error::Refused(format!("cannot open {}: {e}", file.display())))
}

#[tauri::command]
async fn unity_conflict_table(
    session: State<'_, Session>,
    id: u32,
    path: String,
) -> Result<UnityConflictTable, Error> {
    session.unity_conflict_table(id, &path)
}

#[tauri::command]
async fn resolve_unity_conflict(
    session: State<'_, Session>,
    id: u32,
    path: String,
    picks: Vec<UnityPropertyPick>,
) -> Result<Opened, Error> {
    session.resolve_unity_conflict(id, &path, &picks)
}

#[tauri::command]
async fn unity_post_merge_check(
    session: State<'_, Session>,
    id: u32,
    paths: Vec<String>,
) -> Result<Vec<UnityIntegrityIssue>, Error> {
    session.unity_post_merge_check(id, &paths)
}

#[tauri::command]
async fn unity_repo_setup_status(
    session: State<'_, Session>,
    id: u32,
) -> Result<UnityRepoSetupStatus, Error> {
    session.unity_repo_setup_status(id)
}

#[tauri::command]
async fn unity_apply_repo_setup(
    session: State<'_, Session>,
    id: u32,
    write_gitignore: bool,
    write_gitattributes: bool,
) -> Result<UnityRepoSetupStatus, Error> {
    session.unity_apply_repo_setup(id, write_gitignore, write_gitattributes)
}

#[tauri::command]
async fn proposals_list(
    session: State<'_, Session>,
    id: u32,
) -> Result<Vec<ProposalSummary>, Error> {
    session.proposals_list(id)
}

#[tauri::command]
async fn proposal_file_diff(
    session: State<'_, Session>,
    id: u32,
    proposal_id: String,
    path: String,
) -> Result<FileDiff, Error> {
    session.proposal_file_diff(id, &proposal_id, &path)
}

#[tauri::command]
async fn proposal_approve(
    session: State<'_, Session>,
    id: u32,
    proposal_id: String,
    message: Option<String>,
    selected_paths: Vec<String>,
) -> Result<Opened, Error> {
    session.proposal_approve(id, &proposal_id, message.as_deref(), &selected_paths)
}

#[tauri::command]
async fn proposal_request_changes(
    session: State<'_, Session>,
    id: u32,
    proposal_id: String,
    comment: String,
    path: Option<String>,
) -> Result<ProposalSummary, Error> {
    session.proposal_request_changes(id, &proposal_id, &comment, path.as_deref())
}

#[tauri::command]
async fn proposal_reject(
    session: State<'_, Session>,
    id: u32,
    proposal_id: String,
    reason: Option<String>,
) -> Result<Opened, Error> {
    session.proposal_reject(id, &proposal_id, reason.as_deref())
}

#[tauri::command]
async fn branch_stacks(session: State<'_, Session>, id: u32) -> Result<Vec<BranchStack>, Error> {
    session.branch_stacks(id)
}

#[tauri::command]
async fn restack(
    session: State<'_, Session>,
    id: u32,
    top_branch: String,
    onto: String,
) -> Result<Opened, Error> {
    session.restack(id, &top_branch, &onto)
}

#[tauri::command]
async fn pr_url_with_base(
    session: State<'_, Session>,
    id: u32,
    branch: String,
    base: Option<String>,
) -> Result<Option<String>, Error> {
    session.pr_url_with_base(id, &branch, base.as_deref())
}

#[tauri::command]
async fn lfs_cache_stats(session: State<'_, Session>, id: u32) -> Result<LfsCacheStats, Error> {
    session.lfs_cache_stats(id)
}

#[tauri::command]
async fn lfs_prune(session: State<'_, Session>, id: u32) -> Result<LfsCacheStats, Error> {
    session.lfs_prune(id)
}

#[tauri::command]
async fn lfs_fetch_object(
    session: State<'_, Session>,
    id: u32,
    oid: String,
    size: u64,
    path: Option<String>,
) -> Result<bool, Error> {
    session.lfs_fetch_object(id, &oid, size, path.as_deref())
}

#[tauri::command]
async fn ai_providers(session: State<'_, Session>) -> Result<AiProviderStatus, Error> {
    Ok(session.ai_providers())
}

#[tauri::command]
async fn ai_draft_commit_message(
    session: State<'_, Session>,
    id: u32,
    provider: Option<String>,
) -> Result<String, Error> {
    session.ai_draft_commit_message(id, provider.as_deref())
}

#[tauri::command]
async fn agent_setup(
    session: State<'_, Session>,
    id: u32,
    target: String,
    with_hooks: Option<bool>,
) -> Result<String, Error> {
    session.agent_setup(id, &target, with_hooks.unwrap_or(false))
}

#[tauri::command]
async fn stale_origin_commits(
    session: State<'_, Session>,
    id: u32,
    upstream: String,
) -> Result<Vec<String>, Error> {
    session.stale_origin_commits(id, &upstream)
}

#[tauri::command]
async fn refresh_index(session: State<'_, Session>, id: u32) -> Result<Opened, Error> {
    session.refresh_index(id)
}

#[tauri::command]
async fn recent_timings(session: State<'_, Session>) -> Result<Vec<TimingSample>, Error> {
    Ok(session.recent_timings())
}

#[tauri::command]
async fn worktrees_list(session: State<'_, Session>, id: u32) -> Result<Vec<WorktreeInfo>, Error> {
    session.worktrees_list(id)
}

#[tauri::command]
async fn propose_from_worktree(
    session: State<'_, Session>,
    id: u32,
    worktree_path: String,
    message: String,
    why: Option<String>,
    agent: Option<String>,
) -> Result<ProposalSummary, Error> {
    session.propose_from_worktree(
        id,
        Path::new(&worktree_path),
        &message,
        why.as_deref(),
        agent.as_deref(),
    )
}

#[tauri::command]
async fn background_refresh(
    session: State<'_, Session>,
    id: u32,
    min_age_secs: Option<u64>,
) -> Result<bool, Error> {
    session.background_refresh(id, min_age_secs.unwrap_or(600))
}

fn app_with_library<R: Runtime>(builder: Builder<R>, library: Option<LibraryStore>) -> Builder<R> {
    let builder = if let Some(store) = library {
        builder.manage(store)
    } else {
        builder
    };
    builder
        .manage(Session::default())
        .manage(ActiveClone::default())
        .register_asynchronous_uri_scheme_protocol("gitgud", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let uri = request.uri().to_string();
            std::thread::spawn(move || {
                responder.respond(images::respond(&app.state::<Session>(), &uri));
            });
        })
        .invoke_handler(tauri::generate_handler![
            git_info,
            launch_path,
            open_repo,
            close_repo,
            repo_status,
            graph_rows,
            graph_find,
            commit_detail,
            commit_file_diff,
            worktree_file_diff,
            stage_hunk,
            unstage_hunk,
            discard,
            discard_hunk,
            refresh_repo,
            stage,
            unstage,
            commit,
            head_commit_message,
            reset,
            undo,
            redo,
            fetch,
            cancel_fetch,
            cancel_task,
            pull,
            push_info,
            push,
            delete_remote_branch,
            op_log,
            restore_to,
            restore_files,
            branches,
            checkout,
            create_branch,
            rename_branch,
            delete_branch,
            set_upstream,
            checkout_remote,
            predict_merge,
            merge,
            merge_state,
            conflict_text,
            conflict_sides,
            resolve_with,
            resolve_side,
            merge_abort,
            merge_commit,
            predict_rebase,
            rebase,
            rebase_state,
            rebase_step,
            predict_cherry_pick,
            predict_revert,
            cherry_pick,
            revert_commit,
            sequencer_state,
            sequencer_step,
            stashes,
            stash_detail,
            stash_file_diff,
            stash_save,
            stash_apply,
            stash_pop,
            stash_drop,
            pr_url,
            open_external_url,
            history_plan,
            plan_preview,
            preview_rows,
            predicted_file,
            run_plan,
            reword_head,
            quick_edit,
            library_load,
            library_summaries,
            library_add_repos,
            library_remove_repo,
            library_locate_repo,
            library_move_repo,
            library_pin_repo,
            library_create_folder,
            library_rename_folder,
            library_delete_folder,
            library_move_folder,
            library_save_tabs,
            library_undo,
            library_redo,
            scan_folder,
            clone_repo,
            cancel_clone,
            fetch_many,
            stage_lines,
            unstage_lines,
            discard_lines,
            config_get_identity,
            config_set_identity,
            tool_diagnostics,
            unity_info,
            unity_configure_smart_merge,
            unity_meta_issues,
            unity_remerge_conflicts,
            unity_confirm_verify,
            unity_check_open_assets,
            lfs_info,
            lfs_locks,
            lfs_lock,
            lfs_unlock,
            lfs_locks_in_push,
            background_refresh,
            tags,
            create_tag,
            delete_tag,
            push_tag,
            delete_remote_tag,
            file_history,
            blame_file,
            op_log_stats,
            op_log_prune,
            outside_reflog,
            has_commit_graph,
            write_commit_graph,
            cache_stats,
            clear_unity_caches,
            clear_preview_cache,
            warm_textures,
            worktree_unity_diff,
            commit_unity_diff,
            model_info,
            unity_asset_info,
            open_version,
            font_info,
            material_view,
            material_users,
            audio_info,
            data_tables,
            video_info,
            sprite_sheet,
            clip_curves,
            clip_users,
            unity_conflict_table,
            resolve_unity_conflict,
            unity_post_merge_check,
            unity_repo_setup_status,
            unity_apply_repo_setup,
            proposals_list,
            proposal_file_diff,
            proposal_approve,
            proposal_request_changes,
            proposal_reject,
            branch_stacks,
            restack,
            pr_url_with_base,
            lfs_cache_stats,
            lfs_prune,
            lfs_fetch_object,
            ai_providers,
            ai_draft_commit_message,
            agent_setup,
            stale_origin_commits,
            refresh_index,
            recent_timings,
            worktrees_list,
            propose_from_worktree
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_with_library(Builder::default(), None)
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let library_path = if let Some(custom) = std::env::var_os("GITGUD_LIBRARY") {
                std::path::PathBuf::from(custom)
            } else {
                app.path().app_data_dir()?.join("library.json")
            };
            if let Some(parent) = library_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let store = LibraryStore::load(library_path)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            app.manage(store);
            std::thread::spawn(gitgud_core::preview::clean_stale_open_copies);

            #[cfg(windows)]
            if let Some(window) = app.get_webview_window("main")
                && let Err(e) = window.with_webview(|wv| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                    use windows_core::Interface;
                    if let Ok(core) = wv.controller().CoreWebView2()
                        && let Ok(settings) = core.Settings()
                        && let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>()
                    {
                        let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
                    }
                })
            {
                eprintln!("gitgud: cannot disable browser accelerator keys: {e}");
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                gitgud_core::preview::clean_open_copies();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgud_core::testing::TestRepo;
    use serde_json::{Value, json};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::{InvokeRequest, WebviewWindow};

    /// Invokes `cmd` through the real IPC path, with JSON args as the webview sends them.
    struct Ipc {
        _library_dir: tempfile::TempDir,
        app: tauri::App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
    }

    impl Ipc {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let store = LibraryStore::load(dir.path().join("library.json")).unwrap();
            let app = app_with_library(mock_builder(), Some(store))
                .build(mock_context(noop_assets()))
                .unwrap();
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            Self {
                _library_dir: dir,
                app,
                webview,
            }
        }

        fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.into(),
            };
            get_ipc_response(&self.webview, request).map(|body| body.deserialize().unwrap())
        }
    }

    fn history() -> TestRepo {
        let repo = TestRepo::init();
        repo.write("a.txt", "a").commit_all("first");
        repo.write("a.txt", "b").commit_all("second");
        repo.git(&["tag", "v1"]);
        repo
    }

    fn open(ipc: &Ipc, repo: &TestRepo) -> Value {
        ipc.call("open_repo", json!({ "path": repo.path() }))
            .unwrap()
    }

    #[test]
    fn model_info_reads_the_fbx_material_remaps() {
        let repo = TestRepo::init();
        repo.write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.1f1\n")
            .write("Assets/boat.fbx", "fbx")
            .write(
                "Assets/boat.fbx.meta",
                "guid: 4444dddd000000000000000000000000\nModelImporter:\n  externalObjects:\n  - first:\n      type: UnityEngine:Material\n      assembly: UnityEngine.CoreModule\n      name: HullMat\n    second: {fileID: 2100000, guid: 3333cccc000000000000000000000000, type: 2}\n",
            )
            .write(
                "Assets/hull.mat",
                "--- !u!21 &2100000\nMaterial:\n  m_Name: Hull\n  m_Shader: {fileID: 46, guid: 0000000000000000f000000000000000, type: 0}\n",
            )
            .write("Assets/hull.mat.meta", "guid: 3333cccc000000000000000000000000\n")
            .commit_all("one");
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        for version in [json!("worktree"), json!({ "commit": head })] {
            let info = ipc
                .call(
                    "model_info",
                    json!({ "id": id, "path": "Assets/boat.fbx", "version": version }),
                )
                .unwrap();
            assert_eq!(info["kind"], "fbx");
            assert_eq!(info["remaps"]["HullMat"], 0);
            assert_eq!(info["materials"][0]["shader"], "Standard");
            assert_eq!(info["materials"][0]["baseMap"], Value::Null);
            assert!(info["key"].is_string());
        }
        let asset = ipc
            .call(
                "unity_asset_info",
                json!({ "id": id, "path": "Assets/hull.mat", "version": "head" }),
            )
            .unwrap();
        assert_eq!(asset["binary"], false);
        assert_eq!(asset["classId"], 21);
        let stats = ipc.call("cache_stats", json!({ "id": id })).unwrap();
        assert_eq!(stats["modelInfos"], 2);
        assert!(
            stats["guidIndexFile"]
                .as_str()
                .unwrap()
                .ends_with("guid-index")
        );
        let cleared = ipc.call("clear_unity_caches", json!({ "id": id })).unwrap();
        assert_eq!(cleared["modelInfos"], 0);
        assert_eq!(cleared["guidIndexBytes"], 0);
        let previews = ipc
            .call("clear_preview_cache", json!({ "id": id }))
            .unwrap();
        ipc.call(
            "warm_textures",
            json!({ "id": id, "version": "head", "paths": ["Assets/none.png"], "max": 1024, "bc": true }),
        )
        .unwrap();
        assert_eq!(previews["previewModels"], 0);
        assert_eq!(previews["previewBytes"], 0);
        assert!(
            previews["previewCachePath"]
                .as_str()
                .unwrap()
                .ends_with("cache")
        );
        let err = ipc
            .call(
                "model_info",
                json!({ "id": id, "path": "Assets/hull.mat", "version": "head" }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("not a 3D model"));
        let view = ipc
            .call(
                "material_view",
                json!({ "id": id, "path": "Assets/hull.mat", "version": "worktree" }),
            )
            .unwrap();
        assert_eq!(view["material"]["shader"], "Standard");
        assert_eq!(view["renderQueue"], Value::Null);
        let users = ipc
            .call(
                "material_users",
                json!({ "id": id, "path": "Assets/hull.mat", "version": "head" }),
            )
            .unwrap();
        assert_eq!(users, json!(["Assets/boat.fbx"]));
    }

    #[test]
    fn clip_curves_compares_keys_and_finds_users() {
        let anim = |y: &str| {
            format!(
                "--- !u!74 &7400000\nAnimationClip:\n  m_Name: Bob\n  m_EditorCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: {y}\n        inSlope: 0\n        outSlope: 0\n    attribute: m_LocalPosition.y\n    path: Hull\n"
            )
        };
        let repo = TestRepo::init();
        repo.write("Bob.anim", &anim("0"))
            .write("Bob.anim.meta", "guid: a0000000000000000000000000000001\n")
            .commit_all("one");
        repo.write("Bob.anim", &anim("0.5"));
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let c = ipc
            .call(
                "clip_curves",
                json!({ "id": id, "path": "Bob.anim", "before": "head", "after": "worktree" }),
            )
            .unwrap();
        assert_eq!(c["tracks"][0]["label"], "Position.y");
        assert_eq!(c["tracks"][0]["state"], "changed");
        assert_eq!(c["tracks"][0]["after"][0]["value"], 0.5);
        let users = ipc
            .call(
                "clip_users",
                json!({ "id": id, "path": "Bob.anim", "version": "worktree" }),
            )
            .unwrap();
        assert_eq!(users, json!([]));
    }

    #[test]
    fn sprite_sheet_diffs_a_resliced_texture() {
        let meta = |w: u32| {
            format!(
                "TextureImporter:\n  spriteMode: 2\n  spriteSheet:\n    sprites:\n    - name: icon\n      rect: {{x: 0, y: 0, width: {w}, height: 32}}\n      pivot: {{x: 0.5, y: 0.5}}\n      border: {{x: 0, y: 0, z: 0, w: 0}}\n"
            )
        };
        let repo = TestRepo::init();
        repo.write("HUD.png", "png")
            .write("HUD.png.meta", &meta(32))
            .commit_all("one");
        repo.write("HUD.png.meta", &meta(40));
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let s = ipc
            .call(
                "sprite_sheet",
                json!({ "id": id, "texture": "HUD.png", "before": "head", "after": "worktree" }),
            )
            .unwrap();
        assert_eq!(s["mode"], "multiple");
        assert_eq!(s["after"][0]["rect"], json!([0.0, 0.0, 40.0, 32.0]));
        assert_eq!(s["changes"][0]["kinds"], json!(["resized"]));
    }

    #[test]
    fn video_info_reads_an_mp4() {
        let repo = TestRepo::init();
        std::fs::write(
            repo.path().join("intro.mp4"),
            gitgud_core::video::tiny_mp4(b"avc1", 1280, 720, 25, 4),
        )
        .unwrap();
        repo.commit_all("one");
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let v = ipc
            .call(
                "video_info",
                json!({ "id": id, "path": "intro.mp4", "version": "head" }),
            )
            .unwrap();
        assert_eq!(v["codec"], "H.264");
        assert_eq!(
            (v["width"].clone(), v["height"].clone()),
            (json!(1280), json!(720))
        );
        assert_eq!(v["fps"], 25.0);
    }

    #[test]
    fn data_tables_compares_rows_by_key() {
        let repo = TestRepo::init();
        repo.write("stats.csv", "id,speed\nskiff,12\ntug,7\n")
            .commit_all("one");
        repo.write("stats.csv", "id,speed\ntug,7\nskiff,13\n");
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let d = ipc
            .call(
                "data_tables",
                json!({ "id": id, "path": "stats.csv", "before": "head", "after": "worktree" }),
            )
            .unwrap();
        assert_eq!(d["kind"], "CSV");
        let t = &d["tables"][0];
        assert_eq!(t["key"], "id");
        assert_eq!(t["changed"], 1);
        assert_eq!(t["rows"][1]["state"], "changed");
        assert_eq!(t["rows"][1]["cells"][1], json!(["12", "13"]));
        let alone = ipc
            .call(
                "data_tables",
                json!({ "id": id, "path": "stats.csv", "after": "head" }),
            )
            .unwrap();
        assert_eq!(alone["tables"][0]["rows"][0]["state"], "same");
    }

    #[test]
    fn audio_info_reads_rate_length_and_loudness() {
        let repo = TestRepo::init();
        std::fs::write(
            repo.path().join("horn.wav"),
            gitgud_core::audio::sine_wav(48_000, 2, 1.0, 0.5),
        )
        .unwrap();
        repo.commit_all("one");
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let info = ipc
            .call(
                "audio_info",
                json!({ "id": id, "path": "horn.wav", "version": "head" }),
            )
            .unwrap();
        assert_eq!(info["sampleRate"], 48_000);
        assert_eq!(info["channels"], 2);
        assert_eq!(info["frames"], 48_000);
        assert!(info["loudnessLufs"].is_number());
        assert_eq!(info["peaks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn font_info_reads_each_version_and_refuses_what_isnt_a_font() {
        use gitgud_core::font::fixture::tiny_ttf;
        let repo = TestRepo::init();
        std::fs::write(
            repo.path().join("ui.ttf"),
            tiny_ttf(&[(0, 0), (250, 500), (500, 0)]),
        )
        .unwrap();
        repo.write("notes.ttf", "plain text").commit_all("one");
        std::fs::write(
            repo.path().join("ui.ttf"),
            tiny_ttf(&[(0, 0), (250, 600), (500, 0)]),
        )
        .unwrap();
        let ipc = Ipc::new();
        let id = open(&ipc, &repo)["id"].clone();
        let read = |version: Value| {
            ipc.call(
                "font_info",
                json!({ "id": id, "path": "ui.ttf", "version": version }),
            )
            .unwrap()
        };
        let (head, disk) = (read(json!("head")), read(json!("worktree")));
        assert_eq!(head["codepoints"], json!([0x41, 0x42]));
        assert_eq!(head["unitsPerEm"], 1000);
        assert_eq!(head["outlines"][0], disk["outlines"][0]);
        assert_ne!(head["outlines"][1], disk["outlines"][1]);
        let err = ipc
            .call(
                "font_info",
                json!({ "id": id, "path": "notes.ttf", "version": "head" }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("notes.ttf: not a font"));
    }

    #[test]
    fn git_info_returns_path_and_dotted_version() {
        let info = Ipc::new().call("git_info", json!({})).unwrap();
        assert!(info["path"].as_str().unwrap().contains("git"));
        assert_eq!(info["version"].as_str().unwrap().split('.').count(), 3);
    }

    #[test]
    fn open_repo_returns_id_root_and_row_count() {
        let ipc = Ipc::new();
        let opened = open(&ipc, &history());
        assert_eq!(opened.as_object().unwrap().len(), 4);
        assert!(opened["id"].is_u64());
        assert!(opened["root"].is_string());
        assert_eq!(opened["rows"], 2);
        assert_eq!(opened["graphVersion"], 0);
    }

    #[test]
    fn stage_unstage_and_commit_return_the_refreshed_repo() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "changed").write("new file.txt", "n");
        let id = open(&ipc, &repo)["id"].clone();
        let status = || ipc.call("repo_status", json!({ "id": id })).unwrap()["entries"].clone();

        let staged = ipc
            .call(
                "stage",
                json!({ "id": id, "paths": ["a.txt", "new file.txt"] }),
            )
            .unwrap();
        assert_eq!(staged["graphVersion"], 0, "staging leaves history alone");
        assert_eq!(status()[1]["index"], "added");

        ipc.call("unstage", json!({ "id": id, "paths": ["new file.txt"] }))
            .unwrap();
        assert_eq!(status()[1]["kind"], "untracked");

        let committed = ipc
            .call("commit", json!({ "id": id, "message": "via ipc" }))
            .unwrap();
        assert_eq!(committed["graphVersion"], 1);
        assert_eq!(
            committed["rows"], 4,
            "3 commits + the WIP row for new file.txt"
        );
        let rows = ipc
            .call("graph_rows", json!({ "id": id, "start": 0, "count": 2 }))
            .unwrap();
        assert_eq!(rows[0]["wip"], true);
        assert_eq!(rows[1]["subject"], "via ipc");
    }

    #[test]
    fn graph_find_returns_matching_rows() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();
        assert_eq!(
            ipc.call("graph_find", json!({ "id": id, "query": "FIRST" }))
                .unwrap(),
            json!([1])
        );
        assert_eq!(
            ipc.call("graph_find", json!({ "id": id, "query": "v1" }))
                .unwrap(),
            json!([0])
        );
    }

    #[test]
    fn a_failed_commit_returns_gits_message() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();
        let err = ipc
            .call("commit", json!({ "id": id, "message": "nothing staged" }))
            .unwrap_err();
        assert!(err.as_str().unwrap().contains("nothing to commit"), "{err}");
    }

    #[test]
    fn refresh_repo_picks_up_outside_commits() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();
        repo.write("a.txt", "outside").commit_all("outside");

        let refreshed = ipc.call("refresh_repo", json!({ "id": id })).unwrap();
        assert_eq!(refreshed["rows"], 3);
        assert_eq!(refreshed["graphVersion"], 1);
    }

    #[test]
    fn commit_detail_and_file_diff_shapes() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

        let detail = ipc
            .call("commit_detail", json!({ "id": id, "commit": head }))
            .unwrap();
        assert_eq!(detail["subject"], "second");
        assert_eq!(detail["body"], "");
        assert_eq!(detail["author"]["email"], "tests@gitgud.invalid");
        assert!(detail["author"]["time"].is_i64());
        assert_eq!(detail["parents"].as_array().unwrap().len(), 1);
        assert_eq!(
            detail["files"],
            json!([{ "path": "a.txt", "change": "modified" }])
        );
        assert_eq!(detail["shortId"], &head[..7]);

        let diff = ipc
            .call(
                "commit_file_diff",
                json!({ "id": id, "commit": head, "path": "a.txt" }),
            )
            .unwrap();
        assert_eq!(
            diff,
            json!({
                "path": "a.txt",
                "binary": false,
                "hunks": [{
                    "oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1,
                    "lines": [
                        { "kind": "removed", "old": 1, "new": null, "text": "a" },
                        { "kind": "added", "old": null, "new": 1, "text": "b" },
                    ],
                }],
            })
        );
    }

    #[test]
    fn worktree_diff_and_hunk_staging_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        let lines: String = (1..=30).map(|i| format!("l{i}\n")).collect();
        repo.write("f.txt", &lines).commit_all("init");
        repo.write(
            "f.txt",
            &lines.replace("l2\n", "two\n").replace("l28\n", "x\n"),
        );
        let id = open(&ipc, &repo)["id"].clone();

        let diff = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "unstaged" }),
            )
            .unwrap();
        assert_eq!(diff["hunks"].as_array().unwrap().len(), 2);
        let first = &diff["hunks"][0];
        let hunk = json!({
            "oldStart": first["oldStart"], "oldLines": first["oldLines"],
            "newStart": first["newStart"], "newLines": first["newLines"],
        });

        ipc.call(
            "stage_hunk",
            json!({ "id": id, "path": "f.txt", "hunk": hunk }),
        )
        .unwrap();
        let staged = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "staged" }),
            )
            .unwrap();
        assert_eq!(staged["hunks"].as_array().unwrap().len(), 1);

        ipc.call(
            "unstage_hunk",
            json!({ "id": id, "path": "f.txt", "hunk": hunk }),
        )
        .unwrap();
        assert_eq!(repo.git(&["diff", "--cached", "--name-only"]).trim(), "");

        let stale = ipc
            .call(
                "stage_hunk",
                json!({ "id": id, "path": "f.txt", "hunk": { "oldStart": 9, "oldLines": 9, "newStart": 9, "newLines": 9 } }),
            )
            .unwrap_err();
        assert_eq!(
            stale,
            "f.txt changed since its diff was shown; look again and retry"
        );
    }

    #[test]
    fn undo_and_redo_name_the_action() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "changed").write("b.txt", "new");
        let id = open(&ipc, &repo)["id"].clone();
        ipc.call("stage", json!({ "id": id, "paths": ["a.txt", "b.txt"] }))
            .unwrap();
        ipc.call(
            "commit",
            json!({ "id": id, "message": "Subject here\n\nbody" }),
        )
        .unwrap();

        let undone = ipc.call("undo", json!({ "id": id })).unwrap();
        assert_eq!(undone["action"], "Commit \"Subject here\"");
        assert!(undone["opened"]["graphVersion"].is_u64());
        assert_eq!(
            ipc.call("undo", json!({ "id": id })).unwrap()["action"],
            "Stage 2 files"
        );
        assert_eq!(
            ipc.call("undo", json!({ "id": id })).unwrap_err(),
            "nothing to undo"
        );
        assert_eq!(
            ipc.call("redo", json!({ "id": id })).unwrap()["action"],
            "Stage 2 files"
        );
    }

    #[test]
    fn fetch_through_ipc_moves_remote_refs_and_reports_progress() {
        use std::sync::{Arc, Mutex};
        use tauri::Listener;

        let ipc = Ipc::new();
        let local = history();
        let origin = tempfile::tempdir().unwrap();
        let bare = origin.path().join("o.git");
        local.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
        local.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        ipc.app.listen_any(FETCH_PROGRESS, move |e| {
            sink.lock().unwrap().push(e.payload().to_owned());
        });
        let id = open(&ipc, &local)["id"].clone();
        assert_eq!(
            ipc.call("cancel_fetch", json!({ "id": id })).unwrap(),
            false
        );

        let opened = ipc.call("fetch", json!({ "id": id })).unwrap();

        assert_eq!(opened["graphVersion"], 1, "origin/main appeared");
        assert!(local.git_ok(&["rev-parse", "--verify", "-q", "origin/main"]));
        for payload in events.lock().unwrap().iter() {
            let v: Value = serde_json::from_str(payload).unwrap();
            assert_eq!(v["id"], id);
            assert!(v["progress"]["percent"].as_u64().unwrap() <= 100);
        }
    }

    #[test]
    fn discard_through_ipc_is_undoable() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "precious")
            .write("scratch.txt", "draft");
        let id = open(&ipc, &repo)["id"].clone();

        ipc.call(
            "discard",
            json!({ "id": id, "paths": ["a.txt", "scratch.txt"] }),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "b"
        );
        assert!(!repo.path().join("scratch.txt").exists());

        let undone = ipc.call("undo", json!({ "id": id })).unwrap();
        assert_eq!(undone["action"], "Discard 2 files");
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "precious"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("scratch.txt")).unwrap(),
            "draft"
        );
    }

    #[test]
    fn op_log_and_restores_through_ipc() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "keep");
        let id = open(&ipc, &repo)["id"].clone();
        ipc.call("discard", json!({ "id": id, "paths": ["a.txt"] }))
            .unwrap();

        let log = ipc.call("op_log", json!({ "id": id })).unwrap();
        let entry = &log[0];
        assert_eq!(entry["action"], "Discard a.txt");
        assert_eq!(entry["undone"], false);
        assert_eq!(entry["paths"], json!(["a.txt"]));
        assert!(entry["time"].is_i64());
        assert_eq!(entry.as_object().unwrap().len(), 5);

        ipc.call("restore_files", json!({ "id": id, "entry": entry["id"] }))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "keep"
        );

        let restored = ipc
            .call("restore_to", json!({ "id": id, "entry": entry["id"] }))
            .unwrap();
        assert_eq!(restored["action"], "Discard a.txt");
    }

    #[test]
    fn branches_and_checkout_through_ipc() {
        let ipc = Ipc::new();
        let repo = history();
        repo.git(&["branch", "feature/x", "HEAD~1"]);
        let id = open(&ipc, &repo)["id"].clone();

        let branches = ipc.call("branches", json!({ "id": id })).unwrap();
        assert_eq!(
            branches["local"],
            json!([
                { "name": "feature/x", "current": false, "upstream": null, "ahead": 0, "behind": 0 },
                { "name": "main", "current": true, "upstream": null, "ahead": 0, "behind": 0 },
            ])
        );
        assert_eq!(branches["remotes"], json!([]));

        ipc.call("checkout", json!({ "id": id, "branch": "feature/x" }))
            .unwrap();
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "feature/x");
        let undone = ipc.call("undo", json!({ "id": id })).unwrap();
        assert_eq!(undone["action"], "Check out feature/x");
    }

    #[test]
    fn merge_with_conflicts_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("a.txt", "1\n2\n3\n").commit_all("base");
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.write("a.txt", "1\ntheirs\n3\n").commit_all("theirs");
        repo.git(&["checkout", "-q", "main"]);
        repo.write("a.txt", "1\nyours\n3\n").commit_all("yours");
        let id = open(&ipc, &repo)["id"].clone();

        assert_eq!(
            ipc.call("predict_merge", json!({ "id": id, "branch": "other" }))
                .unwrap(),
            json!(["a.txt"])
        );
        assert_eq!(
            ipc.call("merge_state", json!({ "id": id })).unwrap(),
            Value::Null
        );
        ipc.call("merge", json!({ "id": id, "branch": "other" }))
            .unwrap();
        assert_eq!(
            ipc.call("merge_state", json!({ "id": id })).unwrap(),
            json!({
                "theirs": "other",
                "message": "Merge branch 'other'",
                "conflicts": [{ "path": "a.txt", "kind": "code", "resolved": false }],
            })
        );
        assert_eq!(
            ipc.call("conflict_text", json!({ "id": id, "path": "a.txt" }))
                .unwrap(),
            json!({
                "finalNewline": true,
                "chunks": [
                    { "kind": "same", "lines": ["1"] },
                    { "kind": "conflict", "yours": ["yours"], "base": ["2"], "theirs": ["theirs"] },
                    { "kind": "same", "lines": ["3"] },
                ],
            })
        );

        ipc.call(
            "resolve_with",
            json!({ "id": id, "path": "a.txt", "content": "1\nyours\ntheirs\n3\n", "how": "use both" }),
        )
        .unwrap();
        ipc.call(
            "merge_commit",
            json!({ "id": id, "message": "Merge branch 'other'" }),
        )
        .unwrap();
        assert_eq!(
            ipc.call("merge_state", json!({ "id": id })).unwrap(),
            Value::Null
        );

        let repo2 = TestRepo::init();
        repo2.write("f.txt", "1\n").commit_all("base");
        repo2.git(&["checkout", "-q", "-b", "other"]);
        repo2.write("f.txt", "2\n").commit_all("theirs");
        repo2.git(&["checkout", "-q", "main"]);
        repo2.write("f.txt", "3\n").commit_all("yours");
        let id2 = open(&ipc, &repo2)["id"].clone();
        ipc.call("merge", json!({ "id": id2, "branch": "other" }))
            .unwrap();
        assert_eq!(
            ipc.call("conflict_sides", json!({ "id": id2, "path": "f.txt" }))
                .unwrap(),
            json!({ "yours": { "size": 2, "lfs": false, "hydrated": true }, "theirs": { "size": 2, "lfs": false, "hydrated": true } })
        );
        ipc.call(
            "resolve_side",
            json!({ "id": id2, "path": "f.txt", "side": "theirs" }),
        )
        .unwrap();
        ipc.call("merge_abort", json!({ "id": id2 })).unwrap();
        assert_eq!(
            std::fs::read_to_string(repo2.path().join("f.txt")).unwrap(),
            "3\n"
        );
    }

    #[test]
    fn rebase_with_conflicts_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("a.txt", "1\n2\n3\n").commit_all("base");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.write("a.txt", "1\nF\n3\n").commit_all("f1");
        repo.git(&["checkout", "-q", "main"]);
        repo.write("a.txt", "1\nM\n3\n").commit_all("m1");
        repo.git(&["checkout", "-q", "feature"]);
        let id = open(&ipc, &repo)["id"].clone();

        let plan = ipc
            .call("predict_rebase", json!({ "id": id, "onto": "main" }))
            .unwrap();
        assert_eq!(plan["commits"], 1);
        assert_eq!(plan["conflict"]["paths"], json!(["a.txt"]));
        assert_eq!(plan["conflict"]["subject"], "f1");

        ipc.call("rebase", json!({ "id": id, "onto": "main" }))
            .unwrap();
        let state = ipc.call("rebase_state", json!({ "id": id })).unwrap();
        assert_eq!(state["step"], 1);
        assert_eq!(state["total"], 1);
        assert_eq!(state["onto"], "main");
        assert_eq!(state["conflicts"][0]["path"], "a.txt");

        ipc.call("rebase_step", json!({ "id": id, "step": "abort" }))
            .unwrap();
        assert_eq!(
            ipc.call("rebase_state", json!({ "id": id })).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn history_plan_preview_and_run_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("a.txt", "a\n").commit_all("base");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.write("b.txt", "b\n").commit_all("Add b rgiht");
        repo.write("c.txt", "c\n").commit_all("Add c");
        repo.write("c.txt", "c2\n").commit_all("fixup! Add c");
        let id = open(&ipc, &repo)["id"].clone();

        let plan = ipc
            .call("history_plan", json!({ "id": id, "from": "HEAD~2" }))
            .unwrap();
        assert_eq!(plan["branch"], "feature");
        assert_eq!(plan["commits"][0]["action"], "fixup");
        assert_eq!(plan["commits"][2]["subject"], "Add b rgiht");
        let id_of = |i: usize| plan["commits"][i]["id"].clone();
        let steps = json!([
            { "id": id_of(0), "action": "fixup" },
            { "id": id_of(1), "action": "pick" },
            { "id": id_of(2), "action": "reword", "message": "Add b right" },
        ]);
        let args = json!({ "id": id, "onto": plan["base"], "base": plan["base"], "steps": steps });

        let preview = ipc.call("plan_preview", args.clone()).unwrap();
        assert_eq!(preview["rows"], 6);
        assert_eq!(preview["prediction"]["after"], 2);
        assert_eq!(preview["prediction"]["changed"], 0);
        assert_eq!(
            preview["prediction"]["steps"][2]["outcome"]["kind"],
            "clean"
        );
        let rows = ipc
            .call("preview_rows", json!({ "id": id, "start": 0, "count": 3 }))
            .unwrap();
        assert_eq!(rows[1]["subject"], "Add b right");
        assert_eq!(rows[1]["after"], "reworded");
        assert_eq!(rows[0]["layout"]["segments"][0]["stroke"], "dashed");
        assert_eq!(rows[2]["faded"], true);

        let mut run = args;
        run["updateRefs"] = json!(false);
        ipc.call("run_plan", run).unwrap();
        assert_eq!(
            repo.git(&["log", "--format=%s", "main..HEAD"]),
            "Add c\nAdd b right\n"
        );

        ipc.call(
            "reword_head",
            json!({ "id": id, "message": "Add c, smoothed" }),
        )
        .unwrap();
        let tip = repo.git(&["rev-parse", "HEAD"]);
        ipc.call(
            "quick_edit",
            json!({ "id": id, "commit": tip.trim(), "action": "drop" }),
        )
        .unwrap();
        assert_eq!(
            repo.git(&["log", "--format=%s", "main..HEAD"]),
            "Add b right\n"
        );
        let undone = ipc.call("undo", json!({ "id": id })).unwrap();
        assert_eq!(undone["action"], "Drop \"Add c, smoothed\"");
    }

    #[test]
    fn predicted_conflicts_carry_the_file_with_markers() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("a.txt", "1\n2\n3\n").commit_all("base");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.write("a.txt", "1\nA\n3\n").commit_all("one");
        repo.write("a.txt", "1\nB\n3\n").commit_all("two");
        let id = open(&ipc, &repo)["id"].clone();
        let plan = ipc
            .call("history_plan", json!({ "id": id, "from": "HEAD~1" }))
            .unwrap();
        let steps = json!([
            { "id": plan["commits"][1]["id"], "action": "pick" },
            { "id": plan["commits"][0]["id"], "action": "pick" },
        ]);
        let preview = ipc
            .call(
                "plan_preview",
                json!({ "id": id, "onto": "main", "base": plan["base"], "steps": steps }),
            )
            .unwrap();
        let first = &preview["prediction"]["steps"][1];
        assert_eq!(first["step"], 1);
        assert_eq!(first["outcome"]["kind"], "conflict");
        assert_eq!(first["outcome"]["paths"], json!(["a.txt"]));
        let text = ipc
            .call(
                "predicted_file",
                json!({ "id": id, "tree": first["outcome"]["tree"], "path": "a.txt" }),
            )
            .unwrap();
        assert!(text.as_str().unwrap().contains("<<<<<<<"));
        assert_eq!(
            preview["prediction"]["steps"][0]["outcome"],
            json!({ "kind": "conflict", "paths": ["a.txt"], "tree": preview["prediction"]["steps"][0]["outcome"]["tree"] })
        );
    }

    #[test]
    fn outside_changes_emit_repo_changed_with_the_id() {
        use std::sync::mpsc;
        use std::time::Duration;
        use tauri::Listener;

        let ipc = Ipc::new();
        let repo = history();
        let (tx, rx) = mpsc::channel();
        ipc.app.listen_any(REPO_CHANGED, move |event| {
            let _ = tx.send(event.payload().to_owned());
        });
        let id = open(&ipc, &repo)["id"].clone();
        std::thread::sleep(Duration::from_millis(100));

        repo.write("a.txt", "edited outside");

        let payload = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("repo-changed");
        assert_eq!(payload, id.to_string());
    }

    #[test]
    fn open_repo_rejects_a_plain_folder_with_a_message() {
        let dir = tempfile::tempdir().unwrap();
        let err = Ipc::new()
            .call("open_repo", json!({ "path": dir.path() }))
            .unwrap_err();
        assert!(
            err.as_str()
                .unwrap()
                .ends_with("is not inside a git repository"),
            "{err}"
        );
    }

    #[test]
    fn repo_status_by_id() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "dirty").write("new.txt", "n");
        let id = open(&ipc, &repo)["id"].clone();

        let status = ipc.call("repo_status", json!({ "id": id })).unwrap();
        assert_eq!(status["head"], json!({ "kind": "branch", "name": "main" }));
        assert_eq!(
            status["entries"],
            json!([
                { "path": "a.txt", "kind": "tracked", "index": null, "worktree": "modified" },
                { "path": "new.txt", "kind": "untracked", "index": null, "worktree": null },
            ])
        );
    }

    #[test]
    fn graph_rows_shape() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();

        let rows = ipc
            .call("graph_rows", json!({ "id": id, "start": 0, "count": 10 }))
            .unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let head = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(
            rows[0],
            json!({
                "id": head.trim(),
                "shortId": &head.trim()[..7],
                "subject": "second",
                "author": "gitgud tests",
                "time": rows[0]["time"],
                "labels": [
                    { "kind": "branch", "name": "main", "current": true },
                    { "kind": "tag", "name": "v1" },
                ],
                "wip": false,
                "after": null,
                "faded": false,
                "layout": {
                    "node": 0,
                    "segments": [{ "from": 0, "to": 0, "part": "bottom" }],
                    "overflow": false,
                },
            })
        );
        assert!(rows[0]["time"].as_i64().unwrap() > 1_700_000_000);
    }

    #[test]
    fn closed_and_unknown_ids_fail_with_a_message() {
        let ipc = Ipc::new();
        let id = open(&ipc, &history())["id"].clone();
        ipc.call("close_repo", json!({ "id": id })).unwrap();

        let err = ipc
            .call("graph_rows", json!({ "id": id, "start": 0, "count": 1 }))
            .unwrap_err();
        assert_eq!(err, format!("repository {id} is not open"));
        assert!(ipc.call("repo_status", json!({ "id": 4242 })).is_err());
    }

    #[test]
    fn launch_path_is_null_without_gitgud_open() {
        assert_eq!(
            Ipc::new().call("launch_path", json!({})).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn branch_ops_commit_options_reset_and_stash_through_ipc() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();

        ipc.call(
            "create_branch",
            json!({ "id": id, "name": "feat/a", "at": null, "checkout": true }),
        )
        .unwrap();
        ipc.call(
            "rename_branch",
            json!({ "id": id, "old": "feat/a", "new": "feat/b" }),
        )
        .unwrap();
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "feat/b");

        repo.write("a.txt", "stash me");
        ipc.call(
            "stash_save",
            json!({ "id": id, "message": "wip", "includeUntracked": false }),
        )
        .unwrap();
        let list = ipc.call("stashes", json!({ "id": id })).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert_eq!(list[0]["message"], "wip");
        let detail = ipc
            .call("stash_detail", json!({ "id": id, "index": 0 }))
            .unwrap();
        assert_eq!(detail["files"][0]["path"], "a.txt");
        let diff = ipc
            .call(
                "stash_file_diff",
                json!({ "id": id, "index": 0, "path": "a.txt" }),
            )
            .unwrap();
        assert!(!diff["hunks"].as_array().unwrap().is_empty());
        ipc.call("stash_pop", json!({ "id": id, "index": 0 }))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "stash me"
        );

        ipc.call("stage", json!({ "id": id, "paths": ["a.txt"] }))
            .unwrap();
        ipc.call(
            "commit",
            json!({ "id": id, "message": "signed", "amend": false, "signoff": true, "noVerify": true }),
        )
        .unwrap();
        assert!(
            ipc.call("head_commit_message", json!({ "id": id }))
                .unwrap()
                .as_str()
                .unwrap()
                .contains("Signed-off-by:")
        );
        let prev = repo.git(&["rev-parse", "HEAD~1"]).trim().to_owned();
        ipc.call("reset", json!({ "id": id, "target": prev, "mode": "hard" }))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "b"
        );

        ipc.call("checkout", json!({ "id": id, "branch": "main" }))
            .unwrap();
        ipc.call(
            "delete_branch",
            json!({ "id": id, "name": "feat/b", "force": false }),
        )
        .unwrap();
    }

    #[test]
    fn pull_push_sequencer_and_pr_url_through_ipc() {
        let ipc = Ipc::new();
        let local = history();
        let origin = tempfile::tempdir().unwrap();
        let bare = origin.path().join("o.git");
        local.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
        local.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        local.git(&["fetch", "-q", "origin"]);
        local.git(&["branch", "-q", "--set-upstream-to=origin/main", "main"]);
        local.git(&["remote", "add", "gh", "git@github.com:HappyRave/gitgud.git"]);
        let id = open(&ipc, &local)["id"].clone();

        let pulled = ipc
            .call(
                "pull",
                json!({ "id": id, "strategy": "auto", "remember": false }),
            )
            .unwrap();
        assert_eq!(pulled["outcome"]["kind"], "upToDate");

        let info = ipc
            .call("push_info", json!({ "id": id, "remote": "origin" }))
            .unwrap();
        assert_eq!(info["branch"], "main");
        assert_eq!(info["remote"], "origin");

        local.write("p.txt", "p").commit_all("pushable");
        ipc.call(
            "push",
            json!({ "id": id, "remote": "origin", "branch": "main", "remoteBranch": "main", "setUpstream": false, "forceWithLease": false }),
        )
        .unwrap();
        assert_eq!(
            ipc.call("push_info", json!({ "id": id, "remote": "gh" }))
                .unwrap()["prUrl"],
            "https://github.com/HappyRave/gitgud/pull/new/main"
        );
        assert!(validate_external_url("https://github.com/a/b").is_ok());
        assert!(validate_external_url("javascript:alert(1)").is_err());

        // Sequencer IPC: conflicting cherry-pick -> resolve -> continue.
        local.git(&["checkout", "-q", "-b", "other"]);
        local.write("a.txt", "theirs\n").commit_all("theirs");
        let target = local.git(&["rev-parse", "HEAD"]).trim().to_owned();
        local.git(&["checkout", "-q", "main"]);
        local.write("a.txt", "ours\n").commit_all("ours");
        assert_eq!(
            ipc.call("predict_cherry_pick", json!({ "id": id, "commit": target }))
                .unwrap(),
            json!(["a.txt"])
        );
        ipc.call("cherry_pick", json!({ "id": id, "commit": target }))
            .unwrap();
        let state = ipc.call("sequencer_state", json!({ "id": id })).unwrap();
        assert_eq!(state["kind"], "cherryPick");
        ipc.call(
            "resolve_with",
            json!({ "id": id, "path": "a.txt", "content": "merged\n", "how": "edit" }),
        )
        .unwrap();
        ipc.call("sequencer_step", json!({ "id": id, "step": "continue" }))
            .unwrap();
        assert_eq!(
            ipc.call("sequencer_state", json!({ "id": id })).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn library_scan_clone_and_fetch_many_through_ipc() {
        let ipc = Ipc::new();
        let origin = history();
        let bare_dir = tempfile::tempdir().unwrap();
        let bare = bare_dir.path().join("upstream.git");
        origin.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);

        let dest_root = tempfile::tempdir().unwrap();
        let cloned_dest = dest_root.path().join("cloned-app");
        let cloned_path = ipc
            .call(
                "clone_repo",
                json!({
                    "url": bare.to_str().unwrap(),
                    "dest": cloned_dest.to_str().unwrap(),
                    "folderPath": "Studio / Games",
                    "longPaths": true,
                }),
            )
            .unwrap();
        assert!(cloned_path.as_str().unwrap().ends_with("cloned-app"));

        let loaded = ipc.call("library_load", json!({})).unwrap();
        assert_eq!(loaded["repos"].as_array().unwrap().len(), 1);
        assert_eq!(loaded["folders"].as_array().unwrap().len(), 2);

        let scanned = ipc
            .call("scan_folder", json!({ "root": dest_root.path() }))
            .unwrap();
        assert_eq!(scanned["totalFound"], 1);
        assert!(
            scanned["groups"][0]["repos"][0]["alreadyInLibrary"]
                .as_bool()
                .unwrap()
        );

        let folder = ipc
            .call(
                "library_create_folder",
                json!({ "name": "Tools", "parentId": null }),
            )
            .unwrap();
        assert!(
            folder["folders"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["name"] == "Tools")
        );

        let undone = ipc.call("library_undo", json!({})).unwrap();
        assert!(
            !undone["folders"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["name"] == "Tools")
        );

        let redone = ipc.call("library_redo", json!({})).unwrap();
        assert!(
            redone["folders"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["name"] == "Tools")
        );

        origin
            .write("remote.txt", "from remote")
            .commit_all("new remote commit");
        origin.git(&["push", "-q", bare.to_str().unwrap(), "main"]);
        let sums = ipc
            .call("fetch_many", json!({ "paths": [cloned_path] }))
            .unwrap();
        assert_eq!(sums[0]["behind"], 1);
    }

    #[test]
    fn line_staging_whitespace_diff_and_config_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("f.txt", "one\ntwo\nthree\nfour\n")
            .commit_all("base");
        let id = open(&ipc, &repo)["id"].clone();

        repo.write("f.txt", "one\n  two  \nthree changed\nfour\n");

        let ws_on = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "unstaged", "ignoreWhitespace": false }),
            )
            .unwrap();
        let ws_lines = ws_on["hunks"][0]["lines"].as_array().unwrap();
        assert!(ws_lines.iter().any(|l| l["text"] == "  two  "));

        let ws_off = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "unstaged", "ignoreWhitespace": true }),
            )
            .unwrap();
        let ws_off_lines = ws_off["hunks"][0]["lines"].as_array().unwrap();
        assert!(!ws_off_lines.iter().any(|l| l["text"] == "  two  "));
        assert!(ws_off_lines.iter().any(|l| l["text"] == "three changed"));

        let hunk = &ws_on["hunks"][0];
        let select_hunk = json!({
            "oldStart": hunk["oldStart"],
            "oldLines": hunk["oldLines"],
            "newStart": hunk["newStart"],
            "newLines": hunk["newLines"],
        });
        let changed_three: Vec<usize> = ws_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                (l["kind"] == "removed" && l["text"] == "three")
                    || (l["kind"] == "added" && l["text"] == "three changed")
            })
            .map(|(i, _)| i)
            .collect();
        ipc.call(
            "stage_lines",
            json!({ "id": id, "path": "f.txt", "select": { "hunk": select_hunk, "lines": changed_three } }),
        )
        .unwrap();

        let staged = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "staged" }),
            )
            .unwrap();
        let staged_lines = staged["hunks"][0]["lines"].as_array().unwrap();
        assert!(staged_lines.iter().any(|l| l["text"] == "three changed"));
        assert!(!staged_lines.iter().any(|l| l["text"] == "  two  "));

        let staged_hunk = &staged["hunks"][0];
        let staged_three_added: Vec<usize> = staged_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l["kind"] == "added" && l["text"] == "three changed")
            .map(|(i, _)| i)
            .collect();
        ipc.call(
            "unstage_lines",
            json!({
                "id": id,
                "path": "f.txt",
                "select": {
                    "hunk": {
                        "oldStart": staged_hunk["oldStart"],
                        "oldLines": staged_hunk["oldLines"],
                        "newStart": staged_hunk["newStart"],
                        "newLines": staged_hunk["newLines"],
                    },
                    "lines": staged_three_added,
                },
            }),
        )
        .unwrap();

        let unstaged_after = ipc
            .call(
                "worktree_file_diff",
                json!({ "id": id, "path": "f.txt", "side": "unstaged" }),
            )
            .unwrap();
        assert!(
            unstaged_after["hunks"][0]["lines"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["text"] == "three changed")
        );

        let uh = &unstaged_after["hunks"][0];
        let discard_idx: Vec<usize> = uh["lines"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, l)| l["kind"] == "added" && l["text"] == "three changed")
            .map(|(i, _)| i)
            .collect();
        ipc.call(
            "discard_lines",
            json!({
                "id": id,
                "path": "f.txt",
                "select": {
                    "hunk": {
                        "oldStart": uh["oldStart"],
                        "oldLines": uh["oldLines"],
                        "newStart": uh["newStart"],
                        "newLines": uh["newLines"],
                    },
                    "lines": discard_idx,
                },
            }),
        )
        .unwrap();
        ipc.call("undo", json!({ "id": id })).unwrap();
        assert!(
            std::fs::read_to_string(repo.path().join("f.txt"))
                .unwrap()
                .contains("three changed")
        );

        ipc.call(
            "config_set_identity",
            json!({ "id": id, "name": "Alice Test", "email": "alice@example.com" }),
        )
        .unwrap();
        let ident = ipc
            .call("config_get_identity", json!({ "id": id }))
            .unwrap();
        assert_eq!(ident["userName"], "Alice Test");
        assert_eq!(ident["userEmail"], "alice@example.com");

        let diag = ipc.call("tool_diagnostics", json!({})).unwrap();
        assert!(diag["gitPath"].as_str().unwrap().contains("git"));
        assert_eq!(diag["gitVersion"].as_str().unwrap().split('.').count(), 3);
    }

    #[test]
    fn unity_lfs_tga_and_background_refresh_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        // Minimal valid 1x1 32bpp uncompressed TGA
        let mut tga = vec![0u8; 22];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&1u16.to_le_bytes());
        tga[14..16].copy_from_slice(&1u16.to_le_bytes());
        tga[16] = 32;
        tga[17] = 0x20;
        tga[18..22].copy_from_slice(&[0, 255, 0, 255]);
        std::fs::write(repo.path().join("tex.tga"), &tga).unwrap();

        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.12f1\n",
        )
        .write(
            ".gitattributes",
            "*.tga filter=lfs diff=lfs merge=lfs -text\n",
        )
        .write("Assets/Foo.cs", "class Foo {}")
        .write(
            "Assets/Foo.cs.meta",
            "fileFormatVersion: 2\nguid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        )
        .commit_all("unity init");
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://bitbucket.org/team/game.git",
        ]);

        repo.write("Temp/UnityLockfile", "")
            .write("Assets/Foo.cs", "class Foo { int x; }")
            .write(
                "Assets/Foo.cs.meta",
                "fileFormatVersion: 2\nguid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\ntimeCreated: 2\n",
            )
            .write("Assets/Bar.cs", "class Bar {}");

        let id = open(&ipc, &repo)["id"].clone();

        let uinfo = ipc.call("unity_info", json!({ "id": id })).unwrap();
        assert_eq!(uinfo["isUnity"], true);
        assert_eq!(uinfo["editorOpen"], true);
        assert_eq!(uinfo["editorVersion"], "6000.0.12f1");

        let issues = ipc.call("unity_meta_issues", json!({ "id": id })).unwrap();
        assert!(
            issues
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["kind"] == "missingMeta" && i["path"] == "Assets/Bar.cs")
        );

        // Staging Assets/Foo.cs via IPC auto-pairs Assets/Foo.cs.meta.
        ipc.call("stage", json!({ "id": id, "paths": ["Assets/Foo.cs"] }))
            .unwrap();
        let status = ipc.call("repo_status", json!({ "id": id })).unwrap();
        assert!(
            status["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["path"] == "Assets/Foo.cs.meta" && !e["index"].is_null())
        );

        let lfs = ipc.call("lfs_info", json!({ "id": id })).unwrap();
        assert_eq!(lfs["supportsLocks"], false);
        assert!(
            lfs["locksUnsupportedReason"]
                .as_str()
                .unwrap()
                .contains("Bitbucket Cloud")
        );

        let bg = ipc.call("background_refresh", json!({ "id": id })).unwrap();
        assert_eq!(bg, false);

        // TGA image protocol decodes and serves as image/png.
        let r = images::respond(
            &ipc.app.state::<Session>(),
            &format!(
                "http://gitgud.localhost/image?repo={}&path=tex.tga&at=worktree",
                id.as_u64().unwrap()
            ),
        );
        assert_eq!(r.status(), tauri::http::StatusCode::OK);
        assert_eq!(r.headers()[tauri::http::header::CONTENT_TYPE], "image/png");
        assert!(r.body().starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn tags_blame_reflog_and_commit_graph_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("notes.txt", "alpha\nbeta\ngamma\n")
            .commit_all("add notes");

        let id = open(&ipc, &repo)["id"].clone();

        ipc.call(
            "create_tag",
            json!({ "id": id, "name": "v0.1.0", "message": "release 0.1" }),
        )
        .unwrap();
        let listed = ipc.call("tags", json!({ "id": id })).unwrap();
        let arr = listed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "v0.1.0");
        assert_eq!(arr[0]["annotated"], true);
        assert_eq!(arr[0]["message"], "release 0.1");

        let hist = ipc
            .call("file_history", json!({ "id": id, "path": "notes.txt" }))
            .unwrap();
        assert_eq!(hist.as_array().unwrap().len(), 1);
        assert_eq!(hist[0]["subject"], "add notes");

        let blame = ipc
            .call("blame_file", json!({ "id": id, "path": "notes.txt" }))
            .unwrap();
        assert_eq!(blame.as_array().unwrap().len(), 3);
        assert_eq!(blame[1]["text"], "beta");

        let reflog = ipc.call("outside_reflog", json!({ "id": id })).unwrap();
        assert!(!reflog.as_array().unwrap().is_empty());

        let stats = ipc.call("op_log_stats", json!({ "id": id })).unwrap();
        assert_eq!(stats["entryCount"], 0);
        let pruned = ipc.call("op_log_prune", json!({ "id": id })).unwrap();
        assert_eq!(pruned["entryCount"], 0);

        assert_eq!(
            ipc.call("has_commit_graph", json!({ "id": id })).unwrap(),
            false
        );
        ipc.call("write_commit_graph", json!({ "id": id })).unwrap();
        assert_eq!(
            ipc.call("has_commit_graph", json!({ "id": id })).unwrap(),
            true
        );

        ipc.call("delete_tag", json!({ "id": id, "name": "v0.1.0" }))
            .unwrap();
        assert_eq!(
            ipc.call("tags", json!({ "id": id }))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn unity_yaml_structural_diff_conflict_table_and_integrity_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        let script_guid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let base_prefab = format!(
            "%YAML 1.1\n--- !u!1 &10\nGameObject:\n  m_Name: Player\n--- !u!4 &11\nTransform:\n  m_GameObject: {{fileID: 10}}\n  m_Father: {{fileID: 0}}\n--- !u!114 &12\nMonoBehaviour:\n  m_GameObject: {{fileID: 10}}\n  m_Script: {{fileID: 11500000, guid: {script_guid}, type: 3}}\n  speed: 10\n"
        );
        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.12f1\n",
        )
        .write(
            "ProjectSettings/EditorSettings.asset",
            "m_SerializationMode: 2\n",
        )
        .write("Assets/Mover.cs", "class Mover {}")
        .write(
            "Assets/Mover.cs.meta",
            &format!("fileFormatVersion: 2\nguid: {script_guid}\n"),
        )
        .write("Assets/Player.prefab", &base_prefab)
        .commit_all("base prefab");

        let default_branch = repo
            .git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_owned();

        let theirs_prefab = base_prefab.replace("speed: 10", "speed: 25");
        repo.git(&["checkout", "-b", "theirs"]);
        repo.write("Assets/Player.prefab", &theirs_prefab)
            .commit_all("theirs speed");

        repo.git(&["checkout", &default_branch]);
        let ours_prefab = base_prefab.replace("speed: 10", "speed: 15");
        repo.write("Assets/Player.prefab", &ours_prefab)
            .commit_all("ours speed");

        // Unstaged structural change before merge.
        let dirty_prefab = ours_prefab.replace("speed: 15", "speed: 18");
        repo.write("Assets/Player.prefab", &dirty_prefab);

        let id = open(&ipc, &repo)["id"].clone();

        let setup_before = ipc
            .call("unity_repo_setup_status", json!({ "id": id }))
            .unwrap();
        assert_eq!(setup_before["isUnity"], true);
        assert_eq!(setup_before["forceTextOk"], true);
        assert!(
            !setup_before["gitignoreMissing"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let setup_after = ipc
            .call(
                "unity_apply_repo_setup",
                json!({ "id": id, "writeGitignore": true, "writeGitattributes": true }),
            )
            .unwrap();
        assert!(
            setup_after["gitignoreMissing"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            setup_after["gitattributesMissing"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let wdiff = ipc
            .call(
                "worktree_unity_diff",
                json!({ "id": id, "path": "Assets/Player.prefab", "side": "unstaged" }),
            )
            .unwrap();
        assert_eq!(wdiff["modifiedProperties"], 1);
        assert_eq!(wdiff["objects"][0]["objectPath"], "Player");
        assert_eq!(wdiff["objects"][0]["component"], "Mover.cs");

        let head_sha = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        let cdiff = ipc
            .call(
                "commit_unity_diff",
                json!({ "id": id, "commit": head_sha, "path": "Assets/Player.prefab" }),
            )
            .unwrap();
        assert_eq!(cdiff["objects"][0]["properties"][0]["after"], "15");

        // Restore clean working tree before triggering merge conflict.
        repo.git(&["checkout", "--", "Assets/Player.prefab"]);
        ipc.call("merge", json!({ "id": id, "branch": "theirs" }))
            .unwrap();

        let table = ipc
            .call(
                "unity_conflict_table",
                json!({ "id": id, "path": "Assets/Player.prefab" }),
            )
            .unwrap();
        assert_eq!(table["conflicts"].as_array().unwrap().len(), 1);
        assert_eq!(table["conflicts"][0]["property"], "speed");

        ipc.call(
            "resolve_unity_conflict",
            json!({
                "id": id,
                "path": "Assets/Player.prefab",
                "picks": [{ "fileId": "12", "property": "speed", "value": "30" }]
            }),
        )
        .unwrap();

        let issues = ipc
            .call(
                "unity_post_merge_check",
                json!({ "id": id, "paths": ["Assets/Player.prefab"] }),
            )
            .unwrap();
        assert_eq!(issues.as_array().unwrap().len(), 0);
    }

    #[test]
    fn proposals_review_approve_request_changes_and_reject_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("hello.txt", "v1\n").commit_all("initial");
        repo.write("hello.txt", "v2\n");

        let opened = repo::open(repo.path()).unwrap();
        let git = gitgud_core::cli::GitCli::new(&git::locate().unwrap());
        let p1 = gitgud_core::proposals::create_proposal(
            &opened,
            &git,
            &gitgud_core::proposals::CreateProposalInput {
                message: "feat: agent proposal 1".into(),
                why: Some("improves greeting".into()),
                agent: Some("claude".into()),
                paths: vec!["hello.txt".into()],
                hunk_ids: vec![],
            },
        )
        .unwrap();

        let id = open(&ipc, &repo)["id"].clone();

        let listed = ipc.call("proposals_list", json!({ "id": id })).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["id"], p1.id);

        let pdiff = ipc
            .call(
                "proposal_file_diff",
                json!({ "id": id, "proposalId": p1.id, "path": "hello.txt" }),
            )
            .unwrap();
        assert!(!pdiff["hunks"].as_array().unwrap().is_empty());

        let updated = ipc
            .call(
                "proposal_request_changes",
                json!({ "id": id, "proposalId": p1.id, "comment": "add docstring" }),
            )
            .unwrap();
        assert_eq!(updated["status"], "changesRequested");

        ipc.call(
            "proposal_approve",
            json!({
                "id": id,
                "proposalId": p1.id,
                "message": "feat: approved greeting",
                "selectedPaths": ["hello.txt"]
            }),
        )
        .unwrap();
        assert_eq!(
            repo.git(&["log", "-1", "--format=%s"]).trim(),
            "feat: approved greeting"
        );

        repo.write("extra.txt", "discard me\n");
        let p2 = gitgud_core::proposals::create_proposal(
            &opened,
            &git,
            &gitgud_core::proposals::CreateProposalInput {
                message: "feat: rejectable".into(),
                why: None,
                agent: Some("codex".into()),
                paths: vec!["extra.txt".into()],
                hunk_ids: vec![],
            },
        )
        .unwrap();
        ipc.call(
            "proposal_reject",
            json!({ "id": id, "proposalId": p2.id, "reason": "not needed" }),
        )
        .unwrap();
        let after = ipc.call("proposals_list", json!({ "id": id })).unwrap();
        assert!(
            after
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["id"] == p2.id && x["status"] == "rejected")
        );
    }

    #[test]
    fn stacks_lfs_cache_ai_and_agent_setup_through_ipc() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("base.txt", "base\n").commit_all("base");
        repo.git(&["remote", "add", "origin", "https://github.com/Max/boat.git"]);
        repo.git(&["branch", "-M", "main"]);
        repo.git(&["checkout", "-b", "stack/part1"]);
        repo.write("p1.txt", "one\n").commit_all("part1");
        repo.git(&["checkout", "-b", "stack/part2"]);
        repo.write("p2.txt", "two\n").commit_all("part2");
        repo.git(&["checkout", "main"]);
        repo.write("up.txt", "up\n").commit_all("advance main");
        repo.git(&["checkout", "stack/part2"]);

        let id = open(&ipc, &repo)["id"].clone();

        let stacks = ipc.call("branch_stacks", json!({ "id": id })).unwrap();
        let arr = stacks.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["levels"][0]["needsRestack"], true);
        assert_eq!(
            arr[0]["levels"][1]["prUrl"],
            "https://github.com/Max/boat/compare/stack/part1...stack/part2?expand=1"
        );

        let compare_url = ipc
            .call(
                "pr_url_with_base",
                json!({ "id": id, "branch": "stack/part2", "base": "stack/part1" }),
            )
            .unwrap();
        assert_eq!(
            compare_url,
            "https://github.com/Max/boat/compare/stack/part1...stack/part2?expand=1"
        );

        ipc.call(
            "restack",
            json!({ "id": id, "topBranch": "stack/part2", "onto": "main" }),
        )
        .unwrap();
        let after_restack = ipc.call("branch_stacks", json!({ "id": id })).unwrap();
        assert_eq!(after_restack[0]["levels"][0]["needsRestack"], false);

        let lfs_stats = ipc.call("lfs_cache_stats", json!({ "id": id })).unwrap();
        assert_eq!(lfs_stats["objectCount"], 0);

        let ai = ipc.call("ai_providers", json!({})).unwrap();
        assert!(ai["cliAvailable"].is_array());

        let written = ipc
            .call("agent_setup", json!({ "id": id, "target": "generic" }))
            .unwrap();
        assert_eq!(written, "GITGUD_AGENT.md");
        assert!(repo.path().join("GITGUD_AGENT.md").is_file());

        let claude = ipc
            .call(
                "agent_setup",
                json!({ "id": id, "target": "claude", "withHooks": true }),
            )
            .unwrap();
        assert_eq!(
            claude,
            ".claude/skills/gitgud/SKILL.md + .claude/settings.local.json"
        );
        assert!(repo.path().join(".claude/skills/gitgud/SKILL.md").is_file());
        assert!(repo.path().join(".claude/settings.local.json").is_file());
    }

    #[test]
    fn stale_origin_timings_refresh_index_and_worktree_proposal_through_ipc() {
        let ipc = Ipc::new();
        let main = TestRepo::init();
        main.write("app.txt", "v1\n").commit_all("initial");
        let wt_parent = tempfile::tempdir().unwrap();
        let wt_dir = wt_parent.path().join("agent-wt");
        main.git(&[
            "worktree",
            "add",
            "-b",
            "agent/wt",
            wt_dir.to_str().unwrap(),
        ]);
        std::fs::write(wt_dir.join("app.txt"), "v2\n").unwrap();

        let id = open(&ipc, &main)["id"].clone();
        ipc.call("repo_status", json!({ "id": id })).unwrap();
        ipc.call("refresh_index", json!({ "id": id })).unwrap();

        let timings = ipc.call("recent_timings", json!({})).unwrap();
        assert!(!timings.as_array().unwrap().is_empty());

        let stale = ipc
            .call(
                "stale_origin_commits",
                json!({ "id": id, "upstream": "origin/main" }),
            )
            .unwrap();
        assert_eq!(stale.as_array().unwrap().len(), 0);

        let wts = ipc.call("worktrees_list", json!({ "id": id })).unwrap();
        assert!(
            wts.as_array()
                .unwrap()
                .iter()
                .any(|w| w["isMain"] == false && w["dirtyCount"] == 1)
        );

        let prop = ipc
            .call(
                "propose_from_worktree",
                json!({
                    "id": id,
                    "worktreePath": wt_dir,
                    "message": "feat: wt ipc",
                    "why": "ipc test",
                    "agent": "codex"
                }),
            )
            .unwrap();
        assert_eq!(prop["status"], "pending");
        main.git(&["worktree", "remove", "--force", wt_dir.to_str().unwrap()]);
    }

    #[test]
    fn lfs_fetch_object_ipc_handles_hydrated_and_invalid_oids() {
        let ipc = Ipc::new();
        let repo = TestRepo::init();
        repo.write("a.txt", "a\n").commit_all("init");
        let id = open(&ipc, &repo)["id"].clone();

        let err = ipc
            .call(
                "lfs_fetch_object",
                json!({ "id": id, "oid": "bad", "size": 10 }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid LFS oid"));

        let oid = "ef".repeat(32);
        let obj = repo.path().join(".git/lfs/objects/ef/ef").join(&oid);
        std::fs::create_dir_all(obj.parent().unwrap()).unwrap();
        std::fs::write(&obj, b"ok").unwrap();

        let downloaded = ipc
            .call(
                "lfs_fetch_object",
                json!({ "id": id, "oid": oid, "size": 2, "path": "Art/tex.dds" }),
            )
            .unwrap();
        assert_eq!(downloaded, json!(false));
    }
}
