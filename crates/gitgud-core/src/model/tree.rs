use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use gix::traverse::tree::Recorder;

use super::cache::Origin;
use crate::cli::GitCli;
use crate::image::Version;
use crate::lfs::{AUTO_FETCH_MAX_BYTES, parse_lfs_pointer};
use crate::repo::{Error, Repo, read_err};

/// The tracked files of one version, for looking up files the asset refers to.
pub(crate) struct Files<'a> {
    pub repo: &'a Repo,
    pub version: &'a Version,
    git: Option<&'a GitCli>,
    /// The version's tree when it is a commit, so what is read from it can be cached.
    pub tree: Option<gix::ObjectId>,
    listed: Arc<Listed>,
    /// Gitignored files used from disk, for the viewer's note.
    from_disk: RefCell<BTreeSet<String>>,
}

/// A version's file list with a lookup by file name; built once per commit tree (and
/// once for the index until the next refresh), since a scene asks for it per model.
pub(crate) struct Listed {
    paths: Vec<String>,
    /// Each path's blob when the version is a commit, so reads skip the tree lookup.
    blobs: Vec<gix::ObjectId>,
    by_name: HashMap<String, Vec<usize>>,
    tree: Option<gix::ObjectId>,
}

/// How many listings a repo keeps: both sides of a few recent comparisons.
const KEPT_LISTINGS: usize = 6;

impl<'a> Files<'a> {
    pub fn new(repo: &'a Repo, version: &'a Version) -> Result<Self, Error> {
        let listed = listed(repo, version)?;
        Ok(Self {
            repo,
            version,
            git: None,
            tree: listed.tree,
            listed,
            from_disk: RefCell::new(BTreeSet::new()),
        })
    }

    /// Whether `path` can be read at this version: it is in it, or it is a gitignored
    /// file on disk, which every version reads from disk.
    pub fn has(&self, path: &str) -> bool {
        if self.contains(path) {
            return true;
        }
        let disk = self.repo.ignored_on_disk(path);
        if disk {
            self.from_disk.borrow_mut().insert(path.to_owned());
        }
        disk
    }

    /// How many gitignored files were used from disk so far.
    pub fn disk_reads(&self) -> usize {
        self.from_disk.borrow().len()
    }

    pub fn paths(&self) -> &[String] {
        &self.listed.paths
    }

    /// The blob of `paths()[i]`, when the version is a commit.
    pub fn blob(&self, i: usize) -> Option<gix::ObjectId> {
        self.listed.blobs.get(i).copied()
    }

    fn index_of(&self, path: &str) -> Option<usize> {
        self.listed
            .by_name
            .get(&base_name(path).to_ascii_lowercase())?
            .iter()
            .copied()
            .find(|&i| self.listed.paths[i] == path)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.index_of(path).is_some()
    }

    /// A name for `path`'s content at this version that changes whenever the
    /// content does, so the viewer can reuse what it loaded: the blob id, or for
    /// the work tree the file's size and modified time. A gitignored file read from
    /// disk gets its disk key in every version, so both sides share one load.
    /// `None` when there is none.
    pub fn key(&self, path: &str) -> Option<String> {
        let disk_key = || {
            let meta = std::fs::metadata(self.repo.root().join(path)).ok()?;
            let modified = meta
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some(format!("wt:{path}:{}:{}", meta.len(), modified.as_nanos()))
        };
        if !self.contains(path) && self.repo.ignored_on_disk(path) {
            return disk_key();
        }
        if !self.listed.blobs.is_empty() {
            return self
                .index_of(path)
                .map(|i| self.listed.blobs[i].to_string());
        }
        match self.version {
            Version::Worktree => disk_key(),
            Version::Index => {
                let index = self.repo.gix().index_or_empty().ok()?;
                index.entry_by_path(path.into()).map(|e| e.id.to_string())
            }
            _ => None,
        }
    }

    /// Lets LFS objects up to the auto-fetch limit be downloaded while reading.
    pub fn with_git(mut self, git: Option<&'a GitCli>) -> Self {
        self.git = git;
        self
    }

