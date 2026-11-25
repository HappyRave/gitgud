use std::path::Path;

use serde::Serialize;

use crate::disk_cache::DiskCache;
use crate::repo::{Error, Repo};
use crate::unity_yaml::{clear_guid_index, guid_index_file};

/// What gitgud keeps for a repo to go faster, for Settings › Caches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    /// Where the GUID index is saved, and its size (0 until it is written).
    pub guid_index_file: String,
    pub guid_index_bytes: u64,
    /// `.meta` files the GUID index in memory knows.
    pub meta_files: usize,
    /// Commits whose `.meta` files were scanned, and `.meta` blobs read.
    pub commit_scans: usize,
    pub meta_blobs: usize,
    /// 3D model infos kept in memory.
    pub model_infos: usize,
    /// The commit-graph file (or chain folder) and its size; `None` when there is none.
    pub commit_graph_path: String,
    pub commit_graph_bytes: Option<u64>,
    /// Where Git LFS keeps downloaded objects (sizes come from `lfs_cache_stats`).
    pub lfs_objects_path: String,
    /// The 3D preview cache on disk: scene and prefab infos at commits, shrunk textures.
    pub preview_cache_path: String,
    pub preview_models: usize,
    pub preview_textures: usize,
    pub preview_bytes: u64,
}

fn file_bytes(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

impl Repo {
    /// Sizes of the caches kept for this repo; builds nothing.
    #[must_use]
    pub fn cache_stats(&self) -> CacheStats {
        let index_file = guid_index_file(self);
        let common = self.gix().common_dir().to_owned();
        let info = common.join("objects").join("info");
        let single = info.join("commit-graph");
        let chain = info.join("commit-graphs");
        let (graph_path, graph_bytes) = if single.is_file() {
            let bytes = file_bytes(&single);
            (single, bytes)
        } else if chain.is_dir() {
            let bytes = std::fs::read_dir(&chain).ok().map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum()
            });
            (chain, bytes)
        } else {
            (single, None)
        };
        let preview = DiskCache::new(self);
        let preview_stats = preview.stats();
        CacheStats {
            preview_cache_path: preview.root().to_string_lossy().into_owned(),
            preview_models: preview_stats.models,
            preview_textures: preview_stats.textures,
            preview_bytes: preview_stats.bytes,
            guid_index_bytes: file_bytes(&index_file).unwrap_or(0),
            guid_index_file: index_file.to_string_lossy().into_owned(),
            meta_files: self
                .guid_index
                .lock()
                .expect("guid index lock poisoned")
                .as_ref()
                .map_or(0, |i| i.meta_count()),
            commit_scans: self
                .tree_guids
                .lock()
                .expect("tree guids lock poisoned")
                .len(),
            meta_blobs: self
                .meta_blob_guids
                .lock()
                .expect("meta blob guids lock poisoned")
                .len(),
            model_infos: self
                .model_infos
                .lock()
                .expect("model info lock poisoned")
                .len(),
            commit_graph_path: graph_path.to_string_lossy().into_owned(),
            commit_graph_bytes: graph_bytes,
            lfs_objects_path: common
                .join("lfs")
                .join("objects")
                .to_string_lossy()
                .into_owned(),
        }
    }

    /// Drops the Unity reference caches: the GUID index (in memory and on disk),
    /// the commit `.meta` scans and the 3D model infos. They rebuild on next use.
    ///
    /// # Errors
    /// [`Error::Read`] when the saved GUID index can't be deleted.
    pub fn clear_unity_caches(&self) -> Result<(), Error> {
        self.tree_guids
            .lock()
            .expect("tree guids lock poisoned")
            .clear();
        self.meta_blob_guids
            .lock()
            .expect("meta blob guids lock poisoned")
            .clear();
        self.model_infos
            .lock()
            .expect("model info lock poisoned")
            .clear();
        clear_guid_index(self)
    }

    /// Drops the 3D preview cache on disk, and what memory holds of it.
    ///
    /// # Errors
    /// [`Error::Read`] when the cache folder can't be deleted.
    pub fn clear_preview_cache(&self) -> Result<(), Error> {
        self.model_infos
            .lock()
            .expect("model info lock poisoned")
            .clear();
        crate::image::forget_shrunk();
        DiskCache::new(self)
            .clear()
            .map_err(|e| Error::Read(format!("cannot clear the preview cache: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use crate::image::Version;
    use crate::testing::TestRepo;

    #[test]
    fn reports_and_clears_the_unity_caches() {
        let repo = TestRepo::init();
        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.1f1\n",
        )
        .write("Assets/boat.fbx", "fbx")
        .write(
            "Assets/boat.fbx.meta",
            "guid: 4444dddd000000000000000000000000\n",
        )
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let empty = r.cache_stats();
        assert_eq!(
            (empty.meta_files, empty.model_infos, empty.guid_index_bytes),
            (0, 0, 0)
        );
        assert!(empty.guid_index_file.ends_with("guid-index"));
        assert_eq!(empty.commit_graph_bytes, None);

        r.model_info(None, "Assets/boat.fbx", &Version::Head)
            .unwrap();
        crate::unity_yaml::repo_guid_index(&r);
        repo.git(&["commit-graph", "write", "--reachable"]);
        let used = r.cache_stats();
        assert_eq!(used.meta_files, 1);
        assert_eq!(used.model_infos, 1);
        assert!(used.guid_index_bytes > 0);
        assert!(used.commit_graph_bytes.is_some_and(|b| b > 0));

        r.clear_unity_caches().unwrap();
        let cleared = r.cache_stats();
        assert_eq!(
            (
                cleared.meta_files,
                cleared.model_infos,
                cleared.commit_scans,
                cleared.meta_blobs,
                cleared.guid_index_bytes
            ),
            (0, 0, 0, 0, 0)
        );
    }

    #[test]
    fn keeps_scene_infos_at_commits_across_restarts_until_cleared() {
        let repo = TestRepo::init();
        repo.write(
            "Assets/Room.unity",
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Wall\n  m_Component:\n  - component: {fileID: 2}\n--- !u!4 &2\nTransform:\n  m_GameObject: {fileID: 1}\n  m_Father: {fileID: 0}\n",
        )
        .commit_all("one");
        let first = crate::repo::open(repo.path()).unwrap();
        let built = first
            .model_info(None, "Assets/Room.unity", &Version::Head)
            .unwrap();
        first
            .model_info(None, "Assets/Room.unity", &Version::Worktree)
            .unwrap();
        assert_eq!(first.cache_stats().preview_models, 1, "only the commit's");

        let restarted = crate::repo::open(repo.path()).unwrap();
        let stats = restarted.cache_stats();
        assert_eq!(stats.preview_models, 1);
        assert!(stats.preview_bytes > 0);
        assert!(stats.preview_cache_path.ends_with("cache"));
        assert_eq!(
            restarted
                .model_info(None, "Assets/Room.unity", &Version::Head)
                .unwrap(),
            built
        );

        restarted.clear_preview_cache().unwrap();
        let cleared = restarted.cache_stats();
        assert_eq!(
            (
                cleared.preview_models,
                cleared.preview_bytes,
                cleared.model_infos
            ),
            (0, 0, 0)
        );
    }
}
