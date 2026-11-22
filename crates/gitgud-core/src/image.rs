use std::path::Path;

use gix::bstr::BStr;
use serde::Deserialize;

use crate::diff::blob_at;
use crate::repo::{Error, Repo, read_err};

/// Which version of a file to read, as the image diff names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Version {
    /// In commit `0`.
    Commit(String),
    /// In the first parent of commit `0`.
    Parent(String),
    Head,
    Index,
    /// The file on disk, as is.
    Worktree,
    /// Stage 2 of a conflicted path.
    Yours,
    /// Stage 3 of a conflicted path.
    Theirs,
}

/// The MIME type of image files the webview can show, by extension.
#[must_use]
pub fn image_type(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";

impl Repo {
    /// The bytes of image `path` at `version`; an LFS pointer is swapped for its
    /// local object.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't an image, doesn't exist in that
    /// version, or its LFS object isn't downloaded; [`Error::Read`] otherwise.
    pub fn image_bytes(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        if image_type(path).is_none() {
            return Err(Error::Refused(format!("{path} is not an image")));
        }
        let inside = Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !inside {
            return Err(Error::Refused(format!(
                "{path} is not a path inside the repo"
            )));
        }
        let missing = || Error::Refused(format!("{path} is not in that version"));
        let data = match version {
            Version::Worktree => {
                return match std::fs::read(self.root().join(path)) {
                    Ok(d) => Ok(d),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(missing()),
                    Err(e) => Err(read_err(e)),
                };
            }
            Version::Commit(id) | Version::Parent(id) => {
                let repo = self.gix();
                let oid = gix::ObjectId::from_hex(id.as_bytes())
                    .map_err(|_| Error::Read(format!("{id:?} is not a commit id")))?;
                let mut commit = repo.find_commit(oid).map_err(read_err)?;
                if matches!(version, Version::Parent(_)) {
                    let Some(parent) = commit.parent_ids().next() else {
                        return Err(missing());
                    };
                    commit = parent.object().map_err(read_err)?.into_commit();
                }
                blob_at(&repo, &commit, path)?
            }
            Version::Head => {
                let repo = self.gix();
                match repo.head_commit() {
                    Ok(commit) => blob_at(&repo, &commit, path)?,
                    Err(_) if repo.head().map_err(read_err)?.is_unborn() => None,
                    Err(e) => return Err(read_err(e)),
                }
            }
            Version::Index => {
                let repo = self.gix();
                let index = repo.index_or_empty().map_err(read_err)?;
                match index.entry_by_path(BStr::new(path)) {
                    Some(e) => Some(repo.find_blob(e.id).map_err(read_err)?.data.clone()),
                    None => None,
                }
            }
            Version::Yours | Version::Theirs => {
                let [_, yours, theirs] = self.stages(path)?;
                if matches!(version, Version::Yours) {
                    yours
                } else {
                    theirs
                }
            }
        }
        .ok_or_else(missing)?;
        self.lfs_resolve(path, data)
    }

    fn lfs_resolve(&self, path: &str, data: Vec<u8>) -> Result<Vec<u8>, Error> {
        if !data.starts_with(LFS_POINTER) {
            return Ok(data);
        }
        let oid = String::from_utf8_lossy(&data)
            .lines()
            .find_map(|l| l.strip_prefix("oid sha256:").map(str::to_owned))
            .filter(|o| o.len() == 64 && o.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| Error::Read(format!("{path} has a malformed LFS pointer")))?;
        let object = self
            .gix()
            .common_dir()
            .join("lfs/objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(&oid);
        match std::fs::read(&object) {
            Ok(d) => Ok(d),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Refused(format!(
                "{path}: this LFS version isn't downloaded"
            ))),
            Err(e) => Err(read_err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_types_by_extension() {
        assert_eq!(image_type("a/b.PNG"), Some("image/png"));
        assert_eq!(image_type("t.jpeg"), Some("image/jpeg"));
        assert_eq!(image_type("Ship.prefab"), None);
        assert_eq!(image_type("noext"), None);
    }
}
