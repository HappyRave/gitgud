use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

use super::guids::{Guids, short};
use super::material::{BUILTIN_EXTRA, Material};
use super::mesh_asset::{self, MESH_CLASS as MESH};
use super::meta::Meta;
use super::tree::base_name;
use super::yaml::{Doc, ObjRef, docs_reading, num, obj_ref, quat, scalar, vec3};
use super::{ModelKind, model_kind};
use crate::repo::Error;

const BUILTIN_RESOURCES: &str = "0000000000000000e000000000000000";
const DEFAULT_MATERIAL: &str = "10303";
const MAX_DEPTH: usize = 12;
/// More material slots than a renderer has in practice: a larger override is corrupt.
const MAX_MATERIALS: usize = 1024;

const GAME_OBJECT: u32 = 1;
const TRANSFORM: u32 = 4;
const RECT_TRANSFORM: u32 = 224;
const MESH_RENDERER: u32 = 23;
const MESH_FILTER: u32 = 33;
const ANIMATOR: u32 = 95;
const SKINNED_MESH_RENDERER: u32 = 137;
const PREFAB_INSTANCE: u32 = 1001;
const LOD_GROUP: u32 = 205;
const TERRAIN: u32 = 218;
/// The classes whose contents the builder reads; the rest only need their header.
const READ: [u32; 10] = [
    GAME_OBJECT,
    TRANSFORM,
    RECT_TRANSFORM,
    MESH_RENDERER,
    MESH_FILTER,
    ANIMATOR,
    SKINNED_MESH_RENDERER,
    PREFAB_INSTANCE,
    LOD_GROUP,
    TERRAIN,
];

/// The file being read and its documents, for meshes stored inside it.
struct Local<'d> {
    path: &'d str,
    docs: &'d HashMap<i64, &'d Doc>,
    /// Content keys of the file's Mesh documents, by `fileID`.
    mesh_keys: HashMap<String, String>,
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
    /// meshes), served as GLB by its `fileID`; `key` names its content, the same in
    /// every version that has the same mesh.
    Inline {
        path: String,
        id: String,
        key: String,
    },
    /// A Unity Terrain: its TerrainData asset (binary), served as GLB with its painted
    /// layers baked; `key` names the asset's content.
    Terrain { path: String, key: Option<String> },
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

/// Node lookups by `fileID`, built once the nodes are in (a scene has thousands).
#[derive(Default)]
struct Index {
    transform: HashMap<i64, usize>,
    game_object: HashMap<i64, usize>,
    filter: HashMap<i64, usize>,
    renderer: HashMap<i64, usize>,
}

impl Tree {
    fn index(&self) -> Index {
        let mut ix = Index::default();
        for (i, n) in self.nodes.iter().enumerate() {
            ix.transform.entry(n.transform).or_insert(i);
            ix.game_object.entry(n.game_object).or_insert(i);
            if let Some(f) = n.filter {
                ix.filter.entry(f).or_insert(i);
            }
            if let Some(r) = n.renderer {
                ix.renderer.entry(r).or_insert(i);
            }
        }
        ix
    }

    /// Marks the nodes at `roots` and everything under them removed.
    fn remove_subtrees(&mut self, roots: &[usize]) {
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(p) = n.out.parent {
                children[p].push(i);
            }
        }
        let mut todo = roots.to_vec();
        while let Some(i) = todo.pop() {
            if !self.nodes[i].removed {
                self.nodes[i].removed = true;
                todo.extend_from_slice(&children[i]);
            }
        }
    }
}

