use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::graph::Graph;
use crate::repo::{self, Error, Repo};

/// The repositories the app has open, by id.
#[derive(Default)]
pub struct Session {
    next_id: AtomicU32,
    open: Mutex<HashMap<u32, Arc<OpenRepo>>>,
}

/// A repository with its graph, built when it was opened.
pub struct OpenRepo {
    pub repo: Repo,
    pub graph: Graph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub id: u32,
    pub root: PathBuf,
    /// Graph rows, the working-copy row included.
    pub rows: usize,
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
        let opened = Opened {
            id,
            root: repo.root().to_owned(),
            rows: graph.len(),
        };
        self.lock().insert(id, Arc::new(OpenRepo { repo, graph }));
        Ok(opened)
    }

    /// # Errors
    /// [`Error::UnknownRepo`] when `id` was never opened or is closed.
    pub fn get(&self, id: u32) -> Result<Arc<OpenRepo>, Error> {
        self.lock().get(&id).cloned().ok_or(Error::UnknownRepo(id))
    }

    /// # Errors
    /// [`Error::UnknownRepo`] when `id` was never opened or is already closed.
    pub fn close(&self, id: u32) -> Result<(), Error> {
        self.lock()
            .remove(&id)
            .map(drop)
            .ok_or(Error::UnknownRepo(id))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Arc<OpenRepo>>> {
        self.open.lock().expect("session lock poisoned")
    }
}
