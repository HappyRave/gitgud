mod anim;
pub(crate) mod cache;
mod guids;
mod material;
mod mesh_asset;
mod meta;
mod prefab;
mod tree;
mod yaml;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;

pub use anim::{ClipSource, SceneClip, Track};
pub use material::Material;
pub use prefab::{MeshRef, ModelNode, ModelScene};

use crate::cli::GitCli;
use crate::image::Version;
use crate::repo::{Error, Repo};
use guids::Guids;
use meta::Meta;
use tree::Files;

/// The kinds of 3D asset the viewer opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelKind {
    Fbx,
    Obj,
    Gltf,
    Glb,
    Prefab,
    /// A Unity `Mesh` saved as `.asset`; served converted to GLB.
    #[serde(rename = "asset")]
    UnityMesh,
}

/// The kind of 3D asset `path` is, by extension.
#[must_use]
pub fn model_kind(path: &str) -> Option<ModelKind> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "fbx" => ModelKind::Fbx,
        "obj" => ModelKind::Obj,
        "gltf" => ModelKind::Gltf,
        "glb" => ModelKind::Glb,
        "prefab" => ModelKind::Prefab,
        "asset" => ModelKind::UnityMesh,
        _ => return None,
    })
}

/// The MIME type a model file is served with.
#[must_use]
pub fn model_mime(kind: ModelKind) -> &'static str {
    match kind {
        ModelKind::Gltf => "model/gltf+json",
        ModelKind::Glb | ModelKind::UnityMesh => "model/gltf-binary",
        ModelKind::Obj | ModelKind::Prefab => "text/plain; charset=utf-8",
        ModelKind::Fbx => "application/octet-stream",
    }
}

/// What the viewer needs besides the model file itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub kind: ModelKind,
    /// A name for the model file's content at this version, for reusing what the
    /// viewer loaded: a blob id, or the size and time of a work-tree file.
    pub key: Option<String>,
    pub materials: Vec<Material>,
    /// Material name inside the model → index in `materials`, from the
    /// importer's material remapping.
    pub remaps: BTreeMap<String, usize>,
    /// The importer's scale on top of the file's own units.
    pub scale: f32,
    /// A prefab's GameObjects, nested prefabs and variants applied.
    pub scene: Option<ModelScene>,
}

impl Repo {
    /// The bytes of model `path` at `version`, LFS pointers swapped for their
    /// objects (fetched up to `auto_fetch_max_bytes`).
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't a model or isn't in that version, or
    /// its LFS object isn't downloaded; [`Error::Read`] otherwise.
    pub fn model_bytes(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
        auto_fetch_max_bytes: u64,
    ) -> Result<Vec<u8>, Error> {
        let Some(kind) = model_kind(path) else {
            return Err(Error::Refused(format!("{path} is not a 3D model")));
        };
        let bytes = self.file_at(path, version)?;
        let bytes = self.lfs_resolve_with_fetch(git, path, bytes, auto_fetch_max_bytes)?;
        if kind == ModelKind::UnityMesh {
            return mesh_asset::mesh_asset_to_glb(&String::from_utf8_lossy(&bytes))
                .map_err(|e| Error::Read(format!("{path}: {e}")));
        }
        Ok(bytes)
    }

    /// The repo path of the file model `near` refers to as `reference` at `version`.
    ///
    /// # Errors
    /// [`Error::Read`] when the version can't be listed.
    pub fn model_reference(
        &self,
        near: &str,
        reference: &str,
        version: &Version,
    ) -> Result<Option<String>, Error> {
        Files::new(self, version).map(|f| f.find_near(reference, near))
    }

    /// Materials, remaps and scale for model `path` at `version`. Files stored
    /// in LFS are read from their objects; with `git`, objects up to the
    /// auto-fetch limit are downloaded first. Results are kept: a commit's for
    /// good, the index's and work tree's until the next refresh.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't a model or an LFS object it needs
    /// isn't downloaded; [`Error::Read`] when the version can't be read.
    pub fn model_info(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
    ) -> Result<ModelInfo, Error> {
        let kind =
            model_kind(path).ok_or_else(|| Error::Refused(format!("{path} is not a 3D model")))?;
        let origin = tree::origin(self, version)?;
        let generation = {
            let mut kept = self.model_infos.lock().expect("model info lock poisoned");
            if let Some(hit) = kept.get(path, &origin) {
                return Ok((*hit).clone());
            }
            kept.generation()
        };
        let info = self.read_model_info(git, path, version, kind)?;
        self.model_infos
            .lock()
            .expect("model info lock poisoned")
            .put(path, origin, Arc::new(info.clone()), generation);
        Ok(info)
    }

