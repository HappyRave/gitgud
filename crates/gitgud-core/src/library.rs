use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gix::remote::Direction;
use serde::{Deserialize, Serialize};

use crate::branches::count_only_in;
use crate::remote_url::HostKind;
use crate::repo::{self, Error, read_err};

/// Maximum depth visited by [`scan_folder`].
pub const DEFAULT_SCAN_DEPTH: usize = 5;

const MAX_UNDO: usize = 100;
const REFRESH_THREADS: usize = 8;

/// Folder in the Library sidebar tree (R3-A, nested freely, one folder per repo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub order: usize,
}

/// Cached local-only repository summary shown in the Library table (R3 Q4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub path: String,
    pub missing: bool,
    pub branch: Option<String>,
    pub detached: bool,
    pub dirty_count: usize,
    pub ahead: u32,
    pub behind: u32,
    pub fetched_at: Option<i64>,
    pub locks_held: usize,
    pub locks_checked_at: Option<i64>,
    pub last_activity: Option<i64>,
    pub host: HostKind,
    pub is_unity: bool,
    pub checked_at: i64,
}

/// A repository registered in gitgud's Library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRepo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub folder_id: Option<String>,
    pub pinned: bool,
    pub order: usize,
    pub last_opened: Option<i64>,
    pub cached: Option<RepoSummary>,
}

/// Tabs open across the previous session (restored at start-up, R3 Q5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTabState {
    pub open_paths: Vec<String>,
    pub active_path: Option<String>,
}

/// Persistent Library configuration stored as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryState {
    pub version: u32,
    pub folders: Vec<LibraryFolder>,
    pub repos: Vec<LibraryRepo>,
    pub tabs: OpenTabState,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            version: 1,
            folders: Vec::new(),
            repos: Vec::new(),
            tabs: OpenTabState::default(),
        }
    }
}

/// Library state returned to the frontend together with undo/redo labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    #[serde(flatten)]
    pub state: LibraryState,
    pub undo_action: Option<String>,
    pub redo_action: Option<String>,
}

/// Input entry for [`LibraryStore::add_repos`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRepoEntry {
    pub path: String,
    pub folder_path: Option<String>,
    pub name: Option<String>,
}

/// Repository discovered during [`scan_folder`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedRepo {
    pub name: String,
    pub path: String,
    pub already_in_library: bool,
}

/// Directory group returned by [`scan_folder`] (R3 state 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanGroup {
    pub dir: String,
    pub suggested_folder: String,
    pub repos: Vec<ScannedRepo>,
}

/// Full result of [`scan_folder`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub root: String,
    pub total_found: usize,
    pub elapsed_ms: u64,
    pub groups: Vec<ScanGroup>,
}

#[derive(Debug, Clone)]
struct UndoSnapshot {
    action: String,
    folders: Vec<LibraryFolder>,
    repos: Vec<LibraryRepo>,
}

struct StoreInner {
    state: LibraryState,
    undo_stack: Vec<UndoSnapshot>,
    redo_stack: Vec<UndoSnapshot>,
    next_id: u64,
}

/// Thread-safe, disk-backed Library store with in-memory Library undo/redo.
pub struct LibraryStore {
    file: PathBuf,
    inner: Mutex<StoreInner>,
}

impl LibraryStore {
    /// Loads the Library from `file`, or initializes an empty store if missing.
    ///
    /// # Errors
    /// [`Error::Read`] when `file` exists but cannot be read or parsed.
    pub fn load(file: PathBuf) -> Result<Self, Error> {
        let state = if file.exists() {
            let text = std::fs::read_to_string(&file).map_err(read_err)?;
            serde_json::from_str::<LibraryState>(&text)
                .map_err(|e| Error::Read(format!("corrupt library file: {e}")))?
        } else {
            LibraryState::default()
        };
        let next_id = next_numeric_id(&state);
        Ok(Self {
            file,
            inner: Mutex::new(StoreInner {
                state,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                next_id,
            }),
        })
    }

    /// Current Library snapshot with undo/redo labels.
    ///
    /// # Errors
    /// [`Error::Read`] if the internal lock is poisoned.
    pub fn snapshot(&self) -> Result<LibrarySnapshot, Error> {
        let guard = self.lock()?;
        Ok(make_snapshot(&guard))
    }

