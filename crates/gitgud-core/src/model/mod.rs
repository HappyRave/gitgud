mod anim;
pub(crate) mod cache;
mod curves;
mod guids;
mod material;
mod mesh_asset;
mod meta;
mod prefab;
mod scriptable;
mod sprites;
pub(crate) mod tree;
mod yaml;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub use anim::{ClipSource, SceneClip, Track};
pub use curves::{ClipCurves, CurveKey, CurveTrack, FlipbookTrack};
pub use material::{Material, MaterialProperty, MaterialView};
pub use prefab::{MeshRef, ModelNode, ModelScene};
pub use sprites::{Sprite, SpriteChange, SpriteSheet};

use crate::cli::GitCli;
use crate::image::Version;
use crate::repo::{Error, Repo};
use crate::table::{Table, TableDiff};
use guids::Guids;
use meta::Meta;
use tree::Files;

/// The tables of a data file, compared across two versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataTables {
    /// "CSV", "JSON", "ScriptableObject · BoatDatabase"...
    pub kind: String,
    /// The earlier version had the file, so rows carry changes.
    pub compared: bool,
    pub tables: Vec<TableDiff>,
}

/// How many users of a material the "Your mesh" list offers.
pub const MAX_USERS: usize = 50;

/// The kinds of 3D asset the viewer opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        "prefab" | "unity" => ModelKind::Prefab,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Gitignored files used from disk (asset packs kept out of git): the same in every version.
    pub from_disk: usize,
    /// For a scene or prefab, the infos of the model files it draws, by path, so the
    /// viewer doesn't ask for each; a file that can't be read is left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ModelInfo>,
}

/// How many scenes or prefabs `Repo::inline_mesh` keeps: both sides of a compare.
const KEPT_INLINE_SOURCES: usize = 2;

/// A scene or prefab read for its inline meshes, with where each Mesh document is.
pub(crate) struct InlineSource {
    text: String,
    meshes: HashMap<String, Range<usize>>,
}

impl InlineSource {
    fn new(bytes: Vec<u8>) -> Self {
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
        let meshes = mesh_asset::mesh_documents(&text);
        Self { text, meshes }
    }

    /// The Mesh document with `fileID` `id`.
    fn mesh(&self, id: &str) -> Option<&str> {
        self.meshes.get(id).map(|r| &self.text[r.clone()])
    }

    /// Mesh `id` of this file (`path`) as GLB.
    fn glb(&self, path: &str, id: &str) -> Result<Vec<u8>, Error> {
        let doc = self
            .mesh(id)
            .ok_or_else(|| Error::Refused(format!("{path} has no Mesh &{id}")))?;
        mesh_asset::mesh_asset_to_glb(doc)
            .map_err(|e| Error::Read(format!("{path} › Mesh &{id}: {e}")))
    }
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

    /// The Mesh document `id` stored inside the scene or prefab `path` at `version`,
    /// as GLB.
    ///
    /// # Errors
    /// [`Error::Refused`] when the file isn't in that version or has no such Mesh;
    /// [`Error::Read`] when the mesh can't be converted.
    pub fn inline_mesh(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
        id: &str,
    ) -> Result<Vec<u8>, Error> {
        self.inline_source(git, path, version)?.glb(path, id)
    }

    /// Several Mesh documents of the scene or prefab `path` at `version` in one
    /// answer, in the order of `ids`: per mesh a status byte (1: GLB, 0: why it
    /// couldn't be read), then the `fileID` and the GLB or the reason, each as a
    /// little-endian `u32` length and its bytes.
    ///
    /// # Errors
    /// [`Error::Refused`] when the file isn't in that version.
    pub fn inline_meshes(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
        ids: &[&str],
    ) -> Result<Vec<u8>, Error> {
        fn put(out: &mut Vec<u8>, bytes: &[u8]) {
            let len = u32::try_from(bytes.len()).expect("a mesh is under 4 GiB");
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(bytes);
        }
        let source = self.inline_source(git, path, version)?;
        let mut out = Vec::new();
        for id in ids {
            let (ok, payload) = match source.glb(path, id) {
                Ok(glb) => (1u8, glb),
                Err(e) => (0u8, e.to_string().into_bytes()),
            };
            out.push(ok);
            put(&mut out, id.as_bytes());
            put(&mut out, &payload);
        }
        Ok(out)
    }

