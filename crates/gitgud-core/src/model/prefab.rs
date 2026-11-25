use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

use super::guids::Guids;
use super::material::Material;
use super::meta::Meta;
use super::tree::base_name;
use super::yaml::{Doc, ObjRef, docs_reading, num, obj_ref, quat, scalar, vec3};
use super::{ModelKind, model_kind};
use crate::repo::Error;

const BUILTIN_RESOURCES: &str = "0000000000000000e000000000000000";
const BUILTIN_EXTRA: &str = "0000000000000000f000000000000000";
const DEFAULT_MATERIAL: &str = "10303";
const MAX_DEPTH: usize = 12;

const GAME_OBJECT: u32 = 1;
const TRANSFORM: u32 = 4;
const RECT_TRANSFORM: u32 = 224;
const MESH_RENDERER: u32 = 23;
const MESH_FILTER: u32 = 33;
const ANIMATOR: u32 = 95;
const SKINNED_MESH_RENDERER: u32 = 137;
const PREFAB_INSTANCE: u32 = 1001;
const MESH: u32 = 43;
const LOD_GROUP: u32 = 205;
/// The classes whose contents the builder reads; the rest only need their header.
const READ: [u32; 9] = [
    GAME_OBJECT,
    TRANSFORM,
    RECT_TRANSFORM,
    MESH_RENDERER,
    MESH_FILTER,
    ANIMATOR,
    SKINNED_MESH_RENDERER,
    PREFAB_INSTANCE,
    LOD_GROUP,
];

/// The file being read and its documents, for meshes stored inside it.
struct Local<'d> {
    path: &'d str,
    docs: &'d HashMap<i64, &'d Doc>,
}

/// The mesh a node draws.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MeshRef {
    /// A mesh inside a model file, by `name` when the importer's table has it,
    /// else by Unity's hashed `id`; the whole model when both are `None`.
    #[serde(rename_all = "camelCase")]
    Asset {
        path: String,
        name: Option<String>,
        id: Option<String>,
        /// The model importer's scale on top of the file's own units.
        scale: f32,
    },
    /// One of Unity's built-in primitives: Cube, Sphere, Capsule, Cylinder, Plane, Quad.
    Builtin { name: String },
    /// A Mesh stored inside the scene or prefab file itself (ProBuilder, combined
    /// meshes), served as GLB by its `fileID`.
    Inline { path: String, id: String },
    /// The referenced mesh isn't in this version.
    Missing,
}

/// One GameObject of an assembled prefab, in three.js's right-handed space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelNode {
    pub name: String,
    pub parent: Option<usize>,
    pub position: [f32; 3],
    /// Quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub active: bool,
    pub mesh: Option<MeshRef>,
    /// Indices into [`super::ModelInfo::materials`], one per sub-mesh.
    pub materials: Vec<usize>,
    pub skinned: bool,
    /// Repo path of the Animator Controller on this object.
    pub controller: Option<String>,
}

/// A prefab flattened into nodes, nested prefabs and variants applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelScene {
    pub nodes: Vec<ModelNode>,
    /// References that couldn't be found, as "Cargo › MeshFilter.m_Mesh → guid 3f9a…".
    pub missing: Vec<String>,
    /// Clips the prefab's Animators can play.
    pub clips: Vec<super::anim::SceneClip>,
}

#[derive(Debug, Clone)]
struct Node {
    out: ModelNode,
    transform: i64,
    game_object: i64,
    filter: Option<i64>,
    renderer: Option<i64>,
    removed: bool,
}

#[derive(Clone)]
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn find(&self, pred: impl Fn(&Node) -> bool) -> Option<usize> {
        self.nodes.iter().position(pred)
    }

    fn by_transform(&self, id: i64) -> Option<usize> {
        self.find(|n| n.transform == id)
    }

    fn by_game_object(&self, id: i64) -> Option<usize> {
        self.find(|n| n.game_object == id)
    }
}