fn id(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

/// Unity's fileID for an object of a nested prefab, as seen from the outer prefab.
fn nested_id(instance: i64, inner: i64) -> i64 {
    (instance ^ inner) & i64::MAX
}

/// A node for each GameObject in `docs` that has a Transform, placed by it;
/// parents, meshes and materials come later.
fn game_object_nodes(docs: &[Doc], by_id: &HashMap<i64, &Doc>) -> Vec<Node> {
    let mut nodes = Vec::new();
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
        nodes.push(Node {
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
    nodes
}

/// Stripped objects stand for an object of a nested instance: their id → that
/// object's id as the outer prefab sees it.
fn stripped_aliases(docs: &[Doc]) -> HashMap<i64, i64> {
    let mut alias = HashMap::new();
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
    alias
}

/// Only a LOD Group's most detailed level is drawn: the others would stack on it.
fn drop_lower_lods(docs: &[Doc], tree: &mut Tree, alias: &HashMap<i64, i64>) {
    let resolve = |x: i64| alias.get(&x).copied().unwrap_or(x);
    let by_renderer = tree.index().renderer;
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
            if let Some(&ix) = by_renderer.get(&r) {
                tree.nodes[ix].out.mesh = None;
                tree.nodes[ix].out.materials.clear();
            }
        }
    }
}

pub(crate) struct PrefabBuilder<'a, 'g> {
    guids: &'g Guids<'a>,
    pub materials: Vec<Material>,
    material_ix: HashMap<String, usize>,
    pub missing: Vec<String>,
    /// Nested prefabs already assembled, by path: a prefab is often placed many times.
    nested: HashMap<String, Tree>,
    /// The prefabs being assembled, outermost first, so one that places itself is caught.
    building: Vec<String>,
    /// Model `.meta`s already read, by model path: a scene names the same few thousands of times.
    metas: HashMap<String, std::rc::Rc<Meta>>,
    /// `.mat` files to load into `materials` once the walk is done, by index: their
    /// files are then read together.
    unloaded: Vec<(usize, String)>,
    /// Nested prefabs read and parsed ahead of the walk, by path: text and documents.
    parsed: HashMap<String, (String, Vec<Doc>)>,
}

impl<'a, 'g> PrefabBuilder<'a, 'g> {
    pub fn new(guids: &'g Guids<'a>) -> Self {
        Self {
            guids,
            materials: Vec::new(),
            material_ix: HashMap::new(),
            missing: Vec::new(),
            nested: HashMap::new(),
            building: Vec::new(),
            metas: HashMap::new(),
            unloaded: Vec::new(),
            parsed: HashMap::new(),
        }
    }

    /// The `.meta` of model `path`, read once per build (empty when there is none).
    fn model_meta(&mut self, path: &str) -> Result<std::rc::Rc<Meta>, Error> {
        if let Some(meta) = self.metas.get(path) {
            return Ok(meta.clone());
        }
        let meta = std::rc::Rc::new(
            self.guids
                .files
                .read_text(&format!("{path}.meta"))?
                .map(|t| Meta::parse(&t))
                .unwrap_or_default(),
        );
        self.metas.insert(path.to_owned(), meta.clone());
        Ok(meta)
    }

    /// Assembles the prefab at `path`.
    pub fn build(mut self, path: &str) -> Result<(ModelScene, Vec<Material>), Error> {
        let text = self
            .guids
            .files
            .read_text(path)?
            .ok_or_else(|| Error::Refused(format!("{path} is not in that version")))?;
        let docs = parse(&text);
        self.building.push(path.to_owned());
        let tree = self.tree(path, &text, docs, 0)?;
        let unloaded = std::mem::take(&mut self.unloaded);
        let paths: Vec<String> = unloaded.iter().map(|(_, p)| p.clone()).collect();
        self.guids.files.prefetch(&paths);
        for (ix, p) in unloaded {
            self.materials[ix] = Material::load(&p, self.guids)?;
        }
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

    /// Reads and parses the prefabs `docs` place, all at once and in parallel, for
    /// the walk to find in `parsed`.
    fn parse_nested(&mut self, docs: &[Doc], depth: usize) -> Result<(), Error> {
        use rayon::prelude::*;
        if depth >= MAX_DEPTH {
            return Ok(());
        }
        let mut paths = Vec::new();
        for d in docs
            .iter()
            .filter(|d| d.class_id == PREFAB_INSTANCE && !d.stripped)
        {
            let Some(guid) = obj_ref(&d.body["m_SourcePrefab"]).and_then(|r| r.guid) else {
                continue;
            };
            if let Some(src) = self.guids.path(&guid)?
                && model_kind(&src) == Some(ModelKind::Prefab)
                && !self.nested.contains_key(&src)
                && !self.building.contains(&src)
                && !self.parsed.contains_key(&src)
                && !paths.contains(&src)
            {
                paths.push(src);
            }
        }
        self.guids.files.prefetch(&paths);
        let mut texts = Vec::with_capacity(paths.len());
        for p in paths {
            if let Some(text) = self.guids.files.read_text(&p)? {
                texts.push((p, text));
            }
        }
        let parsed: Vec<_> = texts
            .into_par_iter()
            .map(|(p, text)| {
                let docs = parse(&text);
                (p, (text, docs))
            })
            .collect();
        self.parsed.extend(parsed);
        Ok(())
    }

    fn tree(
        &mut self,
        path: &str,
        text: &str,
        docs: Vec<Doc>,
        depth: usize,
    ) -> Result<Tree, Error> {
        self.parse_nested(&docs, depth)?;
        let by_id: HashMap<i64, &Doc> = docs.iter().map(|d| (id(&d.file_id), d)).collect();
        let mesh_keys = if docs.iter().any(|d| d.class_id == MESH) {
            mesh_asset::mesh_documents(text)
                .into_iter()
                .map(|(id, range)| (id, mesh_asset::mesh_key(&text[range])))
                .collect()
        } else {
            HashMap::new()
        };
        let local = Local {
            path,
            docs: &by_id,
            mesh_keys,
        };
        let mut tree = Tree {
            nodes: game_object_nodes(&docs, &by_id),
        };

        let mut instance_roots: Vec<(usize, i64)> = Vec::new();
        for d in docs
            .iter()
            .filter(|d| d.class_id == PREFAB_INSTANCE && !d.stripped)
        {
            let instance = id(&d.file_id);
            let m = &d.body["m_Modification"];
            let parent = obj_ref(&m["m_TransformParent"]).map(|r| id(&r.file_id));
            let first = tree.nodes.len();
            match self.instance(d, depth, &local)? {
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

        let alias = stripped_aliases(&docs);
        let resolve = |x: i64| alias.get(&x).copied().unwrap_or(x);
        let ix = tree.index();
        let by_transform = |x: i64| ix.transform.get(&x).copied();

        for (i, p) in instance_roots {
            tree.nodes[i].out.parent = by_transform(resolve(p));
        }
        for d in docs
            .iter()
            .filter(|d| matches!(d.class_id, TRANSFORM | RECT_TRANSFORM) && !d.stripped)
        {
            let Some(child) = by_transform(id(&d.file_id)) else {
                continue;
            };
            if let Some(f) = obj_ref(&d.body["m_Father"]) {
                tree.nodes[child].out.parent = by_transform(resolve(id(&f.file_id)));
            }
        }

        self.attach_components(&docs, &mut tree, &ix.game_object, &alias, &local)?;
        drop_lower_lods(&docs, &mut tree, &alias);
        Ok(tree)
    }

    /// Fills each node's mesh, materials, skinning, terrain and animator from the
    /// components in `docs` that belong to its GameObject (`game_objects`: id → node).
    fn attach_components(
        &mut self,
        docs: &[Doc],
        tree: &mut Tree,
        game_objects: &HashMap<i64, usize>,
        alias: &HashMap<i64, i64>,
        local: &Local<'_>,
    ) -> Result<(), Error> {
        let resolve = |x: i64| alias.get(&x).copied().unwrap_or(x);
        for d in docs.iter().filter(|d| !d.stripped) {
            if !matches!(
                d.class_id,
                MESH_FILTER | MESH_RENDERER | SKINNED_MESH_RENDERER | ANIMATOR | TERRAIN
            ) {
                continue;
            }
            let Some(go) = obj_ref(&d.body["m_GameObject"]) else {
                continue;
            };
            let Some(&ix) = game_objects.get(&resolve(id(&go.file_id))) else {
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
                        Some(local),
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
                            Some(local),
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
                TERRAIN => {
                    tree.nodes[ix].out.mesh =
                        self.terrain(obj_ref(&d.body["m_TerrainData"]), &owner)?;
                }
                _ => {
                    let controller = obj_ref(&d.body["m_Controller"]).and_then(|r| r.guid);
                    if let Some(g) = controller {
                        tree.nodes[ix].out.controller = self.guids.path(&g)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The nodes a PrefabInstance brings in, its modifications applied.
    /// `local` is the file placing the instance, for overrides naming its own meshes.
    fn instance(
        &mut self,
        d: &Doc,
        depth: usize,
        local: &Local<'_>,
    ) -> Result<Option<Tree>, Error> {
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
            Some(ModelKind::Prefab) => match self.nested_tree(&src, depth)? {
                Some(tree) => tree,
                None => return Ok(None),
            },
            Some(_) => self.model_instance(&src)?,
            None => return Ok(None),
        };
        let ix = tree.index();
        for md in &mods {
            self.apply(&mut tree, &ix, md, local)?;
        }
        for r in m["m_RemovedComponents"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(obj_ref)
        {
            let x = id(&r.file_id);
            if let Some(&i) = ix.filter.get(&x) {
                tree.nodes[i].out.mesh = None;
            }
            if let Some(&i) = ix.renderer.get(&x) {
                let n = &mut tree.nodes[i].out;
                n.materials.clear();
                if n.skinned {
                    n.mesh = None;
                }
            }
        }
        let removed: Vec<usize> = m["m_RemovedGameObjects"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(obj_ref)
            .filter_map(|r| ix.game_object.get(&id(&r.file_id)).copied())
            .collect();
        if !removed.is_empty() {
            tree.remove_subtrees(&removed);
        }
        Ok(Some(tree))
    }

    /// The nested prefab `src`, assembled once; `None` (and a missing entry) when it
    /// places itself or nests too deep, or isn't in this version.
    fn nested_tree(&mut self, src: &str, depth: usize) -> Result<Option<Tree>, Error> {
        if let Some(tree) = self.nested.get(src) {
            return Ok(Some(tree.clone()));
        }
        if self.building.iter().any(|b| b == src) {
            self.missing
                .push(format!("Nested prefab {src} places itself"));
            return Ok(None);
        }
        if depth >= MAX_DEPTH {
            self.missing
                .push(format!("{src} is nested more than {MAX_DEPTH} levels deep"));
            return Ok(None);
        }
        let (text, docs) = match self.parsed.remove(src) {
            Some(parsed) => parsed,
            None => {
                let Some(text) = self.guids.files.read_text(src)? else {
                    return Ok(None);
                };
                let docs = parse(&text);
                (text, docs)
            }
        };
        self.building.push(src.to_owned());
        let tree = self.tree(src, &text, docs, depth + 1);
        self.building.pop();
        let tree = tree?;
        self.nested.insert(src.to_owned(), tree.clone());
        Ok(Some(tree))
    }

    /// A model file dropped into a prefab: one node that draws the whole model.
    fn model_instance(&mut self, path: &str) -> Result<Tree, Error> {
        let meta = self.model_meta(path)?;
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
                        scale: meta.global_scale,
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

    fn apply(
        &mut self,
        tree: &mut Tree,
        ix: &Index,
        md: &Modification,
        local: &Local<'_>,
    ) -> Result<(), Error> {
        let p = md.path.as_str();
        let value = || num(&Yaml::String(md.value.clone()));
        let transform = ix.transform.get(&md.target).copied();
        let game_object = ix.game_object.get(&md.target).copied();
        let renderer = ix.renderer.get(&md.target).copied();
        if let Some(axis) = p
            .strip_prefix("m_LocalPosition.")
            .or_else(|| p.strip_prefix("m_LocalScale."))
        {
            let Some(t) = transform else {
                return Ok(());
            };
            let v = if p.starts_with("m_LocalPosition") {
                &mut tree.nodes[t].out.position
            } else {
                &mut tree.nodes[t].out.scale
            };
            if let (Some(i), Some(x)) = (axis_ix(axis), value()) {
                v[i] = x;
            }
        } else if let Some(axis) = p.strip_prefix("m_LocalRotation.") {
            if let (Some(t), Some(i), Some(x)) = (transform, axis_ix(axis), value()) {
                tree.nodes[t].out.rotation[i] = x;
            }
        } else if p == "m_Name" {
            if let Some(g) = game_object {
                tree.nodes[g].out.name = md.value.clone();
            }
        } else if p == "m_IsActive" {
            if let Some(g) = game_object {
                tree.nodes[g].out.active = md.value.trim() != "0";
            }
        } else if p == "m_Mesh" {
            if let Some(n) = ix.filter.get(&md.target).copied().or(renderer) {
                let owner = tree.nodes[n].out.name.clone();
                tree.nodes[n].out.mesh =
                    self.mesh(md.reference.clone(), &owner, "m_Mesh", Some(local))?;
            }
        } else if let Some(rest) = p.strip_prefix("m_Materials.Array.data[") {
            let Some(r) = renderer else {
                return Ok(());
            };
            let owner = tree.nodes[r].out.name.clone();
            let slot = rest
                .trim_end_matches(']')
                .parse::<usize>()
                .ok()
                .filter(|&s| s < MAX_MATERIALS);
            let Some(slot) = slot else {
                self.missing
                    .push(format!("{owner} › Renderer.{p}: not a material slot"));
                return Ok(());
            };
            let m = self.material(md.reference.clone(), &owner)?;
            // Slots between the old count and this one are empty, not copies of it.
            let empty = self.material(None, "")?;
            let mats = &mut tree.nodes[r].out.materials;
            if mats.len() <= slot {
                mats.resize(slot + 1, empty);
            }
            mats[slot] = m;
        } else if p == "m_Materials.Array.size"
            && let (Some(r), Some(n)) = (renderer, value())
        {
            if !(0.0..=MAX_MATERIALS as f32).contains(&n) {
                let owner = &tree.nodes[r].out.name;
                self.missing.push(format!(
                    "{owner} › Renderer.{p} = {n}: not a material count"
                ));
                return Ok(());
            }
            let fill = self.material(None, "")?;
            tree.nodes[r].out.materials.resize(n as usize, fill);
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
            let Some(key) = local.mesh_keys.get(&r.file_id) else {
                return Err(Error::Read(format!(
                    "{}: Mesh &{} wasn't indexed",
                    local.path, r.file_id
                )));
            };
            return Ok(Some(MeshRef::Inline {
                path: local.path.to_owned(),
                key: key.clone(),
                id: r.file_id,
            }));
        }
        let Some(guid) = r.guid.as_deref().map(str::to_ascii_lowercase) else {
            self.missing.push(format!(
                "{owner} › {prop} → &{} (not a Mesh in this file)",
                r.file_id
            ));
            return Ok(Some(MeshRef::Missing));
        };
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
        // A Mesh stored in another YAML file: served by its `fileID` like this file's own.
        if matches!(
            model_kind(&path),
            Some(ModelKind::UnityMesh | ModelKind::Prefab)
        ) {
            let Some(key) = self.guids.files.key(&path) else {
                self.missing.push(format!(
                    "{owner} › {prop} → {path} (not readable in this version)"
                ));
                return Ok(Some(MeshRef::Missing));
            };
            return Ok(Some(MeshRef::Inline {
                key: format!("{key}:{}", r.file_id),
                path,
                id: r.file_id,
            }));
        }
        let meta = self.model_meta(&path)?;
        let name = meta.names.get(&r.file_id).cloned();
        Ok(Some(MeshRef::Asset {
            path,
            name,
            id: Some(r.file_id),
            scale: meta.global_scale,
        }))
    }

    /// A Terrain's mesh: its TerrainData asset, or missing.
    fn terrain(&mut self, r: Option<ObjRef>, owner: &str) -> Result<Option<MeshRef>, Error> {
        let Some(guid) = r.and_then(|r| r.guid) else {
            return Ok(None);
        };
        let Some(path) = self.guids.path(&guid)? else {
            self.missing.push(format!(
                "{owner} › Terrain.m_TerrainData → guid {} (not in this version)",
                short(&guid)
            ));
            return Ok(Some(MeshRef::Missing));
        };
        let key = self.guids.files.key(&path);
        Ok(Some(MeshRef::Terrain { path, key }))
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
            // No guid: an empty slot ({fileID: 0}), or a material stored in this
            // file, which has no asset to look up.
            Some(r) if r.guid.is_none() => Material::missing(if id(&r.file_id) == 0 {
                "(none)"
            } else {
                "(material in this file)"
            }),
            Some(r) => {
                let guid = r.guid.clone().unwrap_or_default();
                match self.guids.path(&guid)? {
                    Some(p) => {
                        let m = Material::missing(base_name(&p));
                        // A material inside a model file has no YAML to read.
                        if !matches!(
                            model_kind(&p),
                            Some(
                                ModelKind::Fbx | ModelKind::Obj | ModelKind::Gltf | ModelKind::Glb
                            )
                        ) {
                            self.unloaded.push((self.materials.len(), p));
                        }
                        m
                    }
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

/// The documents of a prefab or scene the builder reads.
fn parse(text: &str) -> Vec<Doc> {
    docs_reading(text, |class, stripped| stripped || READ.contains(&class))
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
    fn loads_material_files_and_leaves_materials_inside_models_missing() {
        let repo = project();
        let prefab = [
            HEADER.to_owned(),
            go(100, "Deck", &[400, 2300]),
            tf(400, 100, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            format!("--- !u!23 &2300\nMeshRenderer:\n  m_GameObject: {{fileID: 100}}\n  m_Materials:\n  - {{fileID: 2100000, guid: {RED}, type: 2}}\n  - {{fileID: -3033667219593020291, guid: {FBX}, type: 3}}\n  - {{fileID: 2100000, guid: {NAVY}, type: 2}}\n"),
        ]
        .concat();
        repo.write("Assets/Deck.prefab", &prefab).commit_all("deck");
        let (scene, mats) = build(&repo, "Assets/Deck.prefab");
        let (_, deck) = named(&scene, "Deck");
        let names: Vec<(&str, bool)> = deck
            .materials
            .iter()
            .map(|&m| (mats[m].name.as_str(), mats[m].missing))
            .collect();
        assert_eq!(names, [("Red", false), ("hull.fbx", true), ("Navy", false)]);
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
    fn draws_a_terrain_from_its_terrain_data() {
        const DATA: &str = "7777aaaa000000000000000000000000";
        let repo = project();
        let terrain = |id: u32, guid: &str| {
            format!(
                "--- !u!218 &{id}\nTerrain:\n  m_GameObject: {{fileID: 30}}\n  m_TerrainData: {{fileID: 15600000, guid: {guid}, type: 2}}\n"
            )
        };
        let scene = [
            HEADER.to_owned(),
            go(30, "Ground", &[31, 32]),
            tf(
                31,
                30,
                0,
                "{x: -50, y: 0, z: -50}",
                "{x: 0, y: 0, z: 0, w: 1}",
            ),
            terrain(32, DATA),
            go(40, "Gone", &[41, 42]),
            tf(41, 40, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            terrain(42, "dead0000000000000000000000000007").replace("fileID: 30", "fileID: 40"),
        ]
        .concat();
        repo.write("Assets/Ground.asset", "binary terrain data")
            .write("Assets/Ground.asset.meta", &format!("guid: {DATA}\n"))
            .write("Assets/Island.unity", &scene)
            .commit_all("island");
        let (scene, _) = build(&repo, "Assets/Island.unity");
        let (_, ground) = named(&scene, "Ground");
        assert_eq!(ground.position, [50.0, 0.0, -50.0]);
        assert!(
            matches!(&ground.mesh, Some(MeshRef::Terrain { path, key: Some(_) }) if path == "Assets/Ground.asset"),
            "{:?}",
            ground.mesh
        );
        assert_eq!(named(&scene, "Gone").1.mesh, Some(MeshRef::Missing));
        assert_eq!(
            scene.missing,
            vec!["Gone › Terrain.m_TerrainData → guid dead…007 (not in this version)".to_owned()]
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
    fn reports_a_prefab_that_places_itself_instead_of_expanding_it() {
        const LOOP: &str = "1001000000000000000000000000000a";
        let repo = project();
        let instance = |id: u32| {
            format!(
                "--- !u!1001 &{id}\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications: []\n  m_SourcePrefab: {{fileID: 100100000, guid: {LOOP}, type: 3}}\n"
            )
        };
        let text = [
            HEADER.to_owned(),
            go(1, "Ring", &[2]),
            tf(2, 1, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            instance(7000),
            instance(7001),
            instance(7002),
        ]
        .concat();
        repo.write("Assets/Loop.prefab", &text)
            .write("Assets/Loop.prefab.meta", &format!("guid: {LOOP}\n"))
            .commit_all("loop");
        let (scene, _) = build(&repo, "Assets/Loop.prefab");
        assert_eq!(scene.nodes.len(), 1);
        assert_eq!(
            scene.missing,
            vec!["Nested prefab Assets/Loop.prefab places itself".to_owned(); 3]
        );
    }

    #[test]
    fn refuses_corrupt_material_overrides_and_odd_guids() {
        let repo = project();
        let m = |path: &str, value: &str| {
            format!(
                "    - target: {{fileID: 2301, guid: {BOAT}, type: 3}}\n      propertyPath: {path}\n      value: {value}\n      objectReference: {{fileID: 2100000, guid: {NAVY}, type: 2}}\n"
            )
        };
        let text = [
            HEADER.to_owned(),
            "--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {fileID: 0}\n    m_Modifications:\n".into(),
            m("m_Materials.Array.size", "1e30"),
            m("m_Materials.Array.data[18446744073709551615]", ""),
            m("m_Materials.Array.data[1]", ""),
            format!("  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"),
            go(300, "Odd", &[301, 3301]),
            tf(301, 300, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            "--- !u!33 &3301\nMeshFilter:\n  m_GameObject: {fileID: 300}\n  m_Mesh: {fileID: 4300000, guid: ééééééééééééé, type: 3}\n".into(),
        ]
        .concat();
        repo.write("Assets/Odd.prefab", &text).commit_all("odd");
        let (scene, mats) = build(&repo, "Assets/Odd.prefab");
        let (_, hull) = named(&scene, "Hull");
        let names: Vec<&str> = hull
            .materials
            .iter()
            .map(|&m| mats[m].name.as_str())
            .collect();
        assert_eq!(names, ["Red", "Navy"]);
        assert_eq!(named(&scene, "Odd").1.mesh, Some(MeshRef::Missing));
        assert_eq!(scene.missing.len(), 4, "{:?}", scene.missing);
        assert!(scene.missing[3].contains("éééé…ééé"), "{:?}", scene.missing);
    }

    #[test]
    fn a_material_set_past_the_end_leaves_the_slots_between_empty() {
        let repo = project();
        let text = [
            HEADER.to_owned(),
            format!("--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications:\n    - target: {{fileID: 2301, guid: {BOAT}, type: 3}}\n      propertyPath: m_Materials.Array.data[3]\n      value: \n      objectReference: {{fileID: 2100000, guid: {NAVY}, type: 2}}\n  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"),
        ]
        .concat();
        repo.write("Assets/Gap.prefab", &text).commit_all("gap");
        let (scene, mats) = build(&repo, "Assets/Gap.prefab");
        let (_, hull) = named(&scene, "Hull");
        let names: Vec<&str> = hull
            .materials
            .iter()
            .map(|&m| mats[m].name.as_str())
            .collect();
        assert_eq!(names, ["Red", "(none)", "(none)", "Navy"]);
        assert!(
            !scene.missing.iter().any(|m| m.contains("m_Materials")),
            "{:?}",
            scene.missing
        );
    }

    #[test]
    fn an_override_can_draw_a_mesh_stored_in_the_outer_file() {
        let repo = project();
        let text = [
            HEADER.to_owned(),
            format!("--- !u!1001 &7000\nPrefabInstance:\n  m_Modification:\n    m_TransformParent: {{fileID: 0}}\n    m_Modifications:\n    - target: {{fileID: 3301, guid: {BOAT}, type: 3}}\n      propertyPath: m_Mesh\n      value: \n      objectReference: {{fileID: 4300123}}\n  m_SourcePrefab: {{fileID: 100100000, guid: {BOAT}, type: 3}}\n"),
            mesh_asset::tri_mesh("4300123"),
        ]
        .concat();
        repo.write("Assets/Edited.prefab", &text)
            .commit_all("edited");
        let (scene, _) = build(&repo, "Assets/Edited.prefab");
        assert!(
            matches!(
                &named(&scene, "Hull").1.mesh,
                Some(MeshRef::Inline { path, id, .. })
                    if path == "Assets/Edited.prefab" && id == "4300123"
            ),
            "{:?}",
            named(&scene, "Hull").1.mesh
        );
    }

    #[test]
    fn a_mesh_in_another_yaml_file_is_served_by_its_file_id() {
        const MESHES: &str = "a55e7000000000000000000000000001";
        let repo = project();
        let text = [
            HEADER.to_owned(),
            go(1, "Rock", &[2, 3]),
            tf(2, 1, 0, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0, z: 0, w: 1}"),
            format!("--- !u!33 &3\nMeshFilter:\n  m_GameObject: {{fileID: 1}}\n  m_Mesh: {{fileID: 4300002, guid: {MESHES}, type: 2}}\n"),
        ]
        .concat();
        repo.write("Assets/Rock.prefab", &text)
            .write(
                "Assets/Rocks.asset",
                &format!(
                    "{HEADER}{}{}",
                    mesh_asset::tri_mesh("4300001"),
                    mesh_asset::tri_mesh("4300002")
                ),
            )
            .write("Assets/Rocks.asset.meta", &format!("guid: {MESHES}\n"))
            .commit_all("rocks");
        let (scene, _) = build(&repo, "Assets/Rock.prefab");
        assert!(
            matches!(
                &named(&scene, "Rock").1.mesh,
                Some(MeshRef::Inline { path, id, key })
                    if path == "Assets/Rocks.asset" && id == "4300002" && key.ends_with(":4300002")
            ),
            "{:?}",
            named(&scene, "Rock").1.mesh
        );
    }

    #[test]
    fn nested_ids_match_unitys_scheme() {
        assert_eq!(nested_id(7000, 401), 7000 ^ 401);
        assert!(nested_id(-5, 3) >= 0);
    }
}