    /// The scene or prefab `path` at `version`, indexed for its meshes; kept for both
    /// sides of a compare, since a scene's meshes are requested all at once.
    fn inline_source(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
    ) -> Result<Arc<InlineSource>, Error> {
        let key = format!("{path}@{version:?}");
        // Held while loading, so the burst of requests reads the file once.
        let mut kept = self
            .inline_sources
            .lock()
            .expect("inline source lock poisoned");
        if let Some((_, s)) = kept.iter().find(|(k, _)| *k == key) {
            return Ok(s.clone());
        }
        let raw = self.file_at(path, version)?;
        let bytes =
            self.lfs_resolve_with_fetch(git, path, raw, crate::lfs::AUTO_FETCH_MAX_BYTES)?;
        let s = Arc::new(InlineSource::new(bytes));
        if kept.len() >= KEPT_INLINE_SOURCES {
            kept.remove(0);
        }
        kept.push((key, s.clone()));
        Ok(s)
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
        // Building a scene or prefab is the slow part, and a commit's never changes.
        let disk = match &origin {
            cache::Origin::Tree(tree) if kind == ModelKind::Prefab => {
                Some((crate::disk_cache::DiskCache::new(self), tree.to_string()))
            }
            _ => None,
        };
        let info = match disk.as_ref().and_then(|(d, tree)| d.model_info(tree, path)) {
            Some(hit) => hit,
            None => {
                let info = self.read_model_info(git, path, version, kind)?;
                if let Some((d, tree)) = &disk {
                    d.put_model_info(tree, path, &info);
                }
                info
            }
        };
        self.model_infos
            .lock()
            .expect("model info lock poisoned")
            .put(path, origin, Arc::new(info.clone()), generation);
        Ok(info)
    }

    /// The curves of the `.anim` at `path`, compared between `before` (when the file
    /// is there) and `after`.
    ///
    /// # Errors
    /// [`Error::Refused`] when `after` has no such clip.
    pub fn clip_curves(
        &self,
        git: Option<&GitCli>,
        path: &str,
        before: Option<&Version>,
        after: &Version,
    ) -> Result<ClipCurves, Error> {
        let missing = || Error::Refused(format!("{path} isn't in that version"));
        let after_files = Files::new(self, after)?.with_git(git);
        let after_text = after_files.read_text(path)?.ok_or_else(missing)?;
        let before_files = match before {
            Some(v) => Some(Files::new(self, v)?.with_git(git)),
            None => None,
        };
        let before_text = match &before_files {
            Some(f) => f.read_text(path)?,
            None => None,
        };
        let before_guids = before_files.as_ref().map(Guids::new);
        curves::clip_curves(
            before_text.as_deref().zip(before_guids.as_ref()),
            (&after_text, &Guids::new(&after_files)),
            path,
        )
    }

    /// Prefabs whose Animator plays the clip at `path` in `version`: controllers (and
    /// override controllers) naming its GUID, then prefabs naming theirs; prefabs
    /// first found first, at most [`MAX_USERS`].
    ///
    /// # Errors
    /// As [`Repo::material_users`].
    pub fn clip_users(
        &self,
        git: &GitCli,
        path: &str,
        version: &Version,
    ) -> Result<Vec<String>, Error> {
        let files = Files::new(self, version)?;
        let mut users = Vec::new();
        for controller in self.files_naming(
            git,
            &files,
            path,
            version,
            &["*.controller", "*.overrideController"],
        )? {
            for prefab in self.files_naming(git, &files, &controller, version, &["*.prefab"])? {
                if !users.contains(&prefab) {
                    users.push(prefab);
                }
            }
        }
        users.truncate(MAX_USERS);
        Ok(users)
    }

