use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::repo::{Error, Repo, read_err};

/// What gitgud remembers about an operation it started and git left stopped,
/// as JSON in `.git/gitgud/<name>` (`merge.json`, `rebase.json`, `sequencer.json`).
pub(crate) struct StateFile(PathBuf);

impl StateFile {
    pub(crate) fn new(repo: &Repo, name: &str) -> Self {
        Self(repo.gix().git_dir().join("gitgud").join(name))
    }

    /// The record, `None` when there is none.
    ///
    /// # Errors
    /// [`Error::Read`] when it exists but can't be read or parsed: what it holds
    /// (conflicts, Smart Merge items to confirm) must not be skipped silently.
    pub(crate) fn load<T: DeserializeOwned>(&self) -> Result<Option<T>, Error> {
        match std::fs::read(&self.0) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::Read(format!("{} can't be read: {e}", self.0.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(read_err(e)),
        }
    }

    pub(crate) fn save<T: Serialize>(&self, value: &T) -> Result<(), Error> {
        let json = serde_json::to_vec(value).map_err(read_err)?;
        crate::atomic_file::write_atomic(&self.0, &json, true).map_err(read_err)
    }

    /// Removes the record; none being there is fine.
    pub(crate) fn forget(&self) -> Result<(), Error> {
        match std::fs::remove_file(&self.0) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_what_it_saved_none_when_forgotten_and_fails_on_garbage() {
        let repo = crate::testing::TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let file = StateFile::new(&r, "test.json");
        assert_eq!(file.load::<Vec<u32>>().unwrap(), None);
        file.save(&vec![1u32, 2]).unwrap();
        assert_eq!(file.load::<Vec<u32>>().unwrap(), Some(vec![1, 2]));
        file.forget().unwrap();
        file.forget().unwrap();
        assert_eq!(file.load::<Vec<u32>>().unwrap(), None);
        std::fs::write(repo.path().join(".git/gitgud/test.json"), "{ nope").unwrap();
        assert!(file.load::<Vec<u32>>().is_err());
    }
}