    /// Registered repository paths currently in the Library.
    ///
    /// # Errors
    /// [`Error::Read`] if the internal lock is poisoned.
    pub fn existing_paths(&self) -> Result<Vec<String>, Error> {
        let guard = self.lock()?;
        Ok(guard.state.repos.iter().map(|r| r.path.clone()).collect())
    }

    /// Adds repositories, creating nested folders for `folder_path` (`"Studio / VR"`)
    /// when needed and skipping repositories already present.
    ///
    /// # Errors
    /// [`Error::Read`] when writing fails.
    pub fn add_repos(&self, entries: &[AddRepoEntry]) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        if entries.is_empty() {
            return Ok(make_snapshot(&guard));
        }

        let before = capture_undo("", &guard.state);
        let mut known: HashSet<String> = guard
            .state
            .repos
            .iter()
            .map(|r| path_key(&r.path))
            .collect();
        let mut added = 0usize;

        for entry in entries {
            let normalized = display_path(Path::new(&entry.path));
            let key = path_key(&normalized);
            if key.is_empty() || !known.insert(key) {
                continue;
            }
            let folder_id = entry
                .folder_path
                .as_deref()
                .and_then(|fp| ensure_folder_path(&mut guard, fp));
            let id_num = alloc_id(&mut guard.next_id);
            let name = entry
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| repo_display_name(&normalized));
            let order = guard
                .state
                .repos
                .iter()
                .filter(|r| r.folder_id == folder_id)
                .map(|r| r.order)
                .max()
                .map_or(0, |m| m + 1);
            guard.state.repos.push(LibraryRepo {
                id: format!("r-{id_num}"),
                name,
                path: normalized,
                folder_id,
                pinned: false,
                order,
                last_opened: None,
                cached: None,
            });
            added += 1;
        }

        if added > 0 {
            let label = if added == 1 {
                "Add repository".to_owned()
            } else {
                format!("Add {added} repositories")
            };
            push_undo(&mut guard, label, before);
            save_atomic(&self.file, &guard.state)?;
        }
        Ok(make_snapshot(&guard))
    }

    /// Removes a repository from the Library without touching files on disk.
    ///
    /// # Errors
    /// [`Error::Refused`] when `id` is unknown, [`Error::Read`] when writing fails.
    pub fn remove_repo(&self, id: &str) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        let Some(pos) = guard.state.repos.iter().position(|r| r.id == id) else {
            return Err(Error::Refused("repository is not in the library".into()));
        };
        let removed_name = guard.state.repos[pos].name.clone();
        let removed_key = path_key(&guard.state.repos[pos].path);
        let before = capture_undo("", &guard.state);
        guard.state.repos.remove(pos);
        guard
            .state
            .tabs
            .open_paths
            .retain(|p| path_key(p) != removed_key);
        if guard
            .state
            .tabs
            .active_path
            .as_deref()
            .is_some_and(|p| path_key(p) == removed_key)
        {
            guard.state.tabs.active_path = None;
        }
        push_undo(&mut guard, format!("Remove {removed_name}"), before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Updates a repository's path after it moved on disk.
    ///
    /// # Errors
    /// [`Error::Refused`] when `id` is unknown or `new_path` is not a valid git repository.
    pub fn locate_repo(&self, id: &str, new_path: &str) -> Result<LibrarySnapshot, Error> {
        let opened = repo::open(new_path)?;
        let canonical = display_path(opened.root());
        let key = path_key(&canonical);
        let mut guard = self.lock()?;
        if guard
            .state
            .repos
            .iter()
            .any(|r| r.id != id && path_key(&r.path) == key)
        {
            return Err(Error::Refused(
                "that repository is already in the library".into(),
            ));
        }
        let Some(idx) = guard.state.repos.iter().position(|r| r.id == id) else {
            return Err(Error::Refused("repository is not in the library".into()));
        };
        let before = capture_undo("", &guard.state);
        let old_key = path_key(&guard.state.repos[idx].path);
        guard.state.repos[idx].path = canonical.clone();
        guard.state.repos[idx].name = repo_display_name(&canonical);
        guard.state.repos[idx].cached = Some(inspect_repo_summary(Path::new(&canonical), None));
        for p in &mut guard.state.tabs.open_paths {
            if path_key(p) == old_key {
                *p = canonical.clone();
            }
        }
        if guard
            .state
            .tabs
            .active_path
            .as_deref()
            .is_some_and(|p| path_key(p) == old_key)
        {
            guard.state.tabs.active_path = Some(canonical);
        }
        let label = format!("Locate {}", guard.state.repos[idx].name);
        push_undo(&mut guard, label, before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Moves a repository to `folder_id` (or root when `None`) at `order`.
    ///
    /// # Errors
    /// [`Error::Refused`] if the target folder or repository doesn't exist.
    pub fn move_repo(
        &self,
        id: &str,
        folder_id: Option<String>,
        order: usize,
    ) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        if let Some(ref fid) = folder_id
            && !guard.state.folders.iter().any(|f| &f.id == fid)
        {
            return Err(Error::Refused("target folder does not exist".into()));
        }
        let Some(pos) = guard.state.repos.iter().position(|r| r.id == id) else {
            return Err(Error::Refused("repository is not in the library".into()));
        };
        if guard.state.repos[pos].folder_id == folder_id && guard.state.repos[pos].order == order {
            return Ok(make_snapshot(&guard));
        }
        let before = capture_undo("", &guard.state);
        let name = guard.state.repos[pos].name.clone();
        guard.state.repos[pos].folder_id = folder_id.clone();
        reorder_repos_in_folder(&mut guard.state.repos, folder_id.as_deref(), id, order);
        push_undo(&mut guard, format!("Move {name}"), before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Pins or unpins a repository in the Library.
    ///
    /// # Errors
    /// [`Error::Refused`] if `id` is unknown.
    pub fn pin_repo(&self, id: &str, pinned: bool) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        let Some(pos) = guard.state.repos.iter().position(|r| r.id == id) else {
            return Err(Error::Refused("repository is not in the library".into()));
        };
        if guard.state.repos[pos].pinned == pinned {
            return Ok(make_snapshot(&guard));
        }
        let before = capture_undo("", &guard.state);
        let name = guard.state.repos[pos].name.clone();
        guard.state.repos[pos].pinned = pinned;
        let label = if pinned {
            format!("Pin {name}")
        } else {
            format!("Unpin {name}")
        };
        push_undo(&mut guard, label, before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Updates `last_opened` when a repository is opened in a tab.
    ///
    /// # Errors
    /// [`Error::Read`] when writing fails.
    pub fn touch_repo(&self, path: &str) -> Result<(), Error> {
        let key = path_key(&display_path(Path::new(path)));
        let mut guard = self.lock()?;
        if let Some(repo) = guard
            .state
            .repos
            .iter_mut()
            .find(|r| path_key(&r.path) == key)
        {
            repo.last_opened = Some(now_secs());
            save_atomic(&self.file, &guard.state)?;
        }
        Ok(())
    }

    /// Creates a new folder under `parent_id` (or root when `None`).
    ///
    /// # Errors
    /// [`Error::Refused`] when `name` is empty or `parent_id` is missing.
    pub fn create_folder(
        &self,
        name: &str,
        parent_id: Option<String>,
    ) -> Result<LibrarySnapshot, Error> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(Error::Refused("folder name can't be empty".into()));
        }
        let mut guard = self.lock()?;
        if let Some(ref pid) = parent_id
            && !guard.state.folders.iter().any(|f| &f.id == pid)
        {
            return Err(Error::Refused("parent folder does not exist".into()));
        }
        let before = capture_undo("", &guard.state);
        let id_num = alloc_id(&mut guard.next_id);
        let order = guard
            .state
            .folders
            .iter()
            .filter(|f| f.parent_id == parent_id)
            .map(|f| f.order)
            .max()
            .map_or(0, |m| m + 1);
        guard.state.folders.push(LibraryFolder {
            id: format!("f-{id_num}"),
            name: trimmed.to_owned(),
            parent_id,
            order,
        });
        push_undo(&mut guard, format!("Create folder {trimmed}"), before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Renames an existing folder.
    ///
    /// # Errors
    /// [`Error::Refused`] when `new_name` is empty or `id` doesn't exist.
    pub fn rename_folder(&self, id: &str, new_name: &str) -> Result<LibrarySnapshot, Error> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(Error::Refused("folder name can't be empty".into()));
        }
        let mut guard = self.lock()?;
        let Some(pos) = guard.state.folders.iter().position(|f| f.id == id) else {
            return Err(Error::Refused("folder does not exist".into()));
        };
        if guard.state.folders[pos].name == trimmed {
            return Ok(make_snapshot(&guard));
        }
        let before = capture_undo("", &guard.state);
        let old = guard.state.folders[pos].name.clone();
        guard.state.folders[pos].name = trimmed.to_owned();
        push_undo(&mut guard, format!("Rename {old} to {trimmed}"), before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Deletes a folder and re-parents its child folders and repos to its parent.
    ///
    /// # Errors
    /// [`Error::Refused`] if `id` doesn't exist.
    pub fn delete_folder(&self, id: &str) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        let Some(pos) = guard.state.folders.iter().position(|f| f.id == id) else {
            return Err(Error::Refused("folder does not exist".into()));
        };
        let before = capture_undo("", &guard.state);
        let removed = guard.state.folders.remove(pos);
        for f in &mut guard.state.folders {
            if f.parent_id.as_deref() == Some(id) {
                f.parent_id.clone_from(&removed.parent_id);
            }
        }
        for r in &mut guard.state.repos {
            if r.folder_id.as_deref() == Some(id) {
                r.folder_id.clone_from(&removed.parent_id);
            }
        }
        push_undo(
            &mut guard,
            format!("Delete folder {}", removed.name),
            before,
        );
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Moves a folder under `parent_id` at `order`, rejecting cycles.
    ///
    /// # Errors
    /// [`Error::Refused`] when `id` or `parent_id` doesn't exist, or moving would form a cycle.
    pub fn move_folder(
        &self,
        id: &str,
        parent_id: Option<String>,
        order: usize,
    ) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        if parent_id.as_deref() == Some(id) {
            return Err(Error::Refused("cannot move a folder inside itself".into()));
        }
        if let Some(ref pid) = parent_id {
            if !guard.state.folders.iter().any(|f| &f.id == pid) {
                return Err(Error::Refused("target folder does not exist".into()));
            }
            if is_descendant(&guard.state.folders, id, pid) {
                return Err(Error::Refused(
                    "cannot move a folder inside its own subfolder".into(),
                ));
            }
        }
        let Some(pos) = guard.state.folders.iter().position(|f| f.id == id) else {
            return Err(Error::Refused("folder does not exist".into()));
        };
        let before = capture_undo("", &guard.state);
        let name = guard.state.folders[pos].name.clone();
        guard.state.folders[pos].parent_id = parent_id.clone();
        reorder_folders_in_parent(&mut guard.state.folders, parent_id.as_deref(), id, order);
        push_undo(&mut guard, format!("Move folder {name}"), before);
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Saves the open tab list and active tab for restoring on next launch.
    ///
    /// # Errors
    /// [`Error::Read`] when writing fails.
    pub fn save_tabs(&self, tabs: OpenTabState) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        guard.state.tabs = tabs;
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Undoes the last Library mutation (folder or repo change).
    ///
    /// # Errors
    /// [`Error::Refused`] when there is nothing to undo.
    pub fn undo(&self) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        let Some(entry) = guard.undo_stack.pop() else {
            return Err(Error::Refused("nothing to undo in Library".into()));
        };
        let redo_entry = UndoSnapshot {
            action: entry.action,
            folders: guard.state.folders.clone(),
            repos: guard.state.repos.clone(),
        };
        guard.redo_stack.push(redo_entry);
        guard.state.folders = entry.folders;
        guard.state.repos = entry.repos;
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Redoes the last undone Library mutation.
    ///
    /// # Errors
    /// [`Error::Refused`] when there is nothing to redo.
    pub fn redo(&self) -> Result<LibrarySnapshot, Error> {
        let mut guard = self.lock()?;
        let Some(entry) = guard.redo_stack.pop() else {
            return Err(Error::Refused("nothing to redo in Library".into()));
        };
        let undo_entry = UndoSnapshot {
            action: entry.action,
            folders: guard.state.folders.clone(),
            repos: guard.state.repos.clone(),
        };
        guard.undo_stack.push(undo_entry);
        guard.state.folders = entry.folders;
        guard.state.repos = entry.repos;
        save_atomic(&self.file, &guard.state)?;
        Ok(make_snapshot(&guard))
    }

    /// Refreshes local-only summaries for `paths` (or all repos if empty) across a bounded worker
    /// pool, updating cached summaries on disk without touching any remote.
    ///
    /// # Errors
    /// [`Error::Read`] if writing the updated cache fails.
    pub fn refresh_summaries(&self, paths: &[String]) -> Result<Vec<RepoSummary>, Error> {
        type RefreshTarget = (String, Option<(usize, Option<i64>)>);
        let targets: Vec<RefreshTarget> = {
            let guard = self.lock()?;
            let mut seen = HashSet::new();
            let filter_keys: HashSet<String> = paths.iter().map(|p| path_key(p)).collect();
            let mut list = Vec::new();
            for r in &guard.state.repos {
                let k = path_key(&r.path);
                if (!filter_keys.is_empty() && !filter_keys.contains(&k)) || !seen.insert(k) {
                    continue;
                }
                let prev = r
                    .cached
                    .as_ref()
                    .map(|c| (c.locks_held, c.locks_checked_at));
                list.push((r.path.clone(), prev));
            }
            // Allow ad-hoc paths not yet persisted if explicitly asked.
            for p in paths {
                let k = path_key(p);
                if seen.insert(k) {
                    list.push((p.clone(), None));
                }
            }
            list
        };

        if targets.is_empty() {
            return Ok(Vec::new());
        }

        let results: Mutex<Vec<Option<RepoSummary>>> = Mutex::new(vec![None; targets.len()]);
        let next = AtomicUsize::new(0);
        let workers = targets.len().clamp(1, REFRESH_THREADS);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| {
                    loop {
                        let idx = next.fetch_add(1, Ordering::Relaxed);
                        if idx >= targets.len() {
                            break;
                        }
                        let (ref raw_path, prev) = targets[idx];
                        let summary = inspect_repo_summary(Path::new(raw_path), prev);
                        if let Ok(mut out) = results.lock() {
                            out[idx] = Some(summary);
                        }
                    }
                });
            }
        });

        let summaries: Vec<RepoSummary> = results
            .into_inner()
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .collect();

        let by_key: BTreeMap<String, &RepoSummary> =
            summaries.iter().map(|s| (path_key(&s.path), s)).collect();
        let mut guard = self.lock()?;
        let mut updated = false;
        for repo in &mut guard.state.repos {
            if let Some(summary) = by_key.get(&path_key(&repo.path)) {
                repo.cached = Some((*summary).clone());
                updated = true;
            }
        }
        if updated {
            save_atomic(&self.file, &guard.state)?;
        }
        Ok(summaries)
    }

    /// Updates cached LFS lock counts for `repo_path` in the Library cache.
    ///
    /// # Errors
    /// [`Error::Read`] when writing fails.
    pub fn update_cached_locks(
        &self,
        repo_path: &str,
        locks_held: usize,
        checked_at: Option<i64>,
    ) -> Result<(), Error> {
        let key = path_key(&display_path(Path::new(repo_path)));
        let mut guard = self.lock()?;
        let Some(repo) = guard
            .state
            .repos
            .iter_mut()
            .find(|r| path_key(&r.path) == key)
        else {
            return Ok(());
        };
        let mut summary = repo
            .cached
            .clone()
            .unwrap_or_else(|| inspect_repo_summary(Path::new(&repo.path), None));
        summary.locks_held = locks_held;
        summary.locks_checked_at = checked_at;
        repo.cached = Some(summary);
        save_atomic(&self.file, &guard.state)?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StoreInner>, Error> {
        self.inner
            .lock()
            .map_err(|_| Error::Read("library lock poisoned".into()))
    }
}

/// Checks whether `root` is a Unity project (`ProjectSettings/ProjectVersion.txt` at the root
/// or one level down).
#[must_use]
pub fn is_unity_project(root: &Path) -> bool {
    if root.join("ProjectSettings/ProjectVersion.txt").is_file() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let p = entry.path();
        if p.is_dir() && p.join("ProjectSettings/ProjectVersion.txt").is_file() {
            return true;
        }
    }
    false
}