    /// Files matching `globs` in `version` whose text names the GUID of `asset`
    /// (read from its `.meta`), found with `git grep`.
    fn files_naming(
        &self,
        git: &GitCli,
        files: &Files<'_>,
        asset: &str,
        version: &Version,
        globs: &[&str],
    ) -> Result<Vec<String>, Error> {
        let Some(guid) = files
            .read_text(&format!("{asset}.meta"))?
            .as_deref()
            .and_then(Meta::guid_only)
        else {
            return Ok(Vec::new());
        };
        let rev = match version {
            Version::Head => Some("HEAD".to_owned()),
            Version::Commit(id) => Some(id.clone()),
            Version::Parent(id) => Some(format!("{id}^")),
            _ => None,
        };
        let mut args = vec!["grep", "-l", "-F", "-e", guid.as_str()];
        if matches!(version, Version::Index) {
            args.push("--cached");
        }
        if let Some(r) = &rev {
            args.push(r);
        }
        args.push("--");
        args.extend(globs);
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        if code > 1 {
            return Err(Error::Command {
                command: "git grep".into(),
                message: err.trim().to_owned(),
            });
        }
        let prefix = rev.map(|r| format!("{r}:")).unwrap_or_default();
        Ok(out
            .lines()
            .map(|l| l.strip_prefix(&prefix).unwrap_or(l).to_owned())
            .collect())
    }

    /// The sprites of `texture` (an image path) from its `.meta` at `after`, compared
    /// with `before` when that version has the `.meta`.
    ///
    /// # Errors
    /// [`Error::Read`] when a version can't be listed.
    pub fn sprite_sheet(
        &self,
        texture: &str,
        before: Option<&Version>,
        after: &Version,
    ) -> Result<SpriteSheet, Error> {
        let meta = format!("{texture}.meta");
        let read = |v: &Version| -> Result<Option<(String, Vec<sprites::Sprite>)>, Error> {
            Ok(Files::new(self, v)?
                .read_text(&meta)?
                .map(|t| sprites::read_sprites(&t)))
        };
        let (mode, after_sprites) = read(after)?.unwrap_or_else(|| ("none".into(), Vec::new()));
        let before_sprites = match before {
            Some(v) => read(v)?.map(|(_, s)| s),
            None => None,
        };
        Ok(SpriteSheet {
            changes: before_sprites
                .as_deref()
                .map(|b| sprites::diff_sprites(b, &after_sprites))
                .unwrap_or_default(),
            mode,
            before: before_sprites,
            after: after_sprites,
        })
    }

    /// The tables in a CSV / TSV / JSON file or a ScriptableObject `.asset`, compared
    /// between `before` (when the file is there) and `after`.
    ///
    /// # Errors
    /// [`Error::Refused`] when `after` has no such file or it can't be read as tables.
    pub fn data_tables(
        &self,
        git: Option<&GitCli>,
        path: &str,
        before: Option<&Version>,
        after: &Version,
    ) -> Result<DataTables, Error> {
        let read = |version: &Version| -> Result<(String, Vec<Table>), Error> {
            let lower = path.to_ascii_lowercase();
            if lower.ends_with(".asset") || lower.ends_with(".spriteatlas") {
                let files = Files::new(self, version)?.with_git(git);
                let text = files
                    .read_text(path)?
                    .ok_or_else(|| Error::Refused(format!("{path} isn't in that version")))?;
                return scriptable::scriptable_tables(&text, path, &Guids::new(&files));
            }
            let bytes = self.file_bytes(git, path, version, crate::lfs::AUTO_FETCH_MAX_BYTES)?;
            let tables = crate::table::read_tables(&bytes, path)
                .map_err(|e| Error::Refused(format!("{path}: {e}")))?;
            let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
            Ok((ext.to_ascii_uppercase(), tables))
        };
        let (kind, after_tables) = read(after)?;
        let before_tables = match before {
            Some(v) if Files::new(self, v).is_ok_and(|f| f.contains(path)) => Some(read(v)?.1),
            _ => None,
        };
        Ok(DataTables {
            kind,
            compared: before_tables.is_some(),
            tables: crate::table::diff_tables(before_tables.as_deref(), &after_tables),
        })
    }

