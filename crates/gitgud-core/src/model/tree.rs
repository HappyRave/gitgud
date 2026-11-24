use std::collections::HashMap;

use gix::traverse::tree::Recorder;

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
    paths: Vec<String>,
    /// Each path's blob when the version is a commit, so reads skip the tree lookup.
    blobs: Vec<gix::ObjectId>,
    by_name: HashMap<String, Vec<usize>>,
}

impl<'a> Files<'a> {
    pub fn new(repo: &'a Repo, version: &'a Version) -> Result<Self, Error> {
        let Listing { paths, blobs, tree } = paths_at(repo, version)?;
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in paths.iter().enumerate() {
            by_name
                .entry(base_name(p).to_ascii_lowercase())
                .or_default()
                .push(i);
        }
        Ok(Self {
            repo,
            version,
            git: None,
            tree,
            paths,
            blobs,
            by_name,
        })
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// The blob of `paths()[i]`, when the version is a commit.
    pub fn blob(&self, i: usize) -> Option<gix::ObjectId> {
        self.blobs.get(i).copied()
    }

    fn index_of(&self, path: &str) -> Option<usize> {
        self.by_name
            .get(&base_name(path).to_ascii_lowercase())?
            .iter()
            .copied()
            .find(|&i| self.paths[i] == path)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.index_of(path).is_some()
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
        let blob = if self.blobs.is_empty() {
            None
        } else {
            match self.index_of(path).and_then(|i| self.blob(i)) {
                Some(id) => Some(id),
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
        let ix = self.by_name.get(&name.to_ascii_lowercase())?;
        ix.iter()
            .map(|&i| &self.paths[i])
            .max_by_key(|p| (shared_dirs(parent(p), dir), std::cmp::Reverse(p.len())))
            .cloned()
    }
}

fn paths_at(repo: &Repo, version: &Version) -> Result<Listing, Error> {
    let gix_repo = repo.gix();
    let commit = match version {
        Version::Commit(id) | Version::Parent(id) => {
            let oid = gix::ObjectId::from_hex(id.as_bytes())
                .map_err(|_| Error::Read(format!("{id:?} is not a commit id")))?;
            let c = gix_repo.find_commit(oid).map_err(read_err)?;
            if matches!(version, Version::Parent(_)) {
                match c.parent_ids().next() {
                    Some(p) => p.object().map_err(read_err)?.into_commit(),
                    None => return Ok(Listing::files(Vec::new())),
                }
            } else {
                c
            }
        }
        Version::Head => match gix_repo.head_commit() {
            Ok(c) => c,
            Err(_) => return Ok(Listing::files(Vec::new())),
        },
        Version::Index | Version::Worktree | Version::Yours | Version::Theirs => {
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