    /// `path` at this version with its LFS object in place of the pointer, or
    /// `None` when it isn't there.
    ///
    /// # Errors
    /// [`Error::Refused`] when the LFS object isn't downloaded and can't be fetched.
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let blob = if self.listed.blobs.is_empty() {
            None
        } else {
            match self.index_of(path).and_then(|i| self.blob(i)) {
                Some(id) => Some(id),
                None if self.has(path) => None,
                None => return Ok(None),
            }
        };
        let read = match blob {
            Some(id) => self.repo.blob(id),
            None => self.repo.file_at(path, self.version),
        };
        let bytes = match read {
            Ok(b) => b,
            Err(Error::Refused(_)) => return Ok(None),
            Err(e) => return Err(e),
        };
        if parse_lfs_pointer(&bytes).is_none() {
            return Ok(Some(bytes));
        }
        self.repo
            .lfs_resolve_with_fetch(self.git, path, bytes, AUTO_FETCH_MAX_BYTES)
            .map(Some)
    }

    pub fn read_text(&self, path: &str) -> Result<Option<String>, Error> {
        Ok(self
            .read(path)?
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// Finds the file a model refers to as `reference` (often an absolute path
    /// from the artist's machine): next to `near` first, then the file with the
    /// same name whose folder shares the most with `near`'s.
    pub fn find_near(&self, reference: &str, near: &str) -> Option<String> {
        let name = base_name(reference);
        if name.is_empty() {
            return None;
        }
        let dir = parent(near);
        let relative = join(dir, &reference.replace('\\', "/"));
        if !reference.contains(':') && !reference.starts_with('/') && self.contains(&relative) {
            return Some(relative);
        }
        let ix = self.listed.by_name.get(&name.to_ascii_lowercase())?;
        ix.iter()
            .map(|&i| &self.listed.paths[i])
            .max_by_key(|p| (shared_dirs(parent(p), dir), std::cmp::Reverse(p.len())))
            .cloned()
    }
}

/// What a version reads its files from.
enum At<'r> {
    Commit(gix::Commit<'r>),
    /// No commit: an unborn HEAD, or the parent of a root commit.
    Nothing,
    Index,
}

fn at<'r>(gix_repo: &'r gix::Repository, version: &Version) -> Result<At<'r>, Error> {
    Ok(match version {
        Version::Commit(id) | Version::Parent(id) => {
            let oid = gix::ObjectId::from_hex(id.as_bytes())
                .map_err(|_| Error::Read(format!("{id:?} is not a commit id")))?;
            let c = gix_repo.find_commit(oid).map_err(read_err)?;
            if matches!(version, Version::Parent(_)) {
                match c.parent_ids().next() {
                    Some(p) => At::Commit(p.object().map_err(read_err)?.into_commit()),
                    None => At::Nothing,
                }
            } else {
                At::Commit(c)
            }
        }
        Version::Head => gix_repo.head_commit().map_or(At::Nothing, At::Commit),
        Version::Index | Version::Worktree | Version::Yours | Version::Theirs => At::Index,
    })
}

/// Where `version`'s files come from, without listing them.
///
/// # Errors
/// [`Error::Read`] when the commit can't be read.
pub(crate) fn origin(repo: &Repo, version: &Version) -> Result<Origin, Error> {
    let gix_repo = repo.gix();
    Ok(match at(&gix_repo, version)? {
        At::Commit(c) => Origin::Tree(c.tree_id().map_err(read_err)?.detach()),
        At::Nothing => Origin::Live("none"),
        At::Index => Origin::Live(match version {
            Version::Index => "index",
            Version::Yours => "yours",
            Version::Theirs => "theirs",
            _ => "worktree",
        }),
    })
}

