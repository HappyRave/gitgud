use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::model::ModelInfo;
use crate::repo::Repo;

/// How much the preview cache keeps on disk; past it, the least recently used files go.
pub const BUDGET: u64 = 1024 * 1024 * 1024;

/// Bumped whenever what is kept changes shape or meaning, so older files aren't read.
const FORMAT: u32 = 3;

/// Slow 3D preview results kept on disk across restarts, in `.git/gitgud/cache`:
/// scene and prefab infos at commits (a commit never changes) and shrunk textures
/// (by content). Keys include the app version, so a new build never reads an old
/// builder's results.
pub(crate) struct DiskCache {
    root: PathBuf,
}

#[derive(Serialize)]
struct StoredRef<'a> {
    tree: &'a str,
    path: &'a str,
    info: &'a ModelInfo,
}

#[derive(Deserialize)]
struct Stored {
    tree: String,
    path: String,
    info: ModelInfo,
}

/// What the cache holds, for Settings › Caches.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiskStats {
    pub models: usize,
    pub textures: usize,
    pub bytes: u64,
}

impl DiskCache {
    pub fn new(repo: &Repo) -> Self {
        Self {
            root: repo.git_dir().join("gitgud").join("cache"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn models(&self) -> PathBuf {
        self.root.join("models")
    }

    fn textures(&self) -> PathBuf {
        self.root.join("textures")
    }

    fn model_file(&self, tree: &str, path: &str) -> PathBuf {
        let key = xxhash_rust::xxh64::xxh64(
            format!("{}\0{FORMAT}\0{tree}\0{path}", env!("CARGO_PKG_VERSION")).as_bytes(),
            0,
        );
        self.models().join(format!("{key:016x}.json"))
    }

    fn texture_file(&self, key: u64) -> PathBuf {
        self.textures().join(format!(
            "{}-{FORMAT}-{key:016x}.tex",
            env!("CARGO_PKG_VERSION")
        ))
    }

    /// The info of the model `path` in commit tree `tree`, when it was kept.
    pub fn model_info(&self, tree: &str, path: &str) -> Option<ModelInfo> {
        let file = self.model_file(tree, path);
        let bytes = read(&file)?;
        match serde_json::from_slice::<Stored>(&bytes) {
            Ok(s) if s.tree == tree && s.path == path => Some(s.info),
            Ok(_) => None,
            Err(e) => {
                eprintln!("gitgud: unreadable cache file {}: {e}", file.display());
                None
            }
        }
    }

    pub fn put_model_info(&self, tree: &str, path: &str, info: &ModelInfo) {
        match serde_json::to_vec(&StoredRef { tree, path, info }) {
            Ok(bytes) => write(&self.model_file(tree, path), &bytes),
            Err(e) => eprintln!("gitgud: cannot cache the model info of {path}: {e}"),
        }
    }

    /// A prepared texture (shrunk PNG or compressed DDS) by its key: `Some(None)` when
    /// it was found to be best used as is.
    pub fn texture(&self, key: u64) -> Option<Option<Vec<u8>>> {
        read(&self.texture_file(key)).map(|b| (!b.is_empty()).then_some(b))
    }

    pub fn put_texture(&self, key: u64, png: Option<&[u8]>) {
        write(&self.texture_file(key), png.unwrap_or_default());
    }

    pub fn stats(&self) -> DiskStats {
        let mut stats = DiskStats::default();
        for (f, _, size) in files(&self.models()) {
            stats.models += usize::from(f.extension().is_some_and(|e| e == "json"));
            stats.bytes += size;
        }
        for (_, _, size) in files(&self.textures()) {
            stats.textures += 1;
            stats.bytes += size;
        }
        stats
    }

    /// Deletes the least recently used files until the cache fits in `budget` bytes.
    pub fn prune(&self, budget: u64) {
        let mut all = files(&self.models());
        all.extend(files(&self.textures()));
        let mut total: u64 = all.iter().map(|f| f.2).sum();
        if total <= budget {
            return;
        }
        all.sort_by_key(|f| f.1);
        for (file, _, size) in all {
            if total <= budget {
                break;
            }
            match fs::remove_file(&file) {
                Ok(()) => total -= size,
                Err(e) => eprintln!("gitgud: cannot delete {}: {e}", file.display()),
            }
        }
    }

    /// Deletes everything the cache holds.
    ///
    /// # Errors
    /// The I/O error when the folder can't be deleted.
    pub fn clear(&self) -> std::io::Result<()> {
        match fs::remove_dir_all(&self.root) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// A cached file's bytes; a hit counts as a use, so pruning keeps it longer.
fn read(file: &Path) -> Option<Vec<u8>> {
    match fs::read(file) {
        Ok(bytes) => {
            if let Ok(f) = fs::File::options().write(true).open(file) {
                let _ = f.set_modified(SystemTime::now());
            }
            Some(bytes)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("gitgud: cannot read cache file {}: {e}", file.display());
            None
        }
    }
}

/// Writes through a temporary file, so a concurrent reader never sees half a file.
fn write(file: &Path, bytes: &[u8]) {
    let tmp = file.with_extension(format!("{}.tmp", std::process::id()));
    let done = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&tmp, bytes))
        .and_then(|()| fs::rename(&tmp, file));
    if let Err(e) = done {
        let _ = fs::remove_file(&tmp);
        eprintln!("gitgud: cannot write cache file {}: {e}", file.display());
    }
}

/// Files in `dir` with their last use and size; none when it doesn't exist.
fn files(dir: &Path) -> Vec<(PathBuf, SystemTime, u64)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok().filter(fs::Metadata::is_file)?;
            Some((
                e.path(),
                m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                m.len(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ModelKind, ModelScene};
    use crate::testing::TestRepo;
    use std::collections::BTreeMap;

    fn info() -> ModelInfo {
        ModelInfo {
            kind: ModelKind::Prefab,
            key: Some("abc".into()),
            materials: Vec::new(),
            remaps: BTreeMap::new(),
            scale: 1.0,
            scene: Some(ModelScene::default()),
            from_disk: 2,
            models: BTreeMap::new(),
        }
    }

    #[test]
    fn keeps_model_infos_and_textures_until_cleared() {
        let repo = TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let cache = DiskCache::new(&r);
        assert_eq!(cache.stats(), DiskStats::default());
        assert!(cache.model_info("t1", "a.prefab").is_none());

        cache.put_model_info("t1", "a.prefab", &info());
        assert_eq!(cache.model_info("t1", "a.prefab"), Some(info()));
        assert!(
            cache.model_info("t2", "a.prefab").is_none(),
            "another commit"
        );
        assert!(cache.model_info("t1", "b.prefab").is_none());

        assert_eq!(cache.texture(7), None);
        cache.put_texture(7, Some(b"png"));
        cache.put_texture(8, None);
        assert_eq!(cache.texture(7), Some(Some(b"png".to_vec())));
        assert_eq!(cache.texture(8), Some(None), "kept as: use the original");

        let stats = cache.stats();
        assert_eq!((stats.models, stats.textures), (1, 2));
        assert!(stats.bytes > 3);
        cache.clear().unwrap();
        assert_eq!(cache.stats(), DiskStats::default());
        cache.clear().unwrap();
    }

    #[test]
    fn prunes_the_least_recently_used_files_first() {
        let repo = TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let cache = DiskCache::new(&r);
        let old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        for key in 0..3 {
            cache.put_texture(key, Some(&[0; 100]));
            fs::File::options()
                .write(true)
                .open(cache.texture_file(key))
                .unwrap()
                .set_modified(old + std::time::Duration::from_secs(key))
                .unwrap();
        }
        assert!(cache.texture(0).is_some(), "used just now");
        cache.prune(200);
        assert!(cache.texture(0).is_some());
        assert!(cache.texture(1).is_none(), "the least recently used went");
        assert!(cache.texture(2).is_some());
        assert_eq!(cache.stats().bytes, 200);
    }
}