/// Computes a repository summary using only local files and `gix` (never network).
#[must_use]
pub fn inspect_repo_summary(path: &Path, prev_locks: Option<(usize, Option<i64>)>) -> RepoSummary {
    let display = display_path(path);
    let checked_at = now_secs();
    let (locks_held, locks_checked_at) = prev_locks.unwrap_or((0, None));

    let Ok(repo) = repo::open(path) else {
        return RepoSummary {
            path: display,
            missing: true,
            branch: None,
            detached: false,
            dirty_count: 0,
            ahead: 0,
            behind: 0,
            fetched_at: None,
            locks_held,
            locks_checked_at,
            last_activity: None,
            host: HostKind::None,
            is_unity: is_unity_project(path),
            checked_at,
        };
    };

    let status = repo.status().ok();
    let (branch, detached) = match status.as_ref().map(|s| &s.head) {
        Some(repo::Head::Branch { name } | repo::Head::Unborn { name }) => {
            (Some(name.clone()), false)
        }
        Some(repo::Head::Detached { id }) => (Some(id.clone()), true),
        None => (None, false),
    };
    let dirty_count = status.map_or(0, |s| s.entries.len());

    let gix_repo = repo.gix();
    let (ahead, behind) = local_head_ahead_behind(&gix_repo);
    let fetched_at = std::fs::metadata(gix_repo.common_dir().join("FETCH_HEAD"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(system_time_secs);
    let last_activity = gix_repo
        .head_commit()
        .ok()
        .and_then(|c| c.committer().ok().map(|sig| sig.seconds()));
    let host = repo.primary_host();
    let is_unity = is_unity_project(repo.root());

    RepoSummary {
        path: display,
        missing: false,
        branch,
        detached,
        dirty_count,
        ahead,
        behind,
        fetched_at,
        locks_held,
        locks_checked_at,
        last_activity,
        host,
        is_unity,
        checked_at,
    }
}

/// Scans `root` up to `max_depth` directories deep for git repositories, grouping by parent
/// folder and suggesting nested folder names from the disk layout.
///
/// # Errors
/// [`Error::Refused`] if `root` doesn't exist or isn't a directory.
pub fn scan_folder(
    root: &Path,
    existing_paths: &[String],
    max_depth: usize,
) -> Result<ScanResult, Error> {
    if !root.is_dir() {
        return Err(Error::Refused(format!(
            "{} is not an existing folder",
            root.display()
        )));
    }
    let started = Instant::now();
    let canon_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let root_display = display_path(&canon_root);
    let existing: HashSet<String> = existing_paths.iter().map(|p| path_key(p)).collect();

    let mut by_dir: BTreeMap<String, Vec<ScannedRepo>> = BTreeMap::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(canon_root.clone(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if dir.join(".git").exists() {
            let repo_path = display_path(&dir);
            let already = existing.contains(&path_key(&repo_path));
            let name = repo_display_name(&repo_path);
            let parent = dir
                .parent()
                .map_or_else(|| root_display.clone(), display_path);
            by_dir.entry(parent).or_default().push(ScannedRepo {
                name,
                path: repo_path,
                already_in_library: already,
            });
            continue;
        }
        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            if should_skip_scan_dir(&name.to_string_lossy()) {
                continue;
            }
            stack.push((entry.path(), depth + 1));
        }
    }

    let mut total_found = 0usize;
    let mut groups: Vec<ScanGroup> = by_dir
        .into_iter()
        .map(|(dir, mut repos)| {
            repos.sort_by_key(|a| a.name.to_lowercase());
            total_found += repos.len();
            let suggested = suggest_folder_name(Path::new(&root_display), Path::new(&dir));
            ScanGroup {
                dir,
                suggested_folder: suggested,
                repos,
            }
        })
        .collect();
    groups.sort_by_key(|a| a.dir.to_lowercase());

    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(ScanResult {
        root: root_display,
        total_found,
        elapsed_ms,
        groups,
    })
}

fn local_head_ahead_behind(repo: &gix::Repository) -> (u32, u32) {
    let Ok(Some(mut head_ref)) = repo.head_ref() else {
        return (0, 0);
    };
    let Ok(tip) = head_ref.peel_to_id().map(|id| id.detach()) else {
        return (0, 0);
    };
    let Some(Ok(upstream_ref)) =
        repo.branch_remote_tracking_ref_name(head_ref.name(), Direction::Fetch)
    else {
        return (0, 0);
    };
    let Ok(Some(mut u)) = repo.try_find_reference(upstream_ref.as_ref()) else {
        return (0, 0);
    };
    let Ok(up) = u.peel_to_id().map(|id| id.detach()) else {
        return (0, 0);
    };
    let ahead = count_only_in(repo, tip, up).unwrap_or(0);
    let behind = count_only_in(repo, up, tip).unwrap_or(0);
    (ahead, behind)
}

fn should_skip_scan_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "Library"
            | "Temp"
            | "obj"
            | "Obj"
            | "bin"
            | ".vs"
            | ".venv"
            | "$RECYCLE.BIN"
            | "System Volume Information"
    ) || name.starts_with('.')
}

