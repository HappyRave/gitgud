use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gitgud_core::branches::Branches;
use gitgud_core::clone::{CloneProgress, clone_repo as core_clone_repo};
use gitgud_core::diff::{CommitDetail, FileDiff};
use gitgud_core::git::{self, GitInfo};
use gitgud_core::graph::Row;
use gitgud_core::history::{Action, HistoryPlan, Step};
use gitgud_core::library::{
    AddRepoEntry, DEFAULT_SCAN_DEPTH, LibrarySnapshot, LibraryStore, OpenTabState, RepoSummary,
    ScanResult, display_path, scan_folder as core_scan_folder,
};
use gitgud_core::merge::{ConflictSides, ConflictText, MergeState, Side as MergeSide};
use gitgud_core::oplog::{OpEntry, label_paths};
use gitgud_core::ops::{CommitOptions, ResetMode};
use gitgud_core::rebase::{RebasePlan, RebaseState, RebaseStep};
use gitgud_core::remote::{PullStrategy, PushInfo};
use gitgud_core::repo::{self, Error, Status};
use gitgud_core::sequencer::{SequencerState, SequencerStep};
use gitgud_core::session::{Opened, PlanPreview, PullResult, Reverted, Session};
use gitgud_core::stash::StashEntry;
use gitgud_core::task::{Cancel, Progress};
use gitgud_core::worktree::{HunkHeader, Side};
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
    session.get(id)?.repo.status()
}

#[tauri::command]
async fn graph_rows(
    session: State<'_, Session>,
    id: u32,
    start: usize,
    count: usize,
) -> Result<Vec<Row>, Error> {
    let open = session.get(id)?;
    open.graph.rows(&open.repo, start, count)
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
) -> Result<FileDiff, Error> {
    session.get(id)?.repo.commit_file_diff(&commit, &path)
}

#[tauri::command]
async fn worktree_file_diff(
    session: State<'_, Session>,
    id: u32,
    path: String,
    side: Side,
) -> Result<FileDiff, Error> {
    session.get(id)?.repo.worktree_file_diff(&path, side)
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
async fn discard(
    session: State<'_, Session>,
    id: u32,
    paths: Vec<String>,
) -> Result<Opened, Error> {
    let action = label_paths("Discard", &paths);
    session.write_destructive(id, &action, &paths, |repo, git| repo.discard(git, &paths))
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
    let action = label_paths("Stage", &paths);
    session.write(id, &action, |repo, git| repo.stage(git, &paths))
}

#[tauri::command]
async fn unstage(
    session: State<'_, Session>,
    id: u32,
    paths: Vec<String>,
) -> Result<Opened, Error> {
    let action = label_paths("Unstage", &paths);
    session.write(id, &action, |repo, git| repo.unstage(git, &paths))
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
) -> Result<FileDiff, Error> {
    session.get(id)?.repo.stash_file_diff(index, &path)
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
            fetch_many
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_with_library(Builder::default(), None)
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = LibraryStore::load(dir.join("library.json"))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            app.manage(store);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
            json!({ "yours": { "size": 2, "lfs": false }, "theirs": { "size": 2, "lfs": false } })
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
}
