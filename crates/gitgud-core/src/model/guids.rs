use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::meta::Meta;
use super::tree::Files;
use crate::repo::{Error, read_err};
use crate::unity::find_unity_root;
use crate::unity_yaml::{GuidIndex, repo_guid_index};

/// Resolves Unity GUIDs to repo paths for one version: the working copy's
/// index first, then the `.meta` files of that version when the GUID isn't
/// there or points at a file the version doesn't have.
pub(crate) struct Guids<'a> {
    pub files: &'a Files<'a>,
    index: Arc<GuidIndex>,
    prefix: String,
    at_version: OnceCell<Arc<HashMap<String, String>>>,
}

/// How many commit trees' `.meta` scans a repo keeps.
const KEPT_TREES: usize = 8;

impl<'a> Guids<'a> {
    pub fn new(files: &'a Files<'a>) -> Self {
        let repo = files.repo;
        let prefix = find_unity_root(repo.root())
            .and_then(|p| {
                p.strip_prefix(repo.root())
                    .ok()
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
            })
            .filter(|p| !p.is_empty())
            .map(|p| format!("{p}/"))
            .unwrap_or_default();
        Self {
            files,
            index: repo_guid_index(repo),
            prefix,
            at_version: OnceCell::new(),
        }
    }

    pub fn path(&self, guid: &str) -> Result<Option<String>, Error> {
        let guid = guid.to_ascii_lowercase();
        if let Some(p) = self.index.resolve_guid(&guid) {
            let p = format!("{}{p}", self.prefix);
            if self.files.has(&p) {
                return Ok(Some(p));
            }
        }
        if self.at_version.get().is_none() {
            let built = self.at_version_guids()?;
            let _ = self.at_version.set(built);
        }
        Ok(self.at_version.get().and_then(|m| m.get(&guid)).cloned())
    }

    /// The version's `.meta` GUIDs; a commit's are kept on the repo, since its files never change.
    fn at_version_guids(&self) -> Result<Arc<HashMap<String, String>>, Error> {
        let Some(tree) = self.files.tree else {
            return Ok(Arc::new(self.scan()?));
        };
        let cache = &self.files.repo.tree_guids;
        if let Some((_, found)) = cache
            .lock()
            .expect("tree guids lock poisoned")
            .iter()
            .find(|(id, _)| *id == tree)
        {
            return Ok(found.clone());
        }
        let built = Arc::new(self.scan()?);
        let mut kept = cache.lock().expect("tree guids lock poisoned");
        if !kept.iter().any(|(id, _)| *id == tree) {
            if kept.len() == KEPT_TREES {
                kept.remove(0);
            }
            kept.push((tree, built.clone()));
        }
        Ok(built)
    }

    fn scan(&self) -> Result<HashMap<String, String>, Error> {
        let mut out = HashMap::new();
        let repo = self.files.repo;
        let gix = repo.gix();
        let mut known = repo
            .meta_blob_guids
            .lock()
            .expect("meta blob guids lock poisoned");
        for (i, p) in self.files.paths().iter().enumerate() {
            let Some(asset) = p.strip_suffix(".meta") else {
                continue;
            };
            let guid = match self.files.blob(i) {
                Some(id) => match known.get(&id) {
                    Some(g) => g.clone(),
                    None => {
                        let blob = gix.find_blob(id).map_err(read_err)?;
                        let head = &blob.data[..blob.data.len().min(1024)];
                        let g = Meta::guid_only(&String::from_utf8_lossy(head));
                        known.insert(id, g.clone());
                        g
                    }
                },
                None => self.files.read_text(p)?.and_then(|t| Meta::guid_only(&t)),
            };
            if let Some(g) = guid {
                out.entry(g).or_insert_with(|| asset.to_owned());
            }
        }
        Ok(out)
    }
}

impl Meta {
    /// The `guid:` line alone, without parsing the whole file.
    pub fn guid_only(text: &str) -> Option<String> {
        text.lines()
            .find_map(|l| l.strip_prefix("guid:"))
            .map(|g| g.trim().to_ascii_lowercase())
            .filter(|g| !g.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Version;
    use crate::testing::TestRepo;

    const A: &str = "aaaa1111000000000000000000000000";
    const B: &str = "bbbb2222000000000000000000000000";

    #[test]
    fn falls_back_to_the_meta_files_of_the_version() {
        let repo = TestRepo::init();
        let meta_a = format!("fileFormatVersion: 2\nguid: {}\n", A.to_ascii_uppercase());
        let meta_b = format!("guid: {B}\n");
        repo.write(
            "Game/ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.1f1\n",
        )
        .write("Game/Assets/Old/hull.png", "x")
        .write("Game/Assets/Old/hull.png.meta", &meta_a)
        .commit_all("one");
        let old = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        repo.git(&["mv", "Game/Assets/Old", "Game/Assets/New"]);
        repo.write("Game/Assets/New/deck.png", "y")
            .write("Game/Assets/New/deck.png.meta", &meta_b)
            .commit_all("two");
        let r = crate::repo::open(repo.path()).unwrap();

        let now = Version::Worktree;
        let files = Files::new(&r, &now).unwrap();
        let g = Guids::new(&files);
        assert_eq!(
            g.path(A).unwrap().as_deref(),
            Some("Game/Assets/New/hull.png")
        );
        assert_eq!(
            g.path(&B.to_ascii_uppercase()).unwrap().as_deref(),
            Some("Game/Assets/New/deck.png")
        );

        let then = Version::Commit(old);
        let files = Files::new(&r, &then).unwrap();
        let g = Guids::new(&files);
        assert_eq!(
            g.path(A).unwrap().as_deref(),
            Some("Game/Assets/Old/hull.png")
        );
        assert_eq!(g.path(B).unwrap(), None);
    }

    #[test]
    fn keeps_a_commits_meta_scan_for_the_next_lookup() {
        let repo = TestRepo::init();
        repo.write("Assets/hull.png.meta", &format!("guid: {A}\n"))
            .commit_all("one");
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        let r = crate::repo::open(repo.path()).unwrap();
        let scans = || {
            r.tree_guids
                .lock()
                .unwrap()
                .iter()
                .map(|(_, m)| m.clone())
                .collect::<Vec<_>>()
        };

        let now = Version::Worktree;
        let files = Files::new(&r, &now).unwrap();
        assert_eq!(Guids::new(&files).path(B).unwrap(), None);
        assert!(
            scans().is_empty(),
            "the working copy can change, so it isn't kept"
        );

        let at = Version::Commit(head);
        let files = Files::new(&r, &at).unwrap();
        assert_eq!(Guids::new(&files).path(B).unwrap(), None);
        let first = scans();
        assert_eq!(first.len(), 1);
        let files = Files::new(&r, &Version::Head).unwrap();
        assert_eq!(
            Guids::new(&files).path(A).unwrap().as_deref(),
            Some("Assets/hull.png")
        );
        let again = scans();
        assert_eq!(again.len(), 1);
        assert!(Arc::ptr_eq(&first[0], &again[0]));
    }
}