    /// Every property, keyword and the render queue of the `.mat` at `path`.
    ///
    /// # Errors
    /// [`Error::Refused`] when it isn't in that version or holds no material.
    pub fn material_view(
        &self,
        git: Option<&GitCli>,
        path: &str,
        version: &Version,
    ) -> Result<MaterialView, Error> {
        let files = Files::new(self, version)?.with_git(git);
        MaterialView::load(path, &Guids::new(&files))
    }

    /// Prefabs and models that use the material at `path` in `version`, prefabs
    /// first, at most [`MAX_USERS`]: files naming its GUID, found with `git grep`.
    ///
    /// # Errors
    /// [`Error::Command`] when git grep fails; [`Error::Read`] when the version can't be read.
    pub fn material_users(
        &self,
        git: &GitCli,
        path: &str,
        version: &Version,
    ) -> Result<Vec<String>, Error> {
        let files = Files::new(self, version)?;
        let globs = [
            "*.prefab",
            "*.fbx.meta",
            "*.obj.meta",
            "*.gltf.meta",
            "*.glb.meta",
        ];
        let mut users: Vec<String> = self
            .files_naming(git, &files, path, version, &globs)?
            .into_iter()
            .map(|l| l.strip_suffix(".meta").map_or(l.clone(), str::to_owned))
            .collect();
        users.sort_by_key(|p| (!p.ends_with(".prefab"), p.clone()));
        users.truncate(MAX_USERS);
        Ok(users)
    }