    fn read_model_info(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
        kind: ModelKind,
    ) -> Result<ModelInfo, Error> {
        let files = Files::new(self, version)?.with_git(git);
        let guids = Guids::new(&files);
        let mut info = ModelInfo {
            kind,
            key: files.key(path),
            materials: Vec::new(),
            remaps: BTreeMap::new(),
            scale: 1.0,
            scene: None,
        };
        if kind == ModelKind::Prefab {
            let (mut scene, materials) = prefab::PrefabBuilder::new(&guids).build(path)?;
            for (node, controller) in scene
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| Some((i, n.controller.clone()?)))
                .collect::<Vec<_>>()
            {
                for (name, source) in anim::controller_clips(&controller, &guids)? {
                    scene.clips.push(SceneClip { node, name, source });
                }
            }
            info.scene = Some(scene);
            info.materials = materials;
            return Ok(info);
        }
        if let Some(text) = files.read_text(&format!("{path}.meta"))? {
            let meta = Meta::parse(&text);
            info.scale = meta.global_scale;
            let mut remaps: Vec<_> = meta.material_remaps.into_iter().collect();
            remaps.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, r) in remaps {
                let m = match r
                    .guid
                    .as_deref()
                    .map(|g| guids.path(g))
                    .transpose()?
                    .flatten()
                {
                    Some(p) => Material::load(&p, &guids)?,
                    None => Material::missing(&name),
                };
                info.remaps.insert(name, info.materials.len());
                info.materials.push(m);
            }
        }
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn recognises_model_files() {
        assert_eq!(model_kind("a/Boat.FBX"), Some(ModelKind::Fbx));
        assert_eq!(model_kind("x.glb"), Some(ModelKind::Glb));
        assert_eq!(model_kind("x.prefab"), Some(ModelKind::Prefab));
        assert_eq!(model_kind("x.png"), None);
        assert_eq!(model_mime(ModelKind::Gltf), "model/gltf+json");
    }

    #[test]
    fn keeps_model_infos_until_the_work_tree_changes() {
        let repo = TestRepo::init();
        let meta = |scale: u32| {
            format!(
                "guid: 4444dddd000000000000000000000000\nModelImporter:\n  meshes:\n    globalScale: {scale}\n"
            )
        };
        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.1f1\n",
        )
        .write("Assets/boat.fbx", "one")
        .write("Assets/boat.fbx.meta", &meta(2))
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let info = |v: &Version| r.model_info(None, "Assets/boat.fbx", v).unwrap();

        let head = info(&Version::Head);
        assert_eq!(head.scale, 2.0);
        let blob = repo.git(&["rev-parse", "HEAD:Assets/boat.fbx"]);
        assert_eq!(head.key.as_deref(), Some(blob.trim()));
        assert_eq!(info(&Version::Worktree).scale, 2.0);
        assert!(
            info(&Version::Worktree)
                .key
                .unwrap()
                .starts_with("wt:Assets/boat.fbx:")
        );

        repo.write("Assets/boat.fbx.meta", &meta(3));
        assert_eq!(info(&Version::Worktree).scale, 2.0, "kept until a refresh");
        crate::unity_yaml::forget_guid_index(&r);
        assert_eq!(info(&Version::Worktree).scale, 3.0);
        assert_eq!(info(&Version::Head).scale, 2.0);
        assert_eq!(r.model_infos.lock().unwrap().len(), 2);
    }

    #[test]
    fn serves_versions_and_reads_remaps() {
        let mat_guid = "3333cccc000000000000000000000000";
        let repo = TestRepo::init();
        repo.write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.1f1\n")
            .write("Assets/boat.fbx", "one")
            .write(
                "Assets/boat.fbx.meta",
                &format!("guid: 4444dddd000000000000000000000000\nModelImporter:\n  externalObjects:\n  - first:\n      type: UnityEngine:Material\n      assembly: UnityEngine.CoreModule\n      name: HullMat\n    second: {{fileID: 2100000, guid: {mat_guid}, type: 2}}\n  - first:\n      type: UnityEngine:Material\n      assembly: UnityEngine.CoreModule\n      name: Lost\n    second: {{fileID: 2100000, guid: 5555eeee000000000000000000000000, type: 2}}\n  meshes:\n    globalScale: 2\n"),
            )
            .write("Assets/hull.mat", "--- !u!21 &2100000\nMaterial:\n  m_Name: Hull\n  m_Shader: {fileID: 46, guid: 0000000000000000f000000000000000, type: 0}\n")
            .write("Assets/hull.mat.meta", &format!("guid: {mat_guid}\n"))
            .commit_all("one");
        repo.write("Assets/boat.fbx", "two");
        let r = crate::repo::open(repo.path()).unwrap();

        assert_eq!(
            r.model_bytes(None, "Assets/boat.fbx", &Version::Head, 0)
                .unwrap(),
            b"one"
        );
        assert_eq!(
            r.model_bytes(None, "Assets/boat.fbx", &Version::Worktree, 0)
                .unwrap(),
            b"two"
        );
        assert!(matches!(
            r.model_bytes(None, "Assets/hull.mat", &Version::Head, 0),
            Err(Error::Refused(_))
        ));

        let info = r
            .model_info(None, "Assets/boat.fbx", &Version::Head)
            .unwrap();
        assert_eq!(info.kind, ModelKind::Fbx);
        assert_eq!(info.scale, 2.0);
        assert_eq!(info.materials[info.remaps["HullMat"]].name, "Hull");
        assert_eq!(info.materials[info.remaps["HullMat"]].shader, "Standard");
        assert!(info.materials[info.remaps["Lost"]].missing);
        assert_eq!(
            r.model_reference("Assets/boat.fbx", "D:/art/hull.mat", &Version::Head)
                .unwrap()
                .as_deref(),
            Some("Assets/hull.mat")
        );
    }
}