fn id(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

/// Unity's fileID for an object of a nested prefab, as seen from the outer prefab.
fn nested_id(instance: i64, inner: i64) -> i64 {
    (instance ^ inner) & i64::MAX
}

pub(crate) struct PrefabBuilder<'a, 'g> {
    guids: &'g Guids<'a>,
    pub materials: Vec<Material>,
    material_ix: HashMap<String, usize>,
    pub missing: Vec<String>,
    /// Nested prefabs already assembled, by path and depth: a prefab is often placed many times.
    nested: HashMap<(String, usize), Tree>,
}

impl<'a, 'g> PrefabBuilder<'a, 'g> {
    pub fn new(guids: &'g Guids<'a>) -> Self {
        Self {
            guids,
            materials: Vec::new(),
            material_ix: HashMap::new(),
            missing: Vec::new(),
            nested: HashMap::new(),
        }
    }

    /// Assembles the prefab at `path`.
    pub fn build(mut self, path: &str) -> Result<(ModelScene, Vec<Material>), Error> {
        let text = self
            .guids
            .files
            .read_text(path)?
            .ok_or_else(|| Error::Refused(format!("{path} is not in that version")))?;
        let tree = self.tree(path, &text, 0)?;
        let keep: Vec<usize> = (0..tree.nodes.len())
            .filter(|&i| !tree.nodes[i].removed)
            .collect();
        let new_ix: HashMap<usize, usize> = keep.iter().enumerate().map(|(n, &o)| (o, n)).collect();
        let nodes = keep
            .iter()
            .map(|&i| {
                let mut out = tree.nodes[i].out.clone();
                out.parent = out.parent.and_then(|p| new_ix.get(&p).copied());
                out.position[0] = -out.position[0];
                out.rotation[1] = -out.rotation[1];
                out.rotation[2] = -out.rotation[2];
                out
            })
            .collect();
        let scene = ModelScene {
            nodes,
            missing: std::mem::take(&mut self.missing),
            clips: Vec::new(),
        };
        Ok((scene, self.materials))
    }

    fn tree(&mut self, path: &str, text: &str, depth: usize) -> Result<Tree, Error> {
        let docs = docs_reading(text, |class, stripped| stripped || READ.contains(&class));
        let by_id: HashMap<i64, &Doc> = docs.iter().map(|d| (id(&d.file_id), d)).collect();
        let local = Local { path, docs: &by_id };
        let mut tree = Tree { nodes: Vec::new() };

        for d in docs
            .iter()
            .filter(|d| d.class_id == GAME_OBJECT && !d.stripped)
        {
            let transform = d.body["m_Component"]
                .as_vec()
                .into_iter()
                .flatten()
                .filter_map(|c| obj_ref(&c["component"]))
                .map(|r| id(&r.file_id))
                .find(|t| {
                    by_id
                        .get(t)
                        .is_some_and(|t| matches!(t.class_id, TRANSFORM | RECT_TRANSFORM))
                });
            let Some(transform) = transform else {
                continue;
            };
            let t = &by_id[&transform].body;
            tree.nodes.push(Node {
                out: ModelNode {
                    name: scalar(&d.body["m_Name"]).unwrap_or_default(),
                    parent: None,
                    position: vec3(&t["m_LocalPosition"], [0.0; 3]),
                    rotation: quat(&t["m_LocalRotation"]),
                    scale: vec3(&t["m_LocalScale"], [1.0; 3]),
                    active: num(&d.body["m_IsActive"]).is_none_or(|a| a != 0.0),
                    mesh: None,
                    materials: Vec::new(),
                    skinned: false,
                    controller: None,
                },
                transform,
                game_object: id(&d.file_id),
                filter: None,
                renderer: None,
                removed: false,
            });
        }

        let mut instance_roots: Vec<(usize, i64)> = Vec::new();
        for d in docs
            .iter()
            .filter(|d| d.class_id == PREFAB_INSTANCE && !d.stripped)
        {
            let instance = id(&d.file_id);
            let m = &d.body["m_Modification"];
            let parent = obj_ref(&m["m_TransformParent"]).map(|r| id(&r.file_id));
            let first = tree.nodes.len();
            match self.instance(d, depth)? {
                Some(sub) => {
                    for mut n in sub.nodes {
                        n.out.parent = n.out.parent.map(|p| p + first);
                        n.transform = nested_id(instance, n.transform);
                        n.game_object = nested_id(instance, n.game_object);
                        n.filter = n.filter.map(|f| nested_id(instance, f));
                        n.renderer = n.renderer.map(|r| nested_id(instance, r));
                        tree.nodes.push(n);
                    }
                }
                None => continue,
            }
            for i in first..tree.nodes.len() {
                if tree.nodes[i].out.parent.is_none()
                    && let Some(p) = parent
                {
                    instance_roots.push((i, p));
                }
            }
        }

        // Stripped objects stand for an object of a nested instance.
        let mut alias: HashMap<i64, i64> = HashMap::new();
        for d in docs.iter().filter(|d| d.stripped) {
            if let (Some(src), Some(inst)) = (
                obj_ref(&d.body["m_CorrespondingSourceObject"]),
                obj_ref(&d.body["m_PrefabInstance"]),
            ) {
                alias.insert(
                    id(&d.file_id),
                    nested_id(id(&inst.file_id), id(&src.file_id)),
                );
            }
        }
        let resolve = |x: i64| alias.get(&x).copied().unwrap_or(x);

        for (i, p) in instance_roots {
            tree.nodes[i].out.parent = tree.by_transform(resolve(p));
        }
        for d in docs
            .iter()
            .filter(|d| matches!(d.class_id, TRANSFORM | RECT_TRANSFORM) && !d.stripped)
        {
            let Some(child) = tree.by_transform(id(&d.file_id)) else {
                continue;
            };
            if let Some(f) = obj_ref(&d.body["m_Father"]) {
                tree.nodes[child].out.parent = tree.by_transform(resolve(id(&f.file_id)));
            }
        }

        for d in docs.iter().filter(|d| !d.stripped) {
            if !matches!(
                d.class_id,
                MESH_FILTER | MESH_RENDERER | SKINNED_MESH_RENDERER | ANIMATOR
            ) {
                continue;
            }
            let Some(go) = obj_ref(&d.body["m_GameObject"]) else {
                continue;
            };
            let Some(ix) = tree.by_game_object(resolve(id(&go.file_id))) else {
                continue;
            };
            let me = id(&d.file_id);
            let owner = tree.nodes[ix].out.name.clone();
            match d.class_id {
                MESH_FILTER => {
                    tree.nodes[ix].filter = Some(me);
                    tree.nodes[ix].out.mesh = self.mesh(
                        obj_ref(&d.body["m_Mesh"]),
                        &owner,
                        "MeshFilter.m_Mesh",
                        Some(&local),
                    )?;
                }
                MESH_RENDERER | SKINNED_MESH_RENDERER => {
                    let skinned = d.class_id == SKINNED_MESH_RENDERER;
                    tree.nodes[ix].renderer = Some(me);
                    tree.nodes[ix].out.skinned = skinned;
                    if skinned {
                        tree.nodes[ix].out.mesh = self.mesh(
                            obj_ref(&d.body["m_Mesh"]),
                            &owner,
                            "SkinnedMeshRenderer.m_Mesh",
                            Some(&local),
                        )?;
                    }
                    let refs: Vec<Option<ObjRef>> = d.body["m_Materials"]
                        .as_vec()
                        .into_iter()
                        .flatten()
                        .map(obj_ref)
                        .collect();
                    let mut mats = Vec::with_capacity(refs.len());
                    for r in refs {
                        mats.push(self.material(r, &owner)?);
                    }
                    tree.nodes[ix].out.materials = mats;
                }
                _ => {
                    let controller = obj_ref(&d.body["m_Controller"]).and_then(|r| r.guid);
                    if let Some(g) = controller {
                        tree.nodes[ix].out.controller = self.guids.path(&g)?;
                    }
                }
            }
        }

        // Only a LOD Group's most detailed level is drawn: the others would stack on it.
        for d in docs
            .iter()
            .filter(|d| d.class_id == LOD_GROUP && !d.stripped)
        {
            let renderers = |lod: &yaml_rust2::Yaml| -> Vec<i64> {
                lod["renderers"]
                    .as_vec()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| obj_ref(&r["renderer"]))
                    .map(|r| resolve(id(&r.file_id)))
                    .collect()
            };
            let Some(lods) = d.body["m_LODs"].as_vec() else {
                continue;
            };
            let Some((first, rest)) = lods.split_first() else {
                continue;
            };
            let keep = renderers(first);
            for r in rest.iter().flat_map(renderers) {
                if keep.contains(&r) {
                    continue;
                }
                if let Some(ix) = tree.find(|n| n.renderer == Some(r)) {
                    tree.nodes[ix].out.mesh = None;
                    tree.nodes[ix].out.materials.clear();
                }
            }
        }
        Ok(tree)
    }

    /// The nodes a PrefabInstance brings in, its modifications applied.
    fn instance(&mut self, d: &Doc, depth: usize) -> Result<Option<Tree>, Error> {
        let Some(src_guid) = obj_ref(&d.body["m_SourcePrefab"]).and_then(|r| r.guid) else {
            return Ok(None);
        };
        let Some(src) = self.guids.path(&src_guid)? else {
            self.missing.push(format!(
                "Nested prefab → guid {} (not in this version)",
                short(&src_guid)
            ));
            return Ok(None);
        };
        let m = &d.body["m_Modification"];
        let mods = modifications(&m["m_Modifications"]);
        let mut tree = match model_kind(&src) {
            Some(ModelKind::Prefab) if depth < MAX_DEPTH => {
                let key = (src.clone(), depth + 1);
                if let Some(tree) = self.nested.get(&key) {
                    tree.clone()
                } else {
                    let Some(text) = self.guids.files.read_text(&src)? else {
                        return Ok(None);
                    };
                    let tree = self.tree(&src, &text, depth + 1)?;
                    self.nested.insert(key, tree.clone());
                    tree
                }
            }
            Some(ModelKind::Prefab) => return Ok(None),
            Some(_) => self.model_instance(&src)?,
            None => return Ok(None),
        };
        for md in &mods {
            self.apply(&mut tree, md)?;
        }
        for r in m["m_RemovedComponents"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(obj_ref)
        {
            let x = id(&r.file_id);
            for n in &mut tree.nodes {
                if n.filter == Some(x) {
                    n.out.mesh = None;
                }
                if n.renderer == Some(x) {
                    n.out.materials.clear();
                    n.out.mesh = n.out.mesh.take().filter(|_| !n.out.skinned);
                }
            }
        }
        for r in m["m_RemovedGameObjects"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(obj_ref)
        {
            if let Some(ix) = tree.by_game_object(id(&r.file_id)) {
                remove_subtree(&mut tree, ix);
            }
        }
        Ok(Some(tree))
    }

    /// A model file dropped into a prefab: one node that draws the whole model.
    fn model_instance(&mut self, path: &str) -> Result<Tree, Error> {
        let meta = self
            .guids
            .files
            .read_text(&format!("{path}.meta"))?
            .map(|t| Meta::parse(&t))
            .unwrap_or_default();
        let stem = base_name(path);
        let name = stem.rsplit_once('.').map_or(stem, |(s, _)| s).to_owned();
        Ok(Tree {
            nodes: vec![Node {
                out: ModelNode {
                    name,
                    parent: None,
                    position: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                    active: true,
                    mesh: Some(MeshRef::Asset {
                        path: path.to_owned(),
                        name: None,
                        id: None,
                        scale: if meta.global_scale > 0.0 {
                            meta.global_scale
                        } else {
                            1.0
                        },
                    }),
                    materials: Vec::new(),
                    skinned: false,
                    controller: None,
                },
                transform: meta.root_transform.as_deref().map_or(-1, id),
                game_object: meta.root_game_object.as_deref().map_or(-1, id),
                filter: None,
                renderer: None,
                removed: false,
            }],
        })
    }

    fn apply(&mut self, tree: &mut Tree, md: &Modification) -> Result<(), Error> {
        let p = md.path.as_str();
        let value = || num(&Yaml::String(md.value.clone()));
        if let Some(axis) = p
            .strip_prefix("m_LocalPosition.")
            .or_else(|| p.strip_prefix("m_LocalScale."))
        {
            let Some(ix) = tree.by_transform(md.target) else {
                return Ok(());
            };
            let v = if p.starts_with("m_LocalPosition") {
                &mut tree.nodes[ix].out.position
            } else {
                &mut tree.nodes[ix].out.scale
            };
            if let (Some(i), Some(x)) = (axis_ix(axis), value()) {
                v[i] = x;
            }
        } else if let Some(axis) = p.strip_prefix("m_LocalRotation.") {
            if let (Some(ix), Some(i), Some(x)) =
                (tree.by_transform(md.target), axis_ix(axis), value())
            {
                tree.nodes[ix].out.rotation[i] = x;
            }
        } else if p == "m_Name" {
            if let Some(ix) = tree.by_game_object(md.target) {
                tree.nodes[ix].out.name = md.value.clone();
            }
        } else if p == "m_IsActive" {
            if let Some(ix) = tree.by_game_object(md.target) {
                tree.nodes[ix].out.active = md.value.trim() != "0";
            }
        } else if p == "m_Mesh" {
            if let Some(ix) =
                tree.find(|n| n.filter == Some(md.target) || n.renderer == Some(md.target))
            {
                let owner = tree.nodes[ix].out.name.clone();
                tree.nodes[ix].out.mesh =
                    self.mesh(md.reference.clone(), &owner, "m_Mesh", None)?;
            }
        } else if let Some(rest) = p.strip_prefix("m_Materials.Array.data[") {
            let slot: Option<usize> = rest.trim_end_matches(']').parse().ok();
            if let (Some(ix), Some(slot)) = (tree.find(|n| n.renderer == Some(md.target)), slot) {
                let owner = tree.nodes[ix].out.name.clone();
                let m = self.material(md.reference.clone(), &owner)?;
                let mats = &mut tree.nodes[ix].out.materials;
                if mats.len() <= slot {
                    mats.resize(slot + 1, m);
                }
                mats[slot] = m;
            }
        } else if p == "m_Materials.Array.size"
            && let (Some(ix), Some(n)) = (tree.find(|n| n.renderer == Some(md.target)), value())
        {
            let n = n.max(0.0) as usize;
            let fill = self.material(None, "")?;
            tree.nodes[ix].out.materials.resize(n, fill);
        }
        Ok(())
    }

    fn mesh(
        &mut self,
        r: Option<ObjRef>,
        owner: &str,
        prop: &str,
        local: Option<&Local<'_>>,
    ) -> Result<Option<MeshRef>, Error> {
        let Some(r) = r else {
            return Ok(None);
        };
        if r.guid.is_none()
            && let Some(local) = local
            && local
                .docs
                .get(&id(&r.file_id))
                .is_some_and(|d| d.class_id == MESH)
        {
            return Ok(Some(MeshRef::Inline {
                path: local.path.to_owned(),
                id: r.file_id,
            }));
        }
        let guid = r.guid.clone().unwrap_or_default().to_ascii_lowercase();
        if guid == BUILTIN_RESOURCES {
            let name = match r.file_id.as_str() {
                "10202" => "Cube",
                "10206" => "Cylinder",
                "10207" => "Sphere",
                "10208" => "Capsule",
                "10209" => "Plane",
                "10210" => "Quad",
                _ => return Ok(None),
            };
            return Ok(Some(MeshRef::Builtin { name: name.into() }));
        }
        let Some(path) = self.guids.path(&guid)? else {
            self.missing.push(format!(
                "{owner} › {prop} → guid {} (not in this version)",
                short(&guid)
            ));
            return Ok(Some(MeshRef::Missing));
        };
        let meta = self
            .guids
            .files
            .read_text(&format!("{path}.meta"))?
            .map(|t| Meta::parse(&t))
            .unwrap_or_default();
        let scale = if meta.global_scale > 0.0 {
            meta.global_scale
        } else {
            1.0
        };
        let name = meta.names.get(&r.file_id).cloned();
        Ok(Some(MeshRef::Asset {
            path,
            name,
            id: Some(r.file_id),
            scale,
        }))
    }

    fn material(&mut self, r: Option<ObjRef>, owner: &str) -> Result<usize, Error> {
        let key = r.as_ref().map_or_else(
            || "none".to_owned(),
            |r| format!("{}:{}", r.guid.as_deref().unwrap_or(""), r.file_id),
        );
        if let Some(&ix) = self.material_ix.get(&key) {
            return Ok(ix);
        }
        let m = match &r {
            None => Material::missing("(none)"),
            Some(r)
                if r.guid
                    .as_deref()
                    .is_some_and(|g| g.eq_ignore_ascii_case(BUILTIN_EXTRA)) =>
            {
                let mut m = Material::missing(if r.file_id == DEFAULT_MATERIAL {
                    "Default-Material"
                } else {
                    "Built-in material"
                });
                m.missing = false;
                m.unlit = false;
                m.shader = "Standard".into();
                m.color = [1.0; 4];
                m.smoothness = 0.5;
                m
            }
            Some(r) => {
                let guid = r.guid.clone().unwrap_or_default();
                match self.guids.path(&guid)? {
                    Some(p) => Material::load(&p, self.guids)?,
                    None => {
                        self.missing.push(format!(
                            "{owner} › Renderer.m_Materials → guid {} (not in this version)",
                            short(&guid)
                        ));
                        Material::missing("(missing)")
                    }
                }
            }
        };
        let ix = self.materials.len();
        self.materials.push(m);
        self.material_ix.insert(key, ix);
        Ok(ix)
    }
}

fn axis_ix(axis: &str) -> Option<usize> {
    match axis {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        "w" => Some(3),
        _ => None,
    }
}

fn short(guid: &str) -> String {
    if guid.len() > 10 {
        format!("{}…{}", &guid[..4], &guid[guid.len() - 3..])
    } else {
        guid.to_owned()
    }
}

fn remove_subtree(tree: &mut Tree, root: usize) {
    tree.nodes[root].removed = true;
    loop {
        let mut changed = false;
        for i in 0..tree.nodes.len() {
            if !tree.nodes[i].removed
                && let Some(p) = tree.nodes[i].out.parent
                && tree.nodes[p].removed
            {
                tree.nodes[i].removed = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

struct Modification {
    target: i64,
    path: String,
    value: String,
    reference: Option<ObjRef>,
}

fn modifications(list: &Yaml) -> Vec<Modification> {
    list.as_vec()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            Some(Modification {
                target: id(&obj_ref(&m["target"])?.file_id),
                path: scalar(&m["propertyPath"])?,
                value: scalar(&m["value"]).unwrap_or_default(),
                reference: obj_ref(&m["objectReference"]),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Version;
    use crate::model::tree::Files;
    use crate::testing::TestRepo;

    const BOAT: &str = "b0a70000000000000000000000000001";
    const FBX: &str = "f0b10000000000000000000000000002";
    const PROP: &str = "f0b10000000000000000000000000003";
    const RED: &str = "ed000000000000000000000000000004";
    const NAVY: &str = "a5000000000000000000000000000005";
    const CTRL: &str = "c0000000000000000000000000000006";
    const HEADER: &str = "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n";

    fn go(id: u32, name: &str, comps: &[u32]) -> String {
        let c: String = comps
            .iter()
            .map(|c| format!("  - component: {{fileID: {c}}}\n"))
            .collect();
        format!(
            "--- !u!1 &{id}\nGameObject:\n  m_Name: {name}\n  m_IsActive: 1\n  m_Component:\n{c}"
        )
    }

    fn tf(id: u32, go: u32, father: u32, pos: &str, rot: &str) -> String {
        format!(
            "--- !u!4 &{id}\nTransform:\n  m_GameObject: {{fileID: {go}}}\n  m_LocalRotation: {rot}\n  m_LocalPosition: {pos}\n  m_LocalScale: {{x: 1, y: 1, z: 1}}\n  m_Father: {{fileID: {father}}}\n"
        )
    }

    fn boat() -> String {
        let id_rot = "{x: 0, y: 0, z: 0, w: 1}";
        [
            HEADER.to_owned(),
            go(100, "Skiff", &[400, 9500]),
            tf(400, 100, 0, "{x: 1, y: 0, z: 0}", id_rot),
            format!("--- !u!95 &9500\nAnimator:\n  m_GameObject: {{fileID: 100}}\n  m_Controller: {{fileID: 9100000, guid: {CTRL}, type: 2}}\n"),
            go(101, "Hull", &[401, 3301, 2301]),
            tf(401, 101, 400, "{x: 2, y: 3, z: 4}", "{x: 0, y: 0.5, z: 0.25, w: 0.8}"),
            format!("--- !u!33 &3301\nMeshFilter:\n  m_GameObject: {{fileID: 101}}\n  m_Mesh: {{fileID: -4216859302048453862, guid: {FBX}, type: 3}}\n"),
            format!("--- !u!23 &2301\nMeshRenderer:\n  m_GameObject: {{fileID: 101}}\n  m_Materials:\n  - {{fileID: 2100000, guid: {RED}, type: 2}}\n"),
            go(102, "Crate", &[402, 3302, 2302]),
            tf(402, 102, 400, "{x: 0, y: 1, z: 0}", id_rot),
            "--- !u!33 &3302\nMeshFilter:\n  m_GameObject: {fileID: 102}\n  m_Mesh: {fileID: 10202, guid: 0000000000000000e000000000000000, type: 0}\n".into(),
            "--- !u!23 &2302\nMeshRenderer:\n  m_GameObject: {fileID: 102}\n  m_Materials:\n  - {fileID: 10303, guid: 0000000000000000f000000000000000, type: 0}\n".into(),
            go(103, "Lost", &[403, 3303]),
            tf(403, 103, 400, "{x: 0, y: 0, z: 0}", id_rot),
            "--- !u!33 &3303\nMeshFilter:\n  m_GameObject: {fileID: 103}\n  m_Mesh: {fileID: 4300000, guid: dead0000000000000000000000000007, type: 3}\n".into(),
        ]
        .concat()
    }

    fn variant() -> String {
        let m = |target: &str, path: &str, value: &str, reference: &str| {
            format!(
                "    - target: {{fileID: {target}, guid: {BOAT}, type: 3}}\n      propertyPath: {path}\n      value: {value}\n      objectReference: {reference}\n"
            )
        };
        [
            HEADER.to_owned(),
            "--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {fileID: 0}\n    m_Modifications:\n".into(),
            m("400", "m_LocalPosition.x", "5", "{fileID: 0}"),
            m("102", "m_Name", "Box", "{fileID: 0}"),
            m("2301", "m_Materials.Array.data[0]", "", &format!("{{fileID: 2100000, guid: {NAVY}, type: 2}}")),
            format!("    m_RemovedComponents: []\n    m_RemovedGameObjects:\n    - {{fileID: 103, guid: {BOAT}, type: 3}}\n  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"),
            format!("--- !u!4 &8401 stripped\nTransform:\n  m_CorrespondingSourceObject: {{fileID: 401, guid: {BOAT}, type: 3}}\n  m_PrefabInstance: {{fileID: 7000}}\n"),
            go(200, "Mast", &[500]),
            tf(500, 200, 8401, "{x: 0, y: 1, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            format!("--- !u!1001 &7100\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 8401}}\n    m_Modifications:\n    - target: {{fileID: -8679921383154817045, guid: {PROP}, type: 3}}\n      propertyPath: m_LocalPosition.z\n      value: 2\n      objectReference: {{fileID: 0}}\n  m_SourcePrefab: {{fileID: 100100000, guid: {PROP}, type: 3}}\n"),
        ]
        .concat()
    }

    fn project() -> TestRepo {
        let mat = |name: &str| {
            format!(
                "--- !u!21 &2100000\nMaterial:\n  m_Name: {name}\n  m_Shader: {{fileID: 46, guid: 0000000000000000f000000000000000, type: 0}}\n"
            )
        };
        let meta = |guid: &str| format!("fileFormatVersion: 2\nguid: {guid}\n");
        let repo = TestRepo::init();
        repo.write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.1f1\n")
            .write("Assets/Boat.prefab", &boat())
            .write("Assets/Boat.prefab.meta", &meta(BOAT))
            .write("Assets/BoatNavy.prefab", &variant())
            .write("Assets/hull.fbx", "fbx")
            .write("Assets/hull.fbx.meta", &format!("guid: {FBX}\nModelImporter:\n  internalIDToNameTable:\n  - first:\n      43: -4216859302048453862\n    second: Hull_Mesh\n  meshes:\n    globalScale: 1\n"))
            .write("Assets/prop.fbx", "fbx")
            .write("Assets/prop.fbx.meta", &format!("guid: {PROP}\nModelImporter:\n  internalIDToNameTable:\n  - first:\n      4: -8679921383154817045\n    second: //RootNode\n  meshes:\n    globalScale: 0.5\n"))
            .write("Assets/red.mat", &mat("Red"))
            .write("Assets/red.mat.meta", &meta(RED))
            .write("Assets/navy.mat", &mat("Navy"))
            .write("Assets/navy.mat.meta", &meta(NAVY))
            .write("Assets/Boat.controller", "ctrl")
            .write("Assets/Boat.controller.meta", &meta(CTRL))
            .commit_all("boat");
        repo
    }

    fn build(repo: &TestRepo, path: &str) -> (ModelScene, Vec<Material>) {
        let r = crate::repo::open(repo.path()).unwrap();
        let v = Version::Head;
        let files = Files::new(&r, &v).unwrap();
        let guids = Guids::new(&files);
        PrefabBuilder::new(&guids).build(path).unwrap()
    }

    fn named<'s>(scene: &'s ModelScene, name: &str) -> (usize, &'s ModelNode) {
        scene
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.name == name)
            .unwrap()
    }

    #[test]
    fn assembles_a_prefab_in_right_handed_space() {
        let repo = project();
        let (scene, mats) = build(&repo, "Assets/Boat.prefab");
        assert_eq!(scene.nodes.len(), 4);
        let (skiff_ix, skiff) = named(&scene, "Skiff");
        assert_eq!(skiff.parent, None);
        assert_eq!(skiff.position, [-1.0, 0.0, 0.0]);
        assert_eq!(skiff.controller.as_deref(), Some("Assets/Boat.controller"));
        let (_, hull) = named(&scene, "Hull");
        assert_eq!(hull.parent, Some(skiff_ix));
        assert_eq!(hull.position, [-2.0, 3.0, 4.0]);
        assert_eq!(hull.rotation, [0.0, -0.5, -0.25, 0.8]);
        assert_eq!(
            hull.mesh,
            Some(MeshRef::Asset {
                path: "Assets/hull.fbx".into(),
                name: Some("Hull_Mesh".into()),
                id: Some("-4216859302048453862".into()),
                scale: 1.0
            })
        );
        assert_eq!(mats[hull.materials[0]].name, "Red");
        let (_, crate_) = named(&scene, "Crate");
        assert_eq!(
            crate_.mesh,
            Some(MeshRef::Builtin {
                name: "Cube".into()
            })
        );
        assert_eq!(mats[crate_.materials[0]].name, "Default-Material");
        assert!(!mats[crate_.materials[0]].missing);
        let (_, lost) = named(&scene, "Lost");
        assert_eq!(lost.mesh, Some(MeshRef::Missing));
        assert_eq!(
            scene.missing,
            vec!["Lost › MeshFilter.m_Mesh → guid dead…007 (not in this version)".to_owned()]
        );
    }

    #[test]
    fn draws_only_the_first_level_of_a_lod_group() {
        let repo = project();
        let lod = |go_id: u32, name: &str, renderer: u32| {
            [
                go(go_id, name, &[go_id + 300, go_id + 3300, renderer]),
                tf(go_id + 300, go_id, 400, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
                format!("--- !u!33 &{}\nMeshFilter:\n  m_GameObject: {{fileID: {go_id}}}\n  m_Mesh: {{fileID: 10202, guid: 0000000000000000e000000000000000, type: 0}}\n", go_id + 3300),
                format!("--- !u!23 &{renderer}\nMeshRenderer:\n  m_GameObject: {{fileID: {go_id}}}\n  m_Materials: []\n"),
            ]
            .concat()
        };
        let text = [
            HEADER.to_owned(),
            go(100, "Pillar", &[400, 20500]),
            tf(400, 100, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            lod(110, "Pillar_LOD0", 2310),
            lod(120, "Pillar_LOD1", 2320),
            lod(130, "Pillar_LOD2", 2330),
            "--- !u!205 &20500\nLODGroup:\n  m_GameObject: {fileID: 100}\n  m_LODs:\n  - screenRelativeHeight: 0.5\n    renderers:\n    - renderer: {fileID: 2310}\n  - screenRelativeHeight: 0.2\n    renderers:\n    - renderer: {fileID: 2320}\n  - screenRelativeHeight: 0.05\n    renderers:\n    - renderer: {fileID: 2330}\n".into(),
        ]
        .concat();
        repo.write("Assets/Pillar.prefab", &text).commit_all("lod");
        let (scene, _) = build(&repo, "Assets/Pillar.prefab");
        assert!(named(&scene, "Pillar_LOD0").1.mesh.is_some());
        assert_eq!(named(&scene, "Pillar_LOD1").1.mesh, None);
        assert_eq!(named(&scene, "Pillar_LOD2").1.mesh, None);
        assert_eq!(
            scene.nodes.len(),
            4,
            "the objects stay, only their meshes go"
        );
    }

    #[test]
    fn assembles_a_scene_like_a_prefab() {
        let repo = project();
        let scene = [
            HEADER.to_owned(),
            "--- !u!29 &1\nOcclusionCullingSettings:\n  m_ObjectHideFlags: 0\n--- !u!104 &2\nRenderSettings:\n  m_Fog: 0\n".into(),
            go(10, "Pier", &[11]),
            tf(11, 10, 0, "{x: 3, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            format!("--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications: []\n  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"),
        ]
        .concat();
        repo.write("Assets/Harbour.unity", &scene)
            .commit_all("scene");
        let (scene, _) = build(&repo, "Assets/Harbour.unity");
        assert_eq!(named(&scene, "Pier").1.position, [-3.0, 0.0, 0.0]);
        assert_eq!(named(&scene, "Skiff").1.parent, None);
        assert!(named(&scene, "Hull").1.parent.is_some());
        assert_eq!(
            super::super::model_kind("Assets/Harbour.unity"),
            Some(super::super::ModelKind::Prefab)
        );
    }

    #[test]
    fn applies_a_variants_overrides_and_nested_instances() {
        let repo = project();
        let (scene, mats) = build(&repo, "Assets/BoatNavy.prefab");
        let names: Vec<&str> = scene.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Mast", "Skiff", "Hull", "Box", "prop"]);
        let (_, skiff) = named(&scene, "Skiff");
        assert_eq!(skiff.position, [-5.0, 0.0, 0.0]);
        let (hull_ix, hull) = named(&scene, "Hull");
        assert_eq!(mats[hull.materials[0]].name, "Navy");
        assert_eq!(named(&scene, "Mast").1.parent, Some(hull_ix));
        let (_, prop) = named(&scene, "prop");
        assert_eq!(prop.parent, Some(hull_ix));
        assert_eq!(prop.position, [0.0, 0.0, 2.0]);
        assert_eq!(
            prop.mesh,
            Some(MeshRef::Asset {
                path: "Assets/prop.fbx".into(),
                name: None,
                id: None,
                scale: 0.5
            })
        );
    }

    #[test]
    fn keeps_each_instance_of_a_nested_prefab_apart() {
        let repo = project();
        let instance = |id: u32, name: &str, x: &str| {
            format!(
                "--- !u!1001 &{id}\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications:\n    - target: {{fileID: 100, guid: {BOAT}, type: 3}}\n      propertyPath: m_Name\n      value: {name}\n      objectReference: {{fileID: 0}}\n    - target: {{fileID: 400, guid: {BOAT}, type: 3}}\n      propertyPath: m_LocalPosition.x\n      value: {x}\n      objectReference: {{fileID: 0}}\n    m_RemovedGameObjects:\n    - {{fileID: 103, guid: {BOAT}, type: 3}}\n  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"
            )
        };
        let fleet = [
            HEADER.to_owned(),
            instance(7000, "Left", "5"),
            instance(7001, "Right", "7"),
        ]
        .concat();
        repo.write("Assets/Fleet.prefab", &fleet)
            .commit_all("fleet");

        let (scene, _) = build(&repo, "Assets/Fleet.prefab");
        assert_eq!(named(&scene, "Left").1.position, [-5.0, 0.0, 0.0]);
        assert_eq!(named(&scene, "Right").1.position, [-7.0, 0.0, 0.0]);
        let hulls = scene.nodes.iter().filter(|n| n.name == "Hull").count();
        assert_eq!(hulls, 2);
        assert!(
            !scene
                .nodes
                .iter()
                .any(|n| n.name == "Lost" || n.name == "Skiff")
        );
    }

    #[test]
    fn nested_ids_match_unitys_scheme() {
        assert_eq!(nested_id(7000, 401), 7000 ^ 401);
        assert!(nested_id(-5, 3) >= 0);
    }
}