/// `version`'s listing, built on first use: a commit tree's for good, the index's and
/// work tree's until the next refresh.
fn listed(repo: &Repo, version: &Version) -> Result<Arc<Listed>, Error> {
    let key = origin(repo, version)?;
    let hit = repo
        .listings
        .lock()
        .expect("listings lock poisoned")
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| l.clone());
    if let Some(hit) = hit {
        return Ok(hit);
    }
    let Listing { paths, blobs, tree } = paths_at(repo, version)?;
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in paths.iter().enumerate() {
        by_name
            .entry(base_name(p).to_ascii_lowercase())
            .or_default()
            .push(i);
    }
    let built = Arc::new(Listed {
        paths,
        blobs,
        by_name,
        tree,
    });
    let mut kept = repo.listings.lock().expect("listings lock poisoned");
    kept.retain(|(k, _)| *k != key);
    if kept.len() >= KEPT_LISTINGS {
        kept.remove(0);
    }
    kept.push((key, built.clone()));
    Ok(built)
}

fn paths_at(repo: &Repo, version: &Version) -> Result<Listing, Error> {
    let gix_repo = repo.gix();
    let commit = match at(&gix_repo, version)? {
        At::Commit(c) => c,
        At::Nothing => return Ok(Listing::files(Vec::new())),
        At::Index => {
            let index = gix_repo.index_or_empty().map_err(read_err)?;
            let mut out: Vec<String> = index
                .entries()
                .iter()
                .map(|e| e.path(&index).to_string())
                .collect();
            out.dedup();
            return Ok(Listing::files(out));
        }
    };
    let tree = commit.tree().map_err(read_err)?;
    let id = tree.id;
    let mut rec = Recorder::default();
    tree.traverse().breadthfirst(&mut rec).map_err(read_err)?;
    let (paths, blobs) = rec
        .records
        .into_iter()
        .filter(|e| e.mode.is_blob())
        .map(|e| (e.filepath.to_string(), e.oid))
        .unzip();
    Ok(Listing {
        paths,
        blobs,
        tree: Some(id),
    })
}

struct Listing {
    paths: Vec<String>,
    /// Each path's blob, for a commit's tree; empty otherwise.
    blobs: Vec<gix::ObjectId>,
    tree: Option<gix::ObjectId>,
}

impl Listing {
    fn files(paths: Vec<String>) -> Self {
        Self {
            paths,
            blobs: Vec::new(),
            tree: None,
        }
    }
}

pub(crate) fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub(crate) fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// `dir/rel` with `.` and `..` folded.
pub(crate) fn join(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn shared_dirs(a: &str, b: &str) -> usize {
    a.split('/')
        .zip(b.split('/'))
        .take_while(|(x, y)| x.eq_ignore_ascii_case(y))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn finds_referenced_files_near_the_model() {
        let repo = TestRepo::init();
        for p in [
            "Assets/Boat/boat.fbx",
            "Assets/Boat/Textures/hull.png",
            "Assets/Other/hull.png",
            "Assets/Boat/deck.png",
        ] {
            repo.write(p, "x");
        }
        repo.commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        for v in [Version::Head, Version::Worktree] {
            let files = Files::new(&r, &v).unwrap();
            assert_eq!(files.paths().len(), 4);
            assert_eq!(
                files
                    .find_near("C:\\Art\\boat\\hull.png", "Assets/Boat/boat.fbx")
                    .as_deref(),
                Some("Assets/Boat/Textures/hull.png")
            );
            assert_eq!(
                files
                    .find_near("deck.png", "Assets/Boat/boat.fbx")
                    .as_deref(),
                Some("Assets/Boat/deck.png")
            );
            assert_eq!(
                files
                    .find_near("../Other/hull.png", "Assets/Boat/boat.fbx")
                    .as_deref(),
                Some("Assets/Other/hull.png")
            );
            assert_eq!(files.find_near("nope.png", "Assets/Boat/boat.fbx"), None);
        }
    }

    #[test]
    fn joins_relative_paths() {
        assert_eq!(join("Assets/A", "../B/./c.png"), "Assets/B/c.png");
        assert_eq!(parent("a/b/c.fbx"), "a/b");
        assert_eq!(base_name("C:\\x\\y.png"), "y.png");
    }
}