fn suggest_folder_name(root: &Path, group_dir: &Path) -> String {
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty());
    let Ok(rel) = group_dir.strip_prefix(root) else {
        return repo_display_name(&display_path(group_dir));
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(rn) = root_name {
        parts.push(rn);
    }
    for comp in rel.components() {
        if let std::path::Component::Normal(s) = comp {
            parts.push(s.to_string_lossy().into_owned());
        }
    }
    if parts.is_empty() {
        "Repositories".to_owned()
    } else {
        parts.join(" / ")
    }
}

fn ensure_folder_path(guard: &mut StoreInner, folder_path: &str) -> Option<String> {
    let mut parent: Option<String> = None;
    for raw in folder_path.split('/') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(existing) = guard
            .state
            .folders
            .iter()
            .find(|f| f.parent_id == parent && f.name.eq_ignore_ascii_case(name))
        {
            parent = Some(existing.id.clone());
            continue;
        }
        let id = format!("f-{}", alloc_id(&mut guard.next_id));
        let order = guard
            .state
            .folders
            .iter()
            .filter(|f| f.parent_id == parent)
            .map(|f| f.order)
            .max()
            .map_or(0, |m| m + 1);
        guard.state.folders.push(LibraryFolder {
            id: id.clone(),
            name: name.to_owned(),
            parent_id: parent.clone(),
            order,
        });
        parent = Some(id);
    }
    parent
}

