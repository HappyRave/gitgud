use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::image::Version;
use crate::lfs::parse_lfs_pointer;
use crate::repo::{Error, Repo, read_err};

/// The MIME type a file is served with on the `gitgud` protocol's `/file` route.
#[must_use]
pub fn file_mime(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "aif" | "aiff" => "audio/aiff",
        "m4a" => "audio/mp4",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "pdf" => "application/pdf",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttc" => "font/collection",
        "csv" | "tsv" => "text/csv; charset=utf-8",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

impl Repo {
    /// `path` at `version` as stored, an LFS pointer swapped for its local object
    /// (downloaded first when it is at most `auto_fetch_max_bytes` and `git` is given).
    ///
    /// # Errors
    /// [`Error::Refused`] when the file isn't in that version or its LFS object isn't
    /// downloaded; [`Error::Read`] otherwise.
    pub fn file_bytes(
        &self,
        git: Option<&crate::cli::GitCli>,
        path: &str,
        version: &Version,
        auto_fetch_max_bytes: u64,
    ) -> Result<Vec<u8>, Error> {
        let bytes = self.file_at(path, version)?;
        self.lfs_resolve_with_fetch(git, path, bytes, auto_fetch_max_bytes)
    }

    /// The LFS object `path` points to at `version`, if it is a pointer.
    ///
    /// # Errors
    /// As [`Repo::file_at`].
    pub fn lfs_pointer_at(
        &self,
        path: &str,
        version: &Version,
    ) -> Result<Option<(String, u64)>, Error> {
        Ok(parse_lfs_pointer(&self.file_at(path, version)?))
    }

    /// Whether a Unity file is stored as text, and the class of its first object.
    ///
    /// # Errors
    /// As [`Repo::file_bytes`].
    pub fn unity_asset_info(&self, path: &str, version: &Version) -> Result<UnityAssetInfo, Error> {
        let bytes = self.file_bytes(None, path, version, 0)?;
        Ok(UnityAssetInfo::read(&bytes))
    }
}

/// What a Unity YAML-extension file holds, for picking its viewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityAssetInfo {
    /// Stored in Unity's binary format (Asset Serialization isn't Force Text).
    pub binary: bool,
    /// Unity class id of the first object (114 MonoBehaviour, 43 Mesh, 21 Material…).
    pub class_id: Option<u32>,
    pub size: u64,
    /// GameObjects in the file (a scene's or prefab's object count).
    pub objects: u32,
}

impl UnityAssetInfo {
    pub(crate) fn read(bytes: &[u8]) -> Self {
        let head = &bytes[..bytes.len().min(64 * 1024)];
        let text = String::from_utf8_lossy(head);
        let binary = head.contains(&0);
        let class_id = if binary {
            None
        } else {
            text.lines()
                .find_map(crate::unity_yaml::parse_doc_header)
                .map(|(class, _, _)| class)
        };
        let objects = if binary {
            0
        } else {
            let marker = b"\n--- !u!1 &";
            bytes.windows(marker.len()).filter(|w| *w == marker).count() as u32
                + u32::from(bytes.starts_with(&marker[1..]))
        };
        Self {
            binary,
            class_id,
            size: bytes.len() as u64,
            objects,
        }
    }
}

/// Where "Open in default app" copies are written for this run of gitgud.
#[must_use]
pub fn open_copies_dir() -> PathBuf {
    std::env::temp_dir()
        .join("gitgud-open")
        .join(std::process::id().to_string())
}

/// Writes `bytes` as a read-only copy named after `path` (with `label` in the name, so
/// two versions can be open at once) and returns where.
///
/// # Errors
/// [`Error::Read`] when the copy can't be written.
pub fn write_open_copy(path: &str, label: &str, bytes: &[u8]) -> Result<PathBuf, Error> {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let safe: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let dir = open_copies_dir();
    std::fs::create_dir_all(&dir).map_err(read_err)?;
    let file = dir.join(if ext.is_empty() {
        format!("{stem} ({safe})")
    } else {
        format!("{stem} ({safe}).{ext}")
    });
    std::fs::write(&file, bytes).map_err(read_err)?;
    let mut perms = std::fs::metadata(&file).map_err(read_err)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file, perms).map_err(read_err)?;
    Ok(file)
}

/// Deletes this run's copies (when gitgud closes).
pub fn clean_open_copies() {
    remove_copies(&open_copies_dir());
}

/// Deletes copies left by runs that didn't close cleanly: folders untouched for a day,
/// so another gitgud still running keeps its own.
pub fn clean_stale_open_copies() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir().join("gitgud-open")) else {
        return;
    };
    let day = std::time::Duration::from_secs(24 * 60 * 60);
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > day);
        if old {
            remove_copies(&entry.path());
        }
    }
}

fn remove_copies(dir: &Path) {
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for f in files.flatten() {
        if let Ok(meta) = f.metadata() {
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(f.path(), perms);
        }
    }
    if let Err(e) = std::fs::remove_dir_all(dir) {
        eprintln!("gitgud: cannot delete {}: {e}", dir.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn tells_text_from_binary_unity_files_and_reads_the_class() {
        let so = UnityAssetInfo::read(
            b"%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!114 &11400000\nMonoBehaviour:\n",
        );
        assert_eq!((so.binary, so.class_id), (false, Some(114)));
        let bin = UnityAssetInfo::read(b"\0\0\0\0\x16\0\0\0unity binary");
        assert_eq!((bin.binary, bin.class_id), (true, None));
        let scene = UnityAssetInfo::read(
            b"--- !u!29 &1\nOcclusionCullingSettings:\n--- !u!1 &5\nGameObject:\n--- !u!4 &6\nTransform:\n--- !u!1 &7\nGameObject:\n--- !u!1001 &8\nPrefabInstance:\n",
        );
        assert_eq!(scene.objects, 2);
        assert_eq!(
            UnityAssetInfo::read(b"--- !u!1 &5\nGameObject:\n").objects,
            1
        );
    }

    #[test]
    fn serves_files_and_writes_open_copies() {
        let repo = TestRepo::init();
        repo.write("Assets/Audio/horn.wav", "RIFF-one")
            .commit_all("one");
        repo.write("Assets/Audio/horn.wav", "RIFF-two");
        let r = crate::repo::open(repo.path()).unwrap();
        assert_eq!(
            r.file_bytes(None, "Assets/Audio/horn.wav", &Version::Head, 0)
                .unwrap(),
            b"RIFF-one"
        );
        assert_eq!(file_mime("x/horn.WAV"), "audio/wav");
        assert_eq!(file_mime("a.woff2"), "font/woff2");
        assert_eq!(file_mime("a.unknown"), "application/octet-stream");

        let copy = write_open_copy("Assets/Audio/horn.wav", "HEAD 1a2b", b"RIFF-one").unwrap();
        assert!(copy.ends_with("horn (HEAD-1a2b).wav"));
        assert_eq!(std::fs::read(&copy).unwrap(), b"RIFF-one");
        assert!(std::fs::metadata(&copy).unwrap().permissions().readonly());
        clean_open_copies();
        assert!(!copy.exists());
    }
}
