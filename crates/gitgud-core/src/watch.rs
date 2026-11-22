use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::repo::{Error, Repo, read_err};

/// Quiet time before a burst of changes is reported (spike 4.4).
pub const DEBOUNCE: Duration = Duration::from_millis(150);
/// A change storm is still reported at least this often.
pub const MAX_WAIT: Duration = Duration::from_secs(1);

/// Watches a repository's work tree and refs, and calls back once per burst of
/// changes. Ignored top-level folders (Unity's `Library/`, `Temp/`...) are not
/// watched at all. Stops when dropped.
pub struct RepoWatcher {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
    paused: Arc<AtomicUsize>,
}

/// While alive, changes are not reported: gitgud's own git operations refresh by
/// themselves.
pub struct Paused(Arc<AtomicUsize>);

impl Drop for Paused {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

enum Msg {
    Changed,
    NewTopLevel(PathBuf),
}

impl RepoWatcher {
    /// # Errors
    /// [`Error::Read`] when the OS watch or the ignore rules can't be set up.
    pub fn start(repo: Arc<Repo>, on_change: impl Fn() + Send + 'static) -> Result<Self, Error> {
        let root = repo.root().to_owned();
        let git_dir = repo.gix().git_dir().to_owned();
        let ignored = ignored_top_level(&repo)?;
        let paused = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();

        let filter = Filter {
            root: root.clone(),
            git_dir: git_dir.clone(),
            ignored,
        };
        let handler_paused = paused.clone();
        let handler = move |event: notify::Result<Event>| {
            if handler_paused.load(Ordering::SeqCst) > 0 {
                return;
            }
            if let Ok(event) = event {
                filter.route(&event, &tx);
            }
        };
        let watcher = Arc::new(Mutex::new(
            notify::recommended_watcher(handler).map_err(read_err)?,
        ));

        {
            let mut w = watcher.lock().expect("watcher lock");
            w.watch(&root, RecursiveMode::NonRecursive)
                .map_err(read_err)?;
            w.watch(&git_dir, RecursiveMode::NonRecursive)
                .map_err(read_err)?;
            let refs = git_dir.join("refs");
            if refs.is_dir() {
                w.watch(&refs, RecursiveMode::Recursive).map_err(read_err)?;
            }
            for dir in top_level_dirs(&root)? {
                if !is_ignored_dir(&repo, &dir)? {
                    w.watch(&dir, RecursiveMode::Recursive).map_err(read_err)?;
                }
            }
        }

        let weak = Arc::downgrade(&watcher);
        std::thread::Builder::new()
            .name("gitgud-watch".into())
            .spawn(move || debounce(&rx, &weak, &repo, &on_change))
            .map_err(read_err)?;

        Ok(Self {
            _watcher: watcher,
            paused,
        })
    }

    pub fn pause(&self) -> Paused {
        self.paused.fetch_add(1, Ordering::SeqCst);
        Paused(self.paused.clone())
    }
}

struct Filter {
    root: PathBuf,
    git_dir: PathBuf,
    ignored: HashSet<OsString>,
}

impl Filter {
    fn route(&self, event: &Event, tx: &Sender<Msg>) {
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for path in &event.paths {
            if let Ok(rel) = path.strip_prefix(&self.git_dir) {
                if is_ref_change(rel) {
                    let _ = tx.send(Msg::Changed);
                }
                continue;
            }
            let Ok(rel) = path.strip_prefix(&self.root) else {
                continue;
            };
            let mut parts = rel.components();
            let Some(top) = parts.next() else {
                continue;
            };
            if top.as_os_str() == ".git" || self.ignored.contains(top.as_os_str()) {
                continue;
            }
            let top_level = parts.next().is_none();
            if top_level && matches!(event.kind, EventKind::Create(_)) && path.is_dir() {
                let _ = tx.send(Msg::NewTopLevel(path.clone()));
            }
            let _ = tx.send(Msg::Changed);
        }
    }
}

/// HEAD, the index and refs move on commits, checkouts, fetches and resets; the
/// rest of `.git` (objects, logs, lock files) churns without changing what we show.
fn is_ref_change(rel: &Path) -> bool {
    if rel.extension().is_some_and(|e| e == "lock") {
        return false;
    }
    let first = rel.components().next().and_then(|c| c.as_os_str().to_str());
    matches!(
        first,
        Some("HEAD" | "index" | "packed-refs" | "MERGE_HEAD" | "refs")
    )
}

fn debounce(
    rx: &mpsc::Receiver<Msg>,
    watcher: &Weak<Mutex<RecommendedWatcher>>,
    repo: &Repo,
    on_change: &dyn Fn(),
) {
    let handle = |msg: Msg| {
        if let Msg::NewTopLevel(dir) = msg
            && let Some(w) = watcher.upgrade()
            && matches!(is_ignored_dir(repo, &dir), Ok(false))
        {
            let watched = w
                .lock()
                .expect("watcher lock")
                .watch(&dir, RecursiveMode::Recursive);
            // A folder that vanished again before we got here has nothing to watch.
            if let Err(e) = watched
                && dir.exists()
            {
                eprintln!("gitgud: cannot watch {}: {e}", dir.display());
            }
        }
    };
    while let Ok(first) = rx.recv() {
        handle(first);
        let burst = Instant::now();
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(msg) => {
                    handle(msg);
                    if burst.elapsed() >= MAX_WAIT {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change();
    }
}

fn top_level_dirs(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root).map_err(read_err)? {
        let entry = entry.map_err(read_err)?;
        if entry.file_name() != ".git" && entry.file_type().map_err(read_err)?.is_dir() {
            dirs.push(entry.path());
        }
    }
    Ok(dirs)
}

fn ignored_top_level(repo: &Repo) -> Result<HashSet<OsString>, Error> {
    let mut ignored = HashSet::new();
    for dir in top_level_dirs(repo.root())? {
        if is_ignored_dir(repo, &dir)? {
            ignored.insert(
                dir.file_name()
                    .expect("read_dir entries have names")
                    .to_owned(),
            );
        }
    }
    Ok(ignored)
}

fn is_ignored_dir(repo: &Repo, dir: &Path) -> Result<bool, Error> {
    let gix = repo.gix();
    let index = gix.index_or_empty().map_err(read_err)?;
    let mut excludes = gix
        .excludes(&index, None, Default::default())
        .map_err(read_err)?;
    let rel = dir.strip_prefix(repo.root()).unwrap_or(dir);
    let platform = excludes
        .at_path(rel, Some(gix::index::entry::Mode::DIR))
        .map_err(read_err)?;
    Ok(platform.is_excluded())
}
