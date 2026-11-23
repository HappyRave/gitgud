use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::meta::Meta;
use super::tree::Files;
use crate::repo::Error;
use crate::unity::find_unity_root;
use crate::unity_yaml::{GuidIndex, repo_guid_index};

/// Resolves Unity GUIDs to repo paths for one version: the working copy's
/// index first, then the `.meta` files of that version when the GUID isn't
/// there or points at a file the version doesn't have.
pub(crate) struct Guids<'a> {
    pub files: &'a Files<'a>,
    index: Arc<GuidIndex>,
    prefix: String,
    at_version: OnceCell<HashMap<String, String>>,
}

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
            if self.files.contains(&p) {
                return Ok(Some(p));
            }
        }
        if self.at_version.get().is_none() {
            let built = self.scan()?;
            let _ = self.at_version.set(built);
        }
        Ok(self.at_version.get().and_then(|m| m.get(&guid)).cloned())
    }

    fn scan(&self) -> Result<HashMap<String, String>, Error> {
        let mut out = HashMap::new();
        for p in self.files.paths() {
            let Some(asset) = p.strip_suffix(".meta") else {
                continue;
            };
            if let Some(text) = self.files.read_text(p)?
                && let Some(g) = Meta::guid_only(&text)
            {
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
}
