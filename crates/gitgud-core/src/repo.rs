use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gix::bstr::BStr;
use gix::diff::index::ChangeRef;
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::index_worktree::iter::Summary;
use gix::status::tree_index::TrackRenames;
use gix::status::{Item, UntrackedFiles};
use serde::{Serialize, Serializer};

/// An open repository with a work tree.
pub struct Repo {
    inner: gix::Repository,
}

/// Opens the repository containing `path`, walking up from a subfolder.
///
/// # Errors
/// [`Error::NotARepo`] when no repository contains `path`, [`Error::Bare`] when
/// it has no work tree.
pub fn open(path: impl AsRef<Path>) -> Result<Repo, Error> {
    let path = path.as_ref();
    let inner = gix::discover(path).map_err(|_| Error::NotARepo(path.to_owned()))?;
    if inner.workdir().is_none() {
        return Err(Error::Bare(path.to_owned()));
    }
    Ok(Repo { inner })
}

impl Repo {
    /// The work tree root.
    pub fn root(&self) -> &Path {
        self.inner
            .workdir()
            .expect("open() rejects repositories without a work tree")
    }

    /// The branch and file changes, like `git status --untracked-files=all --no-renames`.
    ///
    /// # Errors
    /// [`Error::Read`] when gix can't read HEAD, the index or the work tree.
    pub fn status(&self) -> Result<Status, Error> {
        Ok(Status {
            root: self.root().to_owned(),
            head: self.head()?,
            entries: self.entries()?,
        })
    }

    fn head(&self) -> Result<Head, Error> {
        let head = self.inner.head().map_err(read_err)?;
        Ok(match head.kind {
            gix::head::Kind::Symbolic(r) => Head::Branch {
                name: r.name.shorten().to_string(),
            },
            gix::head::Kind::Unborn(name) => Head::Unborn {
                name: name.shorten().to_string(),
            },
            gix::head::Kind::Detached { target, .. } => Head::Detached {
                id: target.to_hex_with_len(7).to_string(),
            },
        })
    }

    fn entries(&self) -> Result<Vec<Entry>, Error> {
        let iter = self
            .inner
            .status(gix::progress::Discard)
            .map_err(read_err)?
            .untracked_files(UntrackedFiles::Files)
            .index_worktree_rewrites(None)
            .tree_index_track_renames(TrackRenames::Disabled)
            .into_iter(None)
            .map_err(read_err)?;

        let mut by_path: BTreeMap<String, Entry> = BTreeMap::new();
        for item in iter {
            match item.map_err(read_err)? {
                Item::TreeIndex(change) => {
                    let kind = match &change {
                        ChangeRef::Addition { .. } => Change::Added,
                        ChangeRef::Deletion { .. } => Change::Deleted,
                        ChangeRef::Modification {
                            previous_entry_mode,
                            entry_mode,
                            ..
                        } if previous_entry_mode != entry_mode => Change::TypeChange,
                        ChangeRef::Modification { .. } => Change::Modified,
                        ChangeRef::Rewrite { .. } => {
                            return Err(Error::Read(
                                "rename reported with renames disabled".into(),
                            ));
                        }
                    };
                    entry(&mut by_path, change.location()).index = Some(kind);
                }
                Item::IndexWorktree(item) => {
                    let Some(summary) = item.summary() else {
                        continue;
                    };
                    let e = entry(&mut by_path, item.rela_path());
                    match summary {
                        Summary::Added
                            if matches!(item, WorktreeItem::DirectoryContents { .. }) =>
                        {
                            e.kind = EntryKind::Untracked;
                        }
                        Summary::Conflict => e.kind = EntryKind::Conflicted,
                        Summary::Removed => e.worktree = Some(Change::Deleted),
                        Summary::Modified => e.worktree = Some(Change::Modified),
                        Summary::TypeChange => e.worktree = Some(Change::TypeChange),
                        Summary::IntentToAdd | Summary::Added => e.worktree = Some(Change::Added),
                        Summary::Renamed | Summary::Copied => {
                            return Err(Error::Read(
                                "rename reported with renames disabled".into(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(by_path
            .into_values()
            .map(|mut e| {
                if e.kind == EntryKind::Conflicted {
                    e.index = None;
                    e.worktree = None;
                }
                e
            })
            .collect())
    }
}

fn entry<'a>(by_path: &'a mut BTreeMap<String, Entry>, path: &BStr) -> &'a mut Entry {
    let path = path.to_string();
    by_path.entry(path.clone()).or_insert(Entry {
        path,
        kind: EntryKind::Tracked,
        index: None,
        worktree: None,
    })
}

fn read_err(err: impl std::error::Error) -> Error {
    Error::Read(err.to_string())
}

/// A repository's branch and changed files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub root: PathBuf,
    pub head: Head,
    /// Sorted by path.
    pub entries: Vec<Entry>,
}

/// What HEAD points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Head {
    Branch {
        name: String,
    },
    /// A branch with no commits yet.
    Unborn {
        name: String,
    },
    /// A commit, by its 7-character id.
    Detached {
        id: String,
    },
}

/// One changed path, relative to the work tree root with `/` separators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: EntryKind,
    /// HEAD to index. Always `None` for untracked and conflicted entries.
    pub index: Option<Change>,
    /// Index to work tree. Always `None` for untracked and conflicted entries.
    pub worktree: Option<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Tracked,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    Added,
    Modified,
    Deleted,
    TypeChange,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} is not inside a git repository")]
    NotARepo(PathBuf),
    #[error("{0} is a bare repository; gitgud needs a work tree")]
    Bare(PathBuf),
    #[error("could not read the repository: {0}")]
    Read(String),
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