    /// The infos of the model files `scene` draws or takes clips from, read on a few
    /// threads; files that can't be read are left out (the viewer asks and shows why).
    fn used_model_infos(
        &self,
        git: Option<&GitCli>,
        scene: &ModelScene,
        version: &Version,
    ) -> BTreeMap<String, ModelInfo> {
        let mut paths: Vec<&str> = scene
            .nodes
            .iter()
            .filter_map(|n| match &n.mesh {
                Some(MeshRef::Asset { path, .. }) => Some(path.as_str()),
                _ => None,
            })
            .chain(scene.clips.iter().filter_map(|c| match &c.source {
                ClipSource::Model { path, .. } => Some(path.as_str()),
                ClipSource::Curves { .. } => None,
            }))
            .filter(|p| model_kind(p).is_some_and(|k| k != ModelKind::Prefab))
            .collect();
        paths.sort_unstable();
        paths.dedup();
        let threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .min(8);
        let chunk = paths.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let handles: Vec<_> = paths
                .chunks(chunk)
                .map(|part| {
                    s.spawn(move || {
                        part.iter()
                            .filter_map(|p| {
                                Some(((*p).to_owned(), self.model_info(git, p, version).ok()?))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("model info thread panicked"))
                .collect()
        })
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
            from_disk: 0,
            models: BTreeMap::new(),
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
            info.models = self.used_model_infos(git, &scene, version);
            info.scene = Some(scene);
            info.materials = materials;
            info.from_disk = files.disk_reads();
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
        info.from_disk = files.disk_reads();
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
    fn reads_gitignored_assets_from_disk_in_every_version() {
        let (mesh, pack) = (
            "0e5e0000000000000000000000000001",
            "9ac00000000000000000000000000002",
        );
        let prefab = |nested: &str| {
            format!(
                "%YAML 1.1\n--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications: []\n  m_SourcePrefab: {{fileID: 100100000, guid: {nested}, type: 3}}\n"
            )
        };
        let crate_prefab = format!(
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Crate\n  m_IsActive: 1\n  m_Component:\n  - component: {{fileID: 4}}\n  - component: {{fileID: 33}}\n--- !u!4 &4\nTransform:\n  m_GameObject: {{fileID: 1}}\n  m_LocalRotation: {{x: 0, y: 0, z: 0, w: 1}}\n  m_LocalPosition: {{x: 0, y: 0, z: 0}}\n  m_LocalScale: {{x: 1, y: 1, z: 1}}\n  m_Father: {{fileID: 0}}\n--- !u!33 &33\nMeshFilter:\n  m_GameObject: {{fileID: 1}}\n  m_Mesh: {{fileID: 4300000, guid: {mesh}, type: 3}}\n"
        );
        let repo = TestRepo::init();
        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.1f1\n",
        )
        .write(".gitignore", "Assets/Packs\n")
        .write("Assets/Packs/Crate.prefab", &crate_prefab)
        .write("Assets/Packs/Crate.prefab.meta", &format!("guid: {pack}\n"))
        .write("Assets/Packs/crate.fbx", "fbx")
        .write("Assets/Packs/crate.fbx.meta", &format!("guid: {mesh}\n"))
        .write("Assets/Level.prefab", &prefab(pack))
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        for v in [Version::Head, Version::Commit(head), Version::Worktree] {
            let info = r.model_info(None, "Assets/Level.prefab", &v).unwrap();
            let scene = info.scene.unwrap();
            assert!(scene.missing.is_empty(), "{v:?}: {:?}", scene.missing);
            let crate_node = scene.nodes.iter().find(|n| n.name == "Crate").unwrap();
            assert!(
                matches!(&crate_node.mesh, Some(MeshRef::Asset { path, .. }) if path == "Assets/Packs/crate.fbx")
            );
            assert!(info.from_disk >= 2, "{v:?}: {}", info.from_disk);
        }
        let key = |v: Version| {
            r.model_info(None, "Assets/Packs/crate.fbx", &v)
                .unwrap()
                .key
        };
        let disk = key(Version::Worktree);
        assert!(
            disk.as_deref()
                .is_some_and(|k| k.starts_with("wt:Assets/Packs/crate.fbx:"))
        );
        assert_eq!(key(Version::Head), disk, "one load serves both sides");
        let listings = || r.listings.lock().unwrap().len();
        let before = listings();
        tree::Files::new(&r, &Version::Head).unwrap();
        assert_eq!(listings(), before, "a version's listing is built once");
        crate::unity_yaml::forget_guid_index(&r);
        assert!(
            r.listings
                .lock()
                .unwrap()
                .iter()
                .all(|(o, _)| matches!(o, cache::Origin::Tree(_))),
            "the work tree's listing goes on refresh, commits' stay"
        );
        assert_eq!(
            r.file_at("Assets/Packs/crate.fbx", &Version::Head).unwrap(),
            b"fbx"
        );
        assert_eq!(
            r.ignored.lock().unwrap().get("Assets/Packs/"),
            Some(&true),
            "the ignored folder answers for its other files"
        );
        assert!(r.file_at("Assets/nothing.fbx", &Version::Head).is_err());
        repo.write("Assets/untracked.fbx", "x");
        assert!(
            !r.ignored_on_disk("Assets/untracked.fbx"),
            "untracked isn't ignored"
        );
        assert!(r.file_at("Assets/untracked.fbx", &Version::Head).is_err());
    }

    #[test]
    fn draws_meshes_stored_inside_the_scene() {
        let go = "--- !u!1 &10\nGameObject:\n  m_Name: Wall\n  m_IsActive: 1\n  m_Component:\n  - component: {fileID: 11}\n  - component: {fileID: 12}\n--- !u!4 &11\nTransform:\n  m_GameObject: {fileID: 10}\n  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}\n  m_LocalPosition: {x: 0, y: 0, z: 0}\n  m_LocalScale: {x: 1, y: 1, z: 1}\n  m_Father: {fileID: 0}\n--- !u!33 &12\nMeshFilter:\n  m_GameObject: {fileID: 10}\n  m_Mesh: {fileID: 1234}\n";
        let scene = format!(
            "%YAML 1.1\n{go}{}--- !u!104 &2\nRenderSettings:\n  m_Fog: 0\n",
            mesh_asset::tri_mesh("1234")
        );
        let repo = TestRepo::init();
        repo.write("Assets/Room.unity", &scene).commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let info = r
            .model_info(None, "Assets/Room.unity", &Version::Head)
            .unwrap();
        let scene = info.scene.unwrap();
        assert!(scene.missing.is_empty(), "{:?}", scene.missing);
        let key = mesh_asset::mesh_key(&mesh_asset::tri_mesh("1234"));
        assert_eq!(
            scene.nodes[0].mesh,
            Some(MeshRef::Inline {
                path: "Assets/Room.unity".into(),
                id: "1234".into(),
                key: key.clone(),
            })
        );
        assert_eq!(
            mesh_asset::mesh_key(&mesh_asset::tri_mesh("5678")),
            key,
            "the same mesh under another fileID"
        );
        let glb = r
            .inline_mesh(None, "Assets/Room.unity", &Version::Head, "1234")
            .unwrap();
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(
            r.inline_mesh(None, "Assets/Room.unity", &Version::Head, "123")
                .unwrap_err()
                .to_string(),
            "Assets/Room.unity has no Mesh &123"
        );
        let bundle = r
            .inline_meshes(None, "Assets/Room.unity", &Version::Head, &["1234", "123"])
            .unwrap();
        let mut at = 0;
        let mut read = || {
            let ok = bundle[at];
            let field = |at: &mut usize| {
                let len = u32::from_le_bytes(bundle[*at..*at + 4].try_into().unwrap()) as usize;
                *at += 4 + len;
                &bundle[*at - len..*at]
            };
            at += 1;
            let id = String::from_utf8(field(&mut at).to_vec()).unwrap();
            let payload = field(&mut at).to_vec();
            (ok, id, payload)
        };
        let (ok, id, glb) = read();
        assert_eq!((ok, id.as_str(), &glb[0..4]), (1, "1234", &b"glTF"[..]));
        let (ok, id, why) = read();
        assert_eq!((ok, id.as_str()), (0, "123"));
        assert_eq!(
            String::from_utf8(why).unwrap(),
            "Assets/Room.unity has no Mesh &123"
        );
        assert_eq!(at, bundle.len());

        repo.write(
            "Assets/Room.unity",
            &format!("%YAML 1.1\n{}", mesh_asset::tri_mesh("5678")),
        );
        for _ in 0..2 {
            assert!(
                r.inline_mesh(None, "Assets/Room.unity", &Version::Head, "1234")
                    .is_ok()
            );
            assert!(
                r.inline_mesh(None, "Assets/Room.unity", &Version::Worktree, "5678")
                    .is_ok()
            );
        }
        assert_eq!(
            r.inline_sources.lock().unwrap().len(),
            2,
            "both sides of the compare are kept"
        );
    }

    #[test]
    fn a_prefab_carries_the_infos_of_the_models_it_draws() {
        let repo = TestRepo::init();
        repo.write("Assets/boat.fbx", "fbx")
            .write(
                "Assets/boat.fbx.meta",
                "guid: 5555eeee000000000000000000000000\nModelImporter:\n  meshes:\n    globalScale: 2\n",
            )
            .write(
                "Assets/Boat.prefab",
                "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Boat\n  m_Component:\n  - component: {fileID: 2}\n  - component: {fileID: 3}\n--- !u!4 &2\nTransform:\n  m_GameObject: {fileID: 1}\n  m_Father: {fileID: 0}\n--- !u!33 &3\nMeshFilter:\n  m_GameObject: {fileID: 1}\n  m_Mesh: {fileID: 4300000, guid: 5555eeee000000000000000000000000, type: 3}\n",
            )
            .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let info = r
            .model_info(None, "Assets/Boat.prefab", &Version::Head)
            .unwrap();
        assert_eq!(info.models.keys().collect::<Vec<_>>(), ["Assets/boat.fbx"]);
        assert_eq!(info.models["Assets/boat.fbx"].scale, 2.0);
        assert!(
            r.model_info(None, "Assets/boat.fbx", &Version::Head)
                .unwrap()
                .models
                .is_empty()
        );
    }

    #[test]
    fn indexes_the_mesh_documents_of_a_scene() {
        let text = format!(
            "{}--- !u!43 &99 stripped\nMesh:\n--- !u!1 &7\nGameObject:\n  m_Name: A\n{}",
            mesh_asset::tri_mesh("7"),
            mesh_asset::tri_mesh("70")
        );
        let source = InlineSource::new(text.clone().into_bytes());
        assert_eq!(source.meshes.len(), 3, "the GameObject &7 isn't a mesh");
        assert_eq!(source.mesh("7").unwrap(), mesh_asset::tri_mesh("7"));
        assert_eq!(
            source.mesh("99").unwrap(),
            "--- !u!43 &99 stripped\nMesh:\n"
        );
        assert_eq!(source.mesh("70").unwrap(), mesh_asset::tri_mesh("70"));
        assert!(text.ends_with(source.mesh("70").unwrap()));
        assert!(source.mesh("1").is_none());
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
    fn compares_scriptable_object_lists_and_csv_files() {
        let script = "5555eeee000000000000000000000000";
        let prefab = "6666ffff000000000000000000000000";
        let asset = |speed: &str, extra: &str| {
            format!(
                "%YAML 1.1\n--- !u!114 &11400000\nMonoBehaviour:\n  m_ObjectHideFlags: 0\n  m_Script: {{fileID: 11500000, guid: {script}, type: 3}}\n  m_Name: Boats\n  version: 2\n  boats:\n  - id: skiff\n    speed: {speed}\n    prefab: {{fileID: 100100000, guid: {prefab}, type: 3}}\n    upgrades:\n    - cost: 150\n{extra}"
            )
        };
        let repo = TestRepo::init();
        repo.write("Assets/BoatDatabase.cs", "class BoatDatabase {}")
            .write("Assets/BoatDatabase.cs.meta", &format!("guid: {script}\n"))
            .write("Assets/Skiff.prefab", "")
            .write("Assets/Skiff.prefab.meta", &format!("guid: {prefab}\n"))
            .write("Assets/Boats.asset", &asset("12", ""))
            .write("Data/stats.csv", "id,speed\nskiff,12\n")
            .commit_all("one");
        repo.write(
            "Assets/Boats.asset",
            &asset(
                "13.5",
                "  - id: tug\n    speed: 7\n    prefab: {fileID: 0}\n",
            ),
        )
        .write("Data/stats.csv", "id,speed\nskiff,14\n")
        .write("Data/new.csv", "a\n1\n");
        let r = crate::repo::open(repo.path()).unwrap();
        let d = r
            .data_tables(
                None,
                "Assets/Boats.asset",
                Some(&Version::Head),
                &Version::Worktree,
            )
            .unwrap();
        assert_eq!(d.kind, "ScriptableObject · BoatDatabase");
        assert_eq!(d.tables[0].name, "fields");
        assert_eq!(d.tables[0].rows.len(), 2, "m_Name and version");
        let boats = &d.tables[1];
        assert_eq!(boats.columns, ["id", "speed", "prefab", "upgrades[0].cost"]);
        assert_eq!(boats.key.as_deref(), Some("id"));
        assert_eq!(
            boats.rows[0].cells[1],
            [Some("12".into()), Some("13.5".into())]
        );
        assert_eq!(boats.rows[0].cells[2][1].as_deref(), Some("Skiff.prefab"));
        assert_eq!(boats.rows[1].cells[2][1].as_deref(), Some("none"));
        assert_eq!((boats.changed, boats.added), (1, 1));

        let csv = r
            .data_tables(
                None,
                "Data/stats.csv",
                Some(&Version::Head),
                &Version::Worktree,
            )
            .unwrap();
        assert_eq!(csv.kind, "CSV");
        assert_eq!(csv.tables[0].changed_cells, 1);
        let added = r
            .data_tables(
                None,
                "Data/new.csv",
                Some(&Version::Head),
                &Version::Worktree,
            )
            .unwrap();
        assert_eq!(added.tables[0].added, 0, "a new file compares as itself");
        repo.write(
            "Assets/HUD.spriteatlas",
            &format!(
                "--- !u!687078895 &4343727234628468602\nSpriteAtlas:\n  m_Name: HUD\n  m_EditorData:\n    packingSettings:\n      padding: 4\n    packables:\n    - {{fileID: 100100000, guid: {prefab}, type: 3}}\n  m_PackedSprites:\n  - {{fileID: 21300000, guid: {prefab}, type: 3}}\n"
            ),
        );
        let a = r
            .data_tables(None, "Assets/HUD.spriteatlas", None, &Version::Worktree)
            .unwrap();
        assert_eq!(a.kind, "Sprite Atlas");
        let names: Vec<_> = a.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["fields", "packables"]);
        assert_eq!(a.tables[1].columns, ["asset"]);
        assert_eq!(
            a.tables[1].rows[0].cells[0][1].as_deref(),
            Some("Skiff.prefab")
        );
        assert!(
            a.tables[0]
                .rows
                .iter()
                .any(|r| r.cells[0][1].as_deref() == Some("packingSettings.padding"))
        );
        repo.write(
            "Assets/Hull.asset",
            "--- !u!43 &4300000\nMesh:\n  m_Name: Hull\n",
        );
        let err = r
            .data_tables(None, "Assets/Hull.asset", None, &Version::Worktree)
            .unwrap_err();
        assert!(err.to_string().contains("isn't a ScriptableObject"));
    }

    #[test]
    fn finds_prefabs_that_play_a_clip_through_their_controller() {
        let (clip, ctrl) = (
            "a0000000000000000000000000000001",
            "c0000000000000000000000000000002",
        );
        let repo = TestRepo::init();
        repo.write(
            "Assets/Bob.anim",
            "--- !u!74 &7400000\nAnimationClip:\n  m_Name: Bob\n",
        )
        .write("Assets/Bob.anim.meta", &format!("guid: {clip}\n"))
        .write(
            "Assets/Boat.controller",
            &format!("m_Motion: {{fileID: 7400000, guid: {clip}, type: 2}}\n"),
        )
        .write("Assets/Boat.controller.meta", &format!("guid: {ctrl}\n"))
        .write(
            "Assets/Skiff.prefab",
            &format!("m_Controller: {{fileID: 9100000, guid: {ctrl}, type: 2}}\n"),
        )
        .write("Assets/Other.prefab", "m_Controller: {fileID: 0}\n")
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let git = GitCli::new(&crate::git::locate().unwrap());
        assert_eq!(
            r.clip_users(&git, "Assets/Bob.anim", &Version::Head)
                .unwrap(),
            ["Assets/Skiff.prefab"]
        );
        let c = r
            .clip_curves(
                None,
                "Assets/Bob.anim",
                Some(&Version::Head),
                &Version::Worktree,
            )
            .unwrap();
        assert_eq!((c.name.as_str(), c.keys_before), ("Bob", Some(0)));
    }

    #[test]
    fn finds_the_prefabs_and_models_that_use_a_material() {
        let guid = "3333cccc000000000000000000000000";
        let uses = format!("m_Materials:\n  - {{fileID: 2100000, guid: {guid}, type: 2}}\n");
        let repo = TestRepo::init();
        repo.write(
            "Assets/hull.mat",
            "--- !u!21 &2100000\nMaterial:\n  m_Name: Hull\n",
        )
        .write("Assets/hull.mat.meta", &format!("guid: {guid}\n"))
        .write("Assets/Boats/Tug.prefab", &uses)
        .write("Assets/Other.prefab", "m_Materials: []\n")
        .write("Assets/Art/boat.fbx", "fbx")
        .write(
            "Assets/Art/boat.fbx.meta",
            &format!("externalObjects:\n  second: {{fileID: 2100000, guid: {guid}, type: 2}}\n"),
        )
        .commit_all("one");
        repo.write("Assets/Barge.prefab", &uses);
        repo.git(&["add", "Assets/Barge.prefab"]);
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        let r = crate::repo::open(repo.path()).unwrap();
        let git = GitCli::new(&crate::git::locate().unwrap());
        let users = |v: Version| r.material_users(&git, "Assets/hull.mat", &v).unwrap();
        assert_eq!(
            users(Version::Head),
            ["Assets/Boats/Tug.prefab", "Assets/Art/boat.fbx"]
        );
        assert_eq!(users(Version::Commit(head.clone())), users(Version::Head));
        assert_eq!(
            users(Version::Worktree),
            [
                "Assets/Barge.prefab",
                "Assets/Boats/Tug.prefab",
                "Assets/Art/boat.fbx"
            ]
        );
        assert_eq!(users(Version::Index), users(Version::Worktree));
        assert!(
            r.material_users(&git, "Assets/none.mat", &Version::Head)
                .unwrap()
                .is_empty()
        );
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