fn is_descendant(folders: &[LibraryFolder], ancestor_id: &str, candidate_id: &str) -> bool {
    let mut cur = Some(candidate_id);
    let mut hops = 0usize;
    while let Some(id) = cur {
        if id == ancestor_id {
            return true;
        }
        hops += 1;
        if hops > folders.len() {
            break;
        }
        cur = folders
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| f.parent_id.as_deref());
    }
    false
}

fn reorder_repos_in_folder(
    repos: &mut [LibraryRepo],
    folder_id: Option<&str>,
    moved_id: &str,
    target_order: usize,
) {
    let mut siblings: Vec<usize> = repos
        .iter()
        .enumerate()
        .filter(|(_, r)| r.folder_id.as_deref() == folder_id && r.id != moved_id)
        .map(|(i, _)| i)
        .collect();
    siblings.sort_by_key(|&i| (repos[i].order, repos[i].name.to_lowercase()));
    let moved_idx = repos
        .iter()
        .position(|r| r.id == moved_id)
        .expect("moved repo exists");
    let pos = target_order.min(siblings.len());
    siblings.insert(pos, moved_idx);
    for (order, idx) in siblings.into_iter().enumerate() {
        repos[idx].order = order;
    }
}

fn reorder_folders_in_parent(
    folders: &mut [LibraryFolder],
    parent_id: Option<&str>,
    moved_id: &str,
    target_order: usize,
) {
    let mut siblings: Vec<usize> = folders
        .iter()
        .enumerate()
        .filter(|(_, f)| f.parent_id.as_deref() == parent_id && f.id != moved_id)
        .map(|(i, _)| i)
        .collect();
    siblings.sort_by_key(|&i| (folders[i].order, folders[i].name.to_lowercase()));
    let moved_idx = folders
        .iter()
        .position(|f| f.id == moved_id)
        .expect("moved folder exists");
    let pos = target_order.min(siblings.len());
    siblings.insert(pos, moved_idx);
    for (order, idx) in siblings.into_iter().enumerate() {
        folders[idx].order = order;
    }
}

fn capture_undo(action: &str, state: &LibraryState) -> UndoSnapshot {
    UndoSnapshot {
        action: action.to_owned(),
        folders: state.folders.clone(),
        repos: state.repos.clone(),
    }
}

fn push_undo(guard: &mut StoreInner, action: String, mut before: UndoSnapshot) {
    before.action = action;
    guard.undo_stack.push(before);
    if guard.undo_stack.len() > MAX_UNDO {
        guard.undo_stack.remove(0);
    }
    guard.redo_stack.clear();
}

fn make_snapshot(guard: &StoreInner) -> LibrarySnapshot {
    LibrarySnapshot {
        state: guard.state.clone(),
        undo_action: guard.undo_stack.last().map(|u| u.action.clone()),
        redo_action: guard.redo_stack.last().map(|r| r.action.clone()),
    }
}

fn save_atomic(file: &Path, state: &LibraryState) -> Result<(), Error> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(read_err)?;
    }
    let tmp = file.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(state)
        .map_err(|e| Error::Read(format!("serialize library: {e}")))?;
    std::fs::write(&tmp, data).map_err(read_err)?;
    std::fs::rename(&tmp, file).map_err(read_err)?;
    Ok(())
}

fn alloc_id(counter: &mut u64) -> u64 {
    let id = *counter;
    *counter += 1;
    id
}

fn next_numeric_id(state: &LibraryState) -> u64 {
    let max_f = state
        .folders
        .iter()
        .filter_map(|f| f.id.strip_prefix("f-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    let max_r = state
        .repos
        .iter()
        .filter_map(|r| r.id.strip_prefix("r-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    max_f.max(max_r) + 1
}

/// Normalizes a filesystem path for display, removing Windows UNC `\\?\` prefixes and trailing
/// slashes.
#[must_use]
pub fn display_path(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let raw = resolved.to_string_lossy();
    let stripped = if let Some(rest) = raw.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            rest
        } else {
            raw.as_ref()
        }
    } else {
        raw.as_ref()
    };
    let trimmed = stripped.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        stripped.to_owned()
    } else if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
        format!("{trimmed}\\")
    } else {
        trimmed.to_owned()
    }
}

/// Case-folded, slash-normalized comparison key for repository paths.
#[must_use]
pub fn path_key(path: &str) -> String {
    let s = path.trim().replace('\\', "/");
    let stripped = s.strip_prefix("//?/").unwrap_or(&s);
    let trimmed = stripped.trim_end_matches('/');
    #[cfg(windows)]
    {
        trimmed.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        trimmed.to_owned()
    }
}

fn repo_display_name(path: &str) -> String {
    path.split(['/', '\\'])
        .rfind(|s| !s.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn now_secs() -> i64 {
    system_time_secs(SystemTime::now()).unwrap_or(0)
}

fn system_time_secs(t: SystemTime) -> Option<i64> {
    t.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_key_deduplicates_slashes_and_case() {
        let a = path_key(r"\\?\C:\Work\MyRepo\");
        let b = path_key("c:/work/myrepo");
        #[cfg(windows)]
        assert_eq!(a, b);
        #[cfg(not(windows))]
        assert_ne!(a, b);
    }

    #[test]
    fn detect_unity_project_root_and_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_unity_project(tmp.path()));

        let nested = tmp.path().join("UnityClient/ProjectSettings");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("ProjectVersion.txt"),
            "m_EditorVersion: 6000.0.1f1",
        )
        .unwrap();
        assert!(is_unity_project(tmp.path()));
    }
}
