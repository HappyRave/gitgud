use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::diff::is_binary;
use crate::repo::{Error, Repo, read_err};
use crate::unity::{UNITY_YAML_EXTENSIONS, collect_asset_guids, find_unity_root};
use crate::worktree::Side as WorktreeSide;

const BUILTIN_GUIDS: [&str; 3] = [
    "00000000000000000000000000000000",
    "0000000000000000e000000000000000",
    "0000000000000000f000000000000000",
];

const GITIGNORE_REQUIRED: [(&str, &str); 5] = [
    ("library", "/[Ll]ibrary/"),
    ("temp", "/[Tt]emp/"),
    ("obj", "/[Oo]bj/"),
    ("logs", "/[Ll]ogs/"),
    ("usersettings", "/[Uu]ser[Ss]ettings/"),
];

const GITATTRIBUTES_REQUIRED: [(&str, &str); 4] = [
    ("*.unity", "*.unity text"),
    ("*.prefab", "*.prefab text"),
    ("*.asset", "*.asset text"),
    ("*.png", "*.png filter=lfs diff=lfs merge=lfs -text"),
];

/// Returns `true` when `path` has a Unity YAML extension (`.prefab`, `.unity`, `.asset`, etc.).
#[must_use]
pub fn is_unity_yaml_path(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    UNITY_YAML_EXTENSIONS
        .iter()
        .any(|u| u.eq_ignore_ascii_case(ext))
}

/// Maps Unity `.meta` 32-hex GUIDs to asset paths relative to the Unity project root.
#[derive(Debug, Clone, Default)]
pub struct GuidIndex {
    guid_to_path: HashMap<String, String>,
}

impl GuidIndex {
    /// Builds an in-memory GUID -> asset-path index by scanning `project_root/Assets`.
    #[must_use]
    pub fn build(project_root: &Path) -> Self {
        let mut raw = HashMap::new();
        let assets = project_root.join("Assets");
        if assets.is_dir() {
            collect_asset_guids(project_root, &assets, &mut raw);
        }
        let mut guid_to_path = HashMap::with_capacity(raw.len());
        for (guid, mut paths) in raw {
            paths.sort();
            if let Some(first) = paths.into_iter().next() {
                let asset = first.strip_suffix(".meta").unwrap_or(&first).to_owned();
                guid_to_path.insert(guid, asset);
            }
        }
        Self { guid_to_path }
    }

    /// Resolves a 32-hex Unity asset GUID (case-insensitive) to its project-relative asset path.
    #[must_use]
    pub fn resolve_guid(&self, guid: &str) -> Option<&str> {
        self.guid_to_path
            .get(&guid.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Resolves a 32-hex GUID to its asset file stem or script name (e.g. `PlayerController.cs`).
    #[must_use]
    pub fn script_name(&self, guid: &str) -> Option<String> {
        let asset = self.resolve_guid(guid)?;
        let p = Path::new(asset);
        if p.extension().and_then(|e| e.to_str()) == Some("cs") {
            return p.file_name().and_then(|n| n.to_str()).map(str::to_owned);
        }
        p.file_stem().and_then(|n| n.to_str()).map(str::to_owned)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.guid_to_path.is_empty()
    }
}

/// Single property entry inside a Unity YAML object document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityProp {
    pub value: String,
    pub line_idx: usize,
    pub indent: usize,
    pub key: String,
    pub is_mod_override: bool,
}

/// One YAML object document (`--- !u!<class_id> &<file_id>`).
#[derive(Debug, Clone)]
pub struct UnityDoc {
    pub file_id: String,
    pub class_id: u32,
    pub stripped: bool,
    pub start_line: usize,
    pub end_line: usize,
    pub root_type: String,
    pub props: BTreeMap<String, UnityProp>,
    pub name_hint: Option<String>,
    pub game_object_ref: Option<String>,
    pub father_ref: Option<String>,
    pub transform_parent_ref: Option<String>,
    pub script_guid: Option<String>,
}

/// Parsed multi-document Unity YAML asset or scene.
#[derive(Debug, Clone)]
pub struct UnityScene {
    pub docs: BTreeMap<String, UnityDoc>,
    pub ordered_ids: Vec<String>,
    pub game_object_names: HashMap<String, String>,
    pub transforms: HashMap<String, (Option<String>, Option<String>)>,
    pub go_to_transform: HashMap<String, String>,
    pub raw_lines: Vec<String>,
    pub line_ending: &'static str,
}

impl UnityScene {
    /// Parses multi-document Unity YAML text.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let raw_lines: Vec<String> = text
            .lines()
            .map(|l| l.trim_end_matches('\r').to_owned())
            .collect();

        let mut docs = BTreeMap::new();
        let mut ordered_ids = Vec::new();
        let mut game_object_names = HashMap::new();
        let mut transforms = HashMap::new();
        let mut go_to_transform = HashMap::new();

        let mut cur_idx = 0;
        while cur_idx < raw_lines.len() {
            let line = &raw_lines[cur_idx];
            if let Some((class_id, file_id, stripped)) = parse_doc_header(line) {
                let start_line = cur_idx;
                let mut end_line = start_line + 1;
                while end_line < raw_lines.len() && !raw_lines[end_line].starts_with("--- !u!") {
                    end_line += 1;
                }
                let doc = parse_single_doc(
                    &raw_lines[start_line..end_line],
                    start_line,
                    end_line,
                    class_id,
                    &file_id,
                    stripped,
                );
                if doc.class_id == 1
                    && let Some(name) = &doc.name_hint
                {
                    game_object_names.insert(doc.file_id.clone(), name.clone());
                } else if doc.class_id == 4 || doc.class_id == 224 {
                    if let Some(go) = &doc.game_object_ref {
                        go_to_transform.insert(go.clone(), doc.file_id.clone());
                    }
                    transforms.insert(
                        doc.file_id.clone(),
                        (doc.game_object_ref.clone(), doc.father_ref.clone()),
                    );
                }
                ordered_ids.push(file_id.clone());
                docs.insert(file_id, doc);
                cur_idx = end_line;
            } else {
                cur_idx += 1;
            }
        }

        Self {
            docs,
            ordered_ids,
            game_object_names,
            transforms,
            go_to_transform,
            raw_lines,
            line_ending,
        }
    }

    /// Computes `(object_path, component_name)` for document `&file_id`.
    #[must_use]
    pub fn labels_for(&self, file_id: &str, guids: &GuidIndex) -> (String, String) {
        let Some(doc) = self.docs.get(file_id) else {
            return (format!("&{file_id}"), "Object".into());
        };
        let component = self.component_label(doc, guids);
        let obj_path = if doc.class_id == 1001 {
            self.prefab_instance_path(doc)
        } else if let Some(tid) = self.transform_for_doc(doc) {
            self.walk_transform_path(&tid)
                .or_else(|| doc.name_hint.clone())
                .unwrap_or_else(|| format!("{component} (&{file_id})"))
        } else if let Some(name) = &doc.name_hint {
            name.clone()
        } else {
            format!("{component} (&{file_id})")
        };
        (obj_path, component)
    }

    fn component_label(&self, doc: &UnityDoc, guids: &GuidIndex) -> String {
        if doc.class_id == 114 {
            if let Some(guid) = &doc.script_guid
                && let Some(script) = guids.script_name(guid)
            {
                return script;
            }
            return "MonoBehaviour".into();
        }
        if !doc.root_type.is_empty() {
            return doc.root_type.clone();
        }
        class_name(doc.class_id).to_owned()
    }

    fn prefab_instance_path(&self, doc: &UnityDoc) -> String {
        let overridden_name = doc
            .props
            .get("override:m_Name")
            .map(|p| p.value.as_str())
            .filter(|s| !s.is_empty());
        let parent_path = doc
            .transform_parent_ref
            .as_deref()
            .and_then(|tid| self.walk_transform_path(tid));
        match (parent_path, overridden_name) {
            (Some(p), Some(n)) => format!("{p} / {n}"),
            (Some(p), None) => format!("{p} / PrefabInstance"),
            (None, Some(n)) => n.to_owned(),
            (None, None) => format!("PrefabInstance (&{})", doc.file_id),
        }
    }

    fn transform_for_doc(&self, doc: &UnityDoc) -> Option<String> {
        if doc.class_id == 4 || doc.class_id == 224 {
            return Some(doc.file_id.clone());
        }
        if doc.class_id == 1 {
            return self.go_to_transform.get(&doc.file_id).cloned();
        }
        doc.game_object_ref
            .as_deref()
            .and_then(|go| self.go_to_transform.get(go))
            .cloned()
    }

    fn walk_transform_path(&self, start_transform: &str) -> Option<String> {
        let mut parts = Vec::new();
        let mut visited = HashSet::new();
        let mut cur = Some(start_transform.to_owned());
        for _ in 0..64 {
            let Some(tid) = cur else {
                break;
            };
            if !visited.insert(tid.clone()) {
                break;
            }
            let Some((go_ref, father_ref)) = self.transforms.get(&tid) else {
                break;
            };
            let name = go_ref
                .as_deref()
                .and_then(|go| self.game_object_names.get(go))
                .cloned()
                .unwrap_or_else(|| format!("&{tid}"));
            parts.push(name);
            cur = father_ref.clone();
        }
        if parts.is_empty() {
            None
        } else {
            parts.reverse();
            Some(parts.join(" / "))
        }
    }
}

/// Property-level difference on a single Unity object or component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityPropChange {
    pub property: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub before_display: Option<String>,
    pub after_display: Option<String>,
}

/// Object-level structural difference inside a Unity YAML file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityObjectChange {
    pub file_id: String,
    pub object_path: String,
    pub component: String,
    pub kind: String,
    pub properties: Vec<UnityPropChange>,
}

/// Structural diff summary for a `.prefab`, `.unity`, or `.asset` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityYamlDiff {
    pub path: String,
    pub added_objects: usize,
    pub removed_objects: usize,
    pub modified_properties: usize,
    pub objects: Vec<UnityObjectChange>,
}

/// Conflicted property row presented by the R4-A property table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityConflictRow {
    pub id: String,
    pub file_id: String,
    pub object_path: String,
    pub component: String,
    pub property: String,
    pub base: Option<String>,
    pub yours: Option<String>,
    pub theirs: Option<String>,
    pub base_display: Option<String>,
    pub yours_display: Option<String>,
    pub theirs_display: Option<String>,
}

/// Auto-merged single-side change shown collapsed under R4-A conflicts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityAutoRow {
    pub file_id: String,
    pub object_path: String,
    pub component: String,
    pub property: String,
    pub side: String,
    pub value: Option<String>,
    pub display: Option<String>,
}

/// Three-way property table for resolving a conflicted Unity YAML file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityConflictTable {
    pub path: String,
    pub conflicts: Vec<UnityConflictRow>,
    pub auto_merged: Vec<UnityAutoRow>,
}

/// Caller choice for one conflicted property in [`UnityConflictTable`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityPropertyPick {
    pub file_id: String,
    pub property: String,
    pub value: Option<String>,
}

/// Reference or duplicate-ID hazard found by post-merge integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityIntegrityIssue {
    pub path: String,
    pub kind: String,
    pub file_id: Option<String>,
    pub detail: String,
}

/// Unity repository hygiene summary (`.gitignore`, `.gitattributes`, `Force Text`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityRepoSetupStatus {
    pub is_unity: bool,
    pub has_gitignore: bool,
    pub gitignore_missing: Vec<String>,
    pub has_gitattributes: bool,
    pub gitattributes_missing: Vec<String>,
    pub force_text_ok: bool,
    pub serialization_mode: Option<i32>,
}

/// Formats a raw YAML property value into a human-friendly display string when it holds asset GUIDs or internal fileIDs.
#[must_use]
pub fn format_value_display(raw: &str, scene: &UnityScene, guids: &GuidIndex) -> String {
    let trimmed = raw.trim();
    if let Some((fid, guid)) = parse_inline_ref(trimmed) {
        if let Some(g) = guid {
            if let Some(asset) = guids.resolve_guid(&g) {
                return asset.to_owned();
            }
            return trimmed.to_owned();
        }
        if fid == "0" {
            return "None".to_owned();
        }
        if let Some(target_doc) = scene.docs.get(&fid) {
            let (obj_path, comp) = scene.labels_for(&fid, guids);
            if target_doc.class_id == 1 {
                return obj_path;
            }
            return format!("{obj_path} ({comp})");
        }
    }
    trimmed.to_owned()
}

/// Computes a structural diff between optional `before` and `after` Unity YAML texts.
#[must_use]
pub fn diff_unity_yaml(
    path: &str,
    before: Option<&str>,
    after: Option<&str>,
    guids: &GuidIndex,
) -> UnityYamlDiff {
    let b_scene = before
        .filter(|s| s.contains("--- !u!"))
        .map(UnityScene::parse);
    let a_scene = after
        .filter(|s| s.contains("--- !u!"))
        .map(UnityScene::parse);

    let mut ids: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    if let Some(s) = &a_scene {
        for id in &s.ordered_ids {
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
    }
    if let Some(s) = &b_scene {
        for id in &s.ordered_ids {
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
    }

    let empty_scene = UnityScene::parse("");
    let b_ref = b_scene.as_ref().unwrap_or(&empty_scene);
    let a_ref = a_scene.as_ref().unwrap_or(&empty_scene);

    let mut added_objects = 0usize;
    let mut removed_objects = 0usize;
    let mut modified_properties = 0usize;
    let mut objects = Vec::new();

    for file_id in ids {
        let bd = b_ref.docs.get(&file_id);
        let ad = a_ref.docs.get(&file_id);
        let (kind, scene_for_labels) = match (bd, ad) {
            (None, Some(_)) => {
                added_objects += 1;
                ("added", a_ref)
            }
            (Some(_), None) => {
                removed_objects += 1;
                ("removed", b_ref)
            }
            (Some(_), Some(_)) => ("modified", a_ref),
            (None, None) => continue,
        };
        let (object_path, component) = scene_for_labels.labels_for(&file_id, guids);

        let mut all_props = BTreeSet::new();
        if let Some(d) = bd {
            all_props.extend(d.props.keys().cloned());
        }
        if let Some(d) = ad {
            all_props.extend(d.props.keys().cloned());
        }

        let mut changes = Vec::new();
        for prop in all_props {
            let bv = bd.and_then(|d| d.props.get(&prop)).map(|p| p.value.clone());
            let av = ad.and_then(|d| d.props.get(&prop)).map(|p| p.value.clone());
            if bv == av {
                continue;
            }
            let before_display = bv.as_deref().map(|v| format_value_display(v, b_ref, guids));
            let after_display = av.as_deref().map(|v| format_value_display(v, a_ref, guids));
            changes.push(UnityPropChange {
                property: prop,
                before: bv,
                after: av,
                before_display,
                after_display,
            });
        }

        if kind == "modified" && changes.is_empty() {
            continue;
        }
        modified_properties += changes.len();
        objects.push(UnityObjectChange {
            file_id,
            object_path,
            component,
            kind: kind.to_owned(),
            properties: changes,
        });
    }

    UnityYamlDiff {
        path: path.to_owned(),
        added_objects,
        removed_objects,
        modified_properties,
        objects,
    }
}

// LFS pointers become the locally cached object; never downloads (a missing one is Refused).
fn resolve_lfs(repo: &Repo, path: &str, bytes: Option<Vec<u8>>) -> Result<Option<Vec<u8>>, Error> {
    bytes
        .map(|b| repo.lfs_resolve_with_fetch(None, path, b, 0))
        .transpose()
}

/// Builds the structural diff for a working-copy Unity YAML file (staged or unstaged).
///
/// # Errors
/// [`Error::Refused`] if the file is binary or lacks Unity YAML headers.
pub fn worktree_unity_diff(
    repo: &Repo,
    path: &str,
    side: WorktreeSide,
) -> Result<UnityYamlDiff, Error> {
    if !is_unity_yaml_path(path) {
        return Err(Error::Refused(format!("{path} is not a Unity YAML file")));
    }
    let versions = repo.versions(path, side)?;
    let old = resolve_lfs(repo, path, versions.old)?;
    let new = resolve_lfs(repo, path, versions.new)?;
    let old_bytes = old.as_deref().unwrap_or(&[]);
    let new_bytes = new.as_deref().unwrap_or(&[]);
    if is_binary(old_bytes) || is_binary(new_bytes) {
        return Err(Error::Refused(format!("{path} is binary")));
    }
    let old_s = old.map(|b| String::from_utf8_lossy(&b).into_owned());
    let new_s = new.map(|b| String::from_utf8_lossy(&b).into_owned());
    if !old_s.as_deref().is_some_and(|s| s.contains("--- !u!"))
        && !new_s.as_deref().is_some_and(|s| s.contains("--- !u!"))
    {
        return Err(Error::Refused(format!(
            "{path} does not contain Unity YAML documents"
        )));
    }
    let guids = repo_guid_index(repo);
    Ok(diff_unity_yaml(
        path,
        old_s.as_deref(),
        new_s.as_deref(),
        &guids,
    ))
}

/// Builds the structural diff for a Unity YAML file inside a historical commit.
///
/// # Errors
/// [`Error::Read`] on git read failure or [`Error::Refused`] if binary/non-Unity YAML.
pub fn commit_unity_diff(repo: &Repo, id: &str, path: &str) -> Result<UnityYamlDiff, Error> {
    if !is_unity_yaml_path(path) {
        return Err(Error::Refused(format!("{path} is not a Unity YAML file")));
    }
    let v = repo.commit_file_versions(id, path)?;
    let old = resolve_lfs(repo, path, v.old)?;
    let new = resolve_lfs(repo, path, v.new)?;
    let old_bytes = old.as_deref().unwrap_or(&[]);
    let new_bytes = new.as_deref().unwrap_or(&[]);
    if is_binary(old_bytes) || is_binary(new_bytes) {
        return Err(Error::Refused(format!("{path} is binary")));
    }
    let old_s = old.map(|b| String::from_utf8_lossy(&b).into_owned());
    let new_s = new.map(|b| String::from_utf8_lossy(&b).into_owned());
    if !old_s.as_deref().is_some_and(|s| s.contains("--- !u!"))
        && !new_s.as_deref().is_some_and(|s| s.contains("--- !u!"))
    {
        return Err(Error::Refused(format!(
            "{path} does not contain Unity YAML documents"
        )));
    }
    let guids = repo_guid_index(repo);
    Ok(diff_unity_yaml(
        path,
        old_s.as_deref(),
        new_s.as_deref(),
        &guids,
    ))
}

/// Builds a 3-way R4-A property conflict table from `base`, `ours`, and `theirs` YAML strings.
///
/// # Errors
/// [`Error::Refused`] if none of the inputs contain `--- !u!` Unity YAML documents.
pub fn build_conflict_table(
    path: &str,
    base: &str,
    ours: &str,
    theirs: &str,
    guids: &GuidIndex,
) -> Result<UnityConflictTable, Error> {
    if !base.contains("--- !u!") && !ours.contains("--- !u!") && !theirs.contains("--- !u!") {
        return Err(Error::Refused(format!(
            "{path} is not valid multi-document Unity YAML"
        )));
    }
    let b_scene = UnityScene::parse(base);
    let o_scene = UnityScene::parse(ours);
    let t_scene = UnityScene::parse(theirs);

    let mut all_ids = Vec::new();
    let mut seen_ids = HashSet::new();
    for id in o_scene
        .ordered_ids
        .iter()
        .chain(&t_scene.ordered_ids)
        .chain(&b_scene.ordered_ids)
    {
        if seen_ids.insert(id.clone()) {
            all_ids.push(id.clone());
        }
    }

    let mut conflicts = Vec::new();
    let mut auto_merged = Vec::new();

    for file_id in all_ids {
        let bd = b_scene.docs.get(&file_id);
        let od = o_scene.docs.get(&file_id);
        let td = t_scene.docs.get(&file_id);

        let (object_path, component) = if o_scene.docs.contains_key(&file_id) {
            o_scene.labels_for(&file_id, guids)
        } else if t_scene.docs.contains_key(&file_id) {
            t_scene.labels_for(&file_id, guids)
        } else {
            b_scene.labels_for(&file_id, guids)
        };

        let mut props = BTreeSet::new();
        for d in [bd, od, td].into_iter().flatten() {
            props.extend(d.props.keys().cloned());
        }

        for prop in props {
            let bv = bd.and_then(|d| d.props.get(&prop)).map(|p| p.value.clone());
            let ov = od.and_then(|d| d.props.get(&prop)).map(|p| p.value.clone());
            let tv = td.and_then(|d| d.props.get(&prop)).map(|p| p.value.clone());

            if ov == tv {
                continue;
            }
            if tv == bv && ov != bv {
                auto_merged.push(UnityAutoRow {
                    file_id: file_id.clone(),
                    object_path: object_path.clone(),
                    component: component.clone(),
                    property: prop,
                    side: "yours".into(),
                    display: ov
                        .as_deref()
                        .map(|v| format_value_display(v, &o_scene, guids)),
                    value: ov,
                });
            } else if ov == bv && tv != bv {
                auto_merged.push(UnityAutoRow {
                    file_id: file_id.clone(),
                    object_path: object_path.clone(),
                    component: component.clone(),
                    property: prop,
                    side: "theirs".into(),
                    display: tv
                        .as_deref()
                        .map(|v| format_value_display(v, &t_scene, guids)),
                    value: tv,
                });
            } else {
                conflicts.push(UnityConflictRow {
                    id: format!("{file_id}::{prop}"),
                    file_id: file_id.clone(),
                    object_path: object_path.clone(),
                    component: component.clone(),
                    property: prop,
                    base_display: bv
                        .as_deref()
                        .map(|v| format_value_display(v, &b_scene, guids)),
                    yours_display: ov
                        .as_deref()
                        .map(|v| format_value_display(v, &o_scene, guids)),
                    theirs_display: tv
                        .as_deref()
                        .map(|v| format_value_display(v, &t_scene, guids)),
                    base: bv,
                    yours: ov,
                    theirs: tv,
                });
            }
        }
    }

    Ok(UnityConflictTable {
        path: path.to_owned(),
        conflicts,
        auto_merged,
    })
}

/// Applies one-sided `"theirs"` auto-merges plus caller property `picks` onto `ours` YAML, returning resolved YAML.
///
/// # Errors
/// [`Error::Refused`] when `ours`/`theirs` cannot be parsed as Unity YAML.
pub fn apply_unity_conflict_picks(
    base: &str,
    ours: &str,
    theirs: &str,
    picks: &[UnityPropertyPick],
) -> Result<String, Error> {
    if !ours.contains("--- !u!") && !theirs.contains("--- !u!") {
        return Err(Error::Refused(
            "conflict does not contain Unity YAML".into(),
        ));
    }
    let guids = GuidIndex::default();
    let table = build_conflict_table("", base, ours, theirs, &guids)?;

    // Gather desired property mutations keyed by (file_id, property) -> Option<String>.
    let mut desired: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();
    for auto in &table.auto_merged {
        if auto.side == "theirs" {
            desired
                .entry(auto.file_id.clone())
                .or_default()
                .push((auto.property.clone(), auto.value.clone()));
        }
    }
    for pick in picks {
        let list = desired.entry(pick.file_id.clone()).or_default();
        if let Some(existing) = list.iter_mut().find(|(p, _)| *p == pick.property) {
            existing.1 = pick.value.clone();
        } else {
            list.push((pick.property.clone(), pick.value.clone()));
        }
    }

    let mut lines: Vec<String> = ours
        .lines()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect();
    let eol = if ours.contains("\r\n") || theirs.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let trailing_newline = ours.ends_with('\n') || theirs.ends_with('\n');
    let t_scene = UnityScene::parse(theirs);

    // Apply edits document by document from bottom to top or re-parse offsets dynamically.
    for (file_id, updates) in desired {
        let cur_text = lines.join("\n");
        let cur_scene = UnityScene::parse(&cur_text);

        if let Some(doc) = cur_scene.docs.get(&file_id) {
            // Apply updates to existing `ours` document cleanly.
            let mut deletions: BTreeSet<usize> = BTreeSet::new();
            let mut insertions: Vec<String> = Vec::new();

            for (prop, new_val) in updates {
                if let Some(existing) = doc.props.get(&prop) {
                    match new_val {
                        Some(val) => {
                            let old_line = &lines[existing.line_idx];
                            let indent: String =
                                old_line.chars().take_while(|c| c.is_whitespace()).collect();
                            let prefix = if existing.key.starts_with('[') {
                                format!("{indent}-")
                            } else if old_line.trim_start().starts_with("- ")
                                && !existing.is_mod_override
                            {
                                format!("{indent}- {}:", existing.key)
                            } else {
                                format!("{indent}{}:", existing.key)
                            };
                            lines[existing.line_idx] = if val.is_empty() {
                                prefix
                            } else {
                                format!("{prefix} {val}")
                            };
                        }
                        None => {
                            deletions.insert(existing.line_idx);
                        }
                    }
                } else if let Some(val) = new_val {
                    if let Some(prop_path) = prop.strip_prefix("override:") {
                        insertions.extend([
                            "    - target: {fileID: 0}".into(),
                            format!("      propertyPath: {prop_path}"),
                            if val.is_empty() {
                                "      value:".into()
                            } else {
                                format!("      value: {val}")
                            },
                            "      objectReference: {fileID: 0}".into(),
                        ]);
                    } else {
                        let leaf = prop.rsplit('.').next().unwrap_or(&prop);
                        let indent = "  ".repeat(prop.split('.').count().max(1));
                        insertions.push(if val.is_empty() {
                            format!("{indent}{leaf}:")
                        } else {
                            format!("{indent}{leaf}: {val}")
                        });
                    }
                }
            }

            if !insertions.is_empty() {
                let insert_at = doc.end_line;
                lines.splice(insert_at..insert_at, insertions);
            }
            if !deletions.is_empty() {
                for idx in deletions.into_iter().rev() {
                    if idx < lines.len() {
                        lines.remove(idx);
                    }
                }
            }
        } else if let Some(tdoc) = t_scene.docs.get(&file_id) {
            // Document only exists in `theirs`: bring it over when any update wants a value present.
            if updates.iter().any(|(_, v)| v.is_some()) {
                let mut cloned: Vec<String> =
                    t_scene.raw_lines[tdoc.start_line..tdoc.end_line].to_vec();
                let mut parsed_clone = UnityScene::parse(&cloned.join("\n"));
                if let Some(cdoc) = parsed_clone.docs.get_mut(&file_id) {
                    for (prop, val) in updates {
                        if let Some(existing) = cdoc.props.get(&prop)
                            && let Some(v) = val
                        {
                            let old_line = &cloned[existing.line_idx];
                            let indent: String =
                                old_line.chars().take_while(|c| c.is_whitespace()).collect();
                            cloned[existing.line_idx] = if v.is_empty() {
                                format!("{indent}{}:", existing.key)
                            } else {
                                format!("{indent}{}: {v}", existing.key)
                            };
                        }
                    }
                }
                lines.extend(cloned);
            }
        }
    }

    let mut out = lines.join(eol);
    if trailing_newline && !out.is_empty() {
        out.push_str(eol);
    }
    Ok(out)
}

/// Checks a resolved Unity YAML file for duplicate `&fileId` headers, missing internal refs, and unknown external GUIDs.
#[must_use]
pub fn check_file_integrity(
    path: &str,
    content: &str,
    guids: &GuidIndex,
) -> Vec<UnityIntegrityIssue> {
    if !content.contains("--- !u!") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let mut defined_counts: BTreeMap<String, usize> = BTreeMap::new();
    for raw in content.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some((_, fid, _)) = parse_doc_header(line) {
            *defined_counts.entry(fid).or_default() += 1;
        }
    }
    for (fid, count) in &defined_counts {
        if *count > 1 {
            issues.push(UnityIntegrityIssue {
                path: path.to_owned(),
                kind: "duplicateFileId".into(),
                file_id: Some(fid.clone()),
                detail: format!("Duplicate fileID &{fid} ({count} occurrences) in {path}"),
            });
        }
    }

    let mut reported_missing_fid = HashSet::new();
    let mut reported_missing_guid = HashSet::new();

    for raw in content.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with("--- !u!") || line.trim_start().starts_with('#') {
            continue;
        }
        let mut cursor = line;
        while let Some(open) = cursor.find('{') {
            let after = &cursor[open..];
            let Some(close) = after.find('}') else {
                break;
            };
            let snippet = &after[..=close];
            if let Some((fid, guid)) = parse_inline_ref(snippet) {
                if let Some(g) = guid {
                    let gl = g.to_ascii_lowercase();
                    if !guids.is_empty()
                        && !BUILTIN_GUIDS.contains(&gl.as_str())
                        && guids.resolve_guid(&gl).is_none()
                        && reported_missing_guid.insert(gl.clone())
                    {
                        issues.push(UnityIntegrityIssue {
                            path: path.to_owned(),
                            kind: "missingGuid".into(),
                            file_id: Some(fid),
                            detail: format!(
                                "Missing external asset GUID {gl} referenced in {path}"
                            ),
                        });
                    }
                } else if fid != "0"
                    && !defined_counts.contains_key(&fid)
                    && reported_missing_fid.insert(fid.clone())
                {
                    issues.push(UnityIntegrityIssue {
                        path: path.to_owned(),
                        kind: "missingFileId".into(),
                        file_id: Some(fid.clone()),
                        detail: format!("Missing internal reference {{fileID: {fid}}} in {path}"),
                    });
                }
            }
            cursor = &after[close + 1..];
        }
    }

    issues
}

/// Runs post-merge Unity YAML integrity verification over all resolved Unity files in `repo`.
#[must_use]
pub fn check_repo_unity_integrity(repo: &Repo, paths: &[String]) -> Vec<UnityIntegrityIssue> {
    let guids = repo_guid_index(repo);
    let mut out = Vec::new();
    for path in paths {
        if !is_unity_yaml_path(path) {
            continue;
        }
        let full = repo.root().join(path);
        let Ok(content) = fs::read_to_string(&full) else {
            continue;
        };
        out.extend(check_file_integrity(path, &content, &guids));
    }
    out
}

/// Checks `.gitignore`, `.gitattributes`, and `EditorSettings.asset` serialization mode for a Unity repo.
///
/// # Errors
/// [`Error::Read`] if existing project config files fail unexpectedly on I/O.
pub fn unity_repo_setup_status(repo: &Repo) -> Result<UnityRepoSetupStatus, Error> {
    let root = repo.root();
    let Some(project) = find_unity_root(root) else {
        return Ok(UnityRepoSetupStatus {
            is_unity: false,
            has_gitignore: false,
            gitignore_missing: Vec::new(),
            has_gitattributes: false,
            gitattributes_missing: Vec::new(),
            force_text_ok: false,
            serialization_mode: None,
        });
    };

    let gitignore_path = root.join(".gitignore");
    let has_gitignore = gitignore_path.is_file();
    let gitignore_body = fs::read_to_string(&gitignore_path)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let gitignore_missing = GITIGNORE_REQUIRED
        .iter()
        .filter(|(needle, _)| !gitignore_has_folder(&gitignore_body, needle))
        .map(|(_, line)| (*line).to_owned())
        .collect();

    let gitattrs_path = root.join(".gitattributes");
    let has_gitattributes = gitattrs_path.is_file();
    let gitattrs_body = fs::read_to_string(&gitattrs_path)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let gitattributes_missing = GITATTRIBUTES_REQUIRED
        .iter()
        .filter(|(prefix, _)| {
            !gitattrs_body
                .lines()
                .map(str::trim)
                .any(|l| !l.starts_with('#') && l.starts_with(prefix))
        })
        .map(|(_, line)| (*line).to_owned())
        .collect();

    let editor_settings = project.join("ProjectSettings/EditorSettings.asset");
    let serialization_mode = fs::read_to_string(&editor_settings)
        .ok()
        .and_then(|s| parse_serialization_mode(&s));
    let force_text_ok = serialization_mode == Some(2);

    Ok(UnityRepoSetupStatus {
        is_unity: true,
        has_gitignore,
        gitignore_missing,
        has_gitattributes,
        gitattributes_missing,
        force_text_ok,
        serialization_mode,
    })
}

/// Appends missing Unity `.gitignore` and/or `.gitattributes` entries without touching existing lines.
///
/// # Errors
/// [`Error::Read`] on filesystem write error.
pub fn apply_unity_repo_setup(
    repo: &Repo,
    write_gitignore: bool,
    write_gitattributes: bool,
) -> Result<UnityRepoSetupStatus, Error> {
    let status = unity_repo_setup_status(repo)?;
    if !status.is_unity {
        return Ok(status);
    }
    let root = repo.root();
    if write_gitignore && !status.gitignore_missing.is_empty() {
        append_missing_lines(&root.join(".gitignore"), &status.gitignore_missing)?;
    }
    if write_gitattributes && !status.gitattributes_missing.is_empty() {
        append_missing_lines(&root.join(".gitattributes"), &status.gitattributes_missing)?;
    }
    unity_repo_setup_status(repo)
}

fn append_missing_lines(path: &Path, additions: &[String]) -> Result<(), Error> {
    let mut existing = fs::read_to_string(path).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    for line in additions {
        existing.push_str(line);
        existing.push('\n');
    }
    fs::write(path, existing).map_err(read_err)
}

fn gitignore_has_folder(lower_body: &str, folder_lower: &str) -> bool {
    let bracketed = match folder_lower {
        "library" => "[ll]ibrary",
        "temp" => "[tt]emp",
        "obj" => "[oo]bj",
        "logs" => "[ll]ogs",
        "usersettings" => "[uu]ser[ss]ettings",
        _ => folder_lower,
    };
    lower_body.lines().any(|raw| {
        let l = raw.trim().trim_start_matches('/').trim_end_matches('/');
        !l.starts_with('#') && (l == folder_lower || l == bracketed)
    })
}

fn parse_serialization_mode(text: &str) -> Option<i32> {
    for raw in text.lines() {
        let l = raw.trim();
        if let Some(rest) = l.strip_prefix("m_SerializationMode:") {
            return rest.trim().parse::<i32>().ok();
        }
    }
    None
}

/// The repo's GUID index, scanned once and kept until [`forget_guid_index`].
pub(crate) fn repo_guid_index(repo: &Repo) -> Arc<GuidIndex> {
    let mut cached = repo.guid_index.lock().expect("guid index lock poisoned");
    cached
        .get_or_insert_with(|| {
            Arc::new(
                find_unity_root(repo.root())
                    .map_or_else(GuidIndex::default, |p| GuidIndex::build(&p)),
            )
        })
        .clone()
}

/// Drops the cached GUID index, so the next structural diff rescans `.meta` files.
pub(crate) fn forget_guid_index(repo: &Repo) {
    *repo.guid_index.lock().expect("guid index lock poisoned") = None;
}

pub(crate) fn parse_doc_header(line: &str) -> Option<(u32, String, bool)> {
    let rest = line.strip_prefix("--- !u!")?;
    let (cls_s, after_cls) = rest.split_once('&')?;
    let class_id = cls_s.trim().parse::<u32>().ok()?;
    let mut parts = after_cls.split_whitespace();
    let file_id = parts.next()?.to_owned();
    let stripped = parts.any(|p| p == "stripped");
    Some((class_id, file_id, stripped))
}

fn parse_single_doc(
    lines: &[String],
    start_line: usize,
    end_line: usize,
    class_id: u32,
    file_id: &str,
    stripped: bool,
) -> UnityDoc {
    let mut root_type = String::new();
    let mut props: BTreeMap<String, UnityProp> = BTreeMap::new();
    let mut name_hint: Option<String> = None;
    let mut game_object_ref: Option<String> = None;
    let mut father_ref: Option<String> = None;
    let mut transform_parent_ref: Option<String> = None;
    let mut script_guid: Option<String> = None;

    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut seq_counters: HashMap<String, usize> = HashMap::new();

    // Active PrefabInstance `m_Modifications` list entry state.
    let mut active_mod_indent: Option<usize> = None;
    let mut mod_path: Option<String> = None;
    let mut mod_val: Option<(String, usize)> = None;
    let mut mod_obj_ref: Option<(String, usize)> = None;

    let flush_mod = |props: &mut BTreeMap<String, UnityProp>,
                     m_path: &mut Option<String>,
                     m_val: &mut Option<(String, usize)>,
                     m_ref: &mut Option<(String, usize)>| {
        let Some(p_path) = m_path.take() else {
            *m_val = None;
            *m_ref = None;
            return;
        };
        let obj = m_ref.take();
        let val = m_val.take();
        let (final_val, line_idx, key_name) = match (val, obj) {
            (Some((v, vline)), Some((oref, oline))) if !is_zero_ref(&oref) => {
                if v.is_empty() {
                    (oref, oline, "objectReference")
                } else {
                    (format!("{v} / {oref}"), vline, "value")
                }
            }
            (Some((v, vline)), _) => (v, vline, "value"),
            (None, Some((oref, oline))) => (oref, oline, "objectReference"),
            (None, None) => return,
        };
        props.insert(
            format!("override:{p_path}"),
            UnityProp {
                value: final_val,
                line_idx,
                indent: 6,
                key: key_name.to_owned(),
                is_mod_override: true,
            },
        );
    };

    for (offset, raw) in lines.iter().enumerate().skip(1) {
        let line_idx = start_line + offset;
        let line = raw.as_str();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 && trimmed.ends_with(':') && !trimmed.contains(' ') {
            flush_mod(&mut props, &mut mod_path, &mut mod_val, &mut mod_obj_ref);
            active_mod_indent = None;
            root_type = trimmed.trim_end_matches(':').to_owned();
            stack.clear();
            continue;
        }

        if let Some(mod_ind) = active_mod_indent {
            if indent < mod_ind || (indent == mod_ind && !trimmed.starts_with('-')) {
                flush_mod(&mut props, &mut mod_path, &mut mod_val, &mut mod_obj_ref);
                active_mod_indent = None;
            } else if trimmed.starts_with("- ") {
                flush_mod(&mut props, &mut mod_path, &mut mod_val, &mut mod_obj_ref);
                let after_dash = trimmed.trim_start_matches("- ").trim();
                if let Some((k, v)) = after_dash.split_once(':') {
                    match k.trim() {
                        "propertyPath" => mod_path = Some(v.trim().to_owned()),
                        "value" => mod_val = Some((v.trim().to_owned(), line_idx)),
                        "objectReference" => mod_obj_ref = Some((v.trim().to_owned(), line_idx)),
                        _ => {}
                    }
                }
                continue;
            } else if let Some((k, v)) = trimmed.split_once(':') {
                match k.trim() {
                    "propertyPath" => mod_path = Some(v.trim().to_owned()),
                    "value" => mod_val = Some((v.trim().to_owned(), line_idx)),
                    "objectReference" => mod_obj_ref = Some((v.trim().to_owned(), line_idx)),
                    _ => {}
                }
                continue;
            }
        }

        let is_seq_item = trimmed.starts_with("- ");
        while stack.last().is_some_and(|(ind, _)| {
            if is_seq_item {
                *ind > indent
            } else {
                *ind >= indent
            }
        }) {
            stack.pop();
        }
        let body = if is_seq_item {
            trimmed.trim_start_matches("- ").trim()
        } else {
            trimmed
        };

        if class_id == 1001 && stack.iter().any(|(_, seg)| seg == "m_Modifications") && is_seq_item
        {
            active_mod_indent = Some(indent);
            if let Some((k, v)) = body.split_once(':') {
                match k.trim() {
                    "propertyPath" => mod_path = Some(v.trim().to_owned()),
                    "value" => mod_val = Some((v.trim().to_owned(), line_idx)),
                    "objectReference" => mod_obj_ref = Some((v.trim().to_owned(), line_idx)),
                    _ => {}
                }
            }
            continue;
        }

        if is_seq_item
            && !body.is_empty()
            && (body.starts_with('{') || body.starts_with('[') || !body.contains(':'))
        {
            let parent_prefix = stack
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let c = seq_counters.entry(parent_prefix.clone()).or_insert(0);
            let idx = *c;
            *c += 1;
            let full_key = if parent_prefix.is_empty() {
                format!("[{idx}]")
            } else {
                format!("{parent_prefix}[{idx}]")
            };
            props.insert(
                full_key,
                UnityProp {
                    value: body.to_owned(),
                    line_idx,
                    indent,
                    key: format!("[{idx}]"),
                    is_mod_override: false,
                },
            );
            continue;
        }

        if let Some((k, v)) = body.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            let parent_prefix = stack
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let seq_tag = if is_seq_item {
                let c = seq_counters.entry(parent_prefix.clone()).or_insert(0);
                let idx = *c;
                *c += 1;
                Some(idx)
            } else {
                None
            };

            if val.is_empty() {
                let seg = match seq_tag {
                    Some(idx) => format!("{key}[{idx}]"),
                    None => key.to_owned(),
                };
                stack.push((indent, seg));
            } else {
                let full_key = match (parent_prefix.is_empty(), seq_tag) {
                    (true, None) => key.to_owned(),
                    (true, Some(idx)) => format!("{key}[{idx}]"),
                    (false, None) => format!("{parent_prefix}.{key}"),
                    (false, Some(idx)) => format!("{parent_prefix}[{idx}].{key}"),
                };

                if key == "m_Name" && name_hint.is_none() {
                    name_hint = Some(val.to_owned());
                } else if full_key == "m_GameObject"
                    && let Some((fid, None)) = parse_inline_ref(val)
                    && fid != "0"
                {
                    game_object_ref = Some(fid);
                } else if full_key == "m_Father"
                    && let Some((fid, None)) = parse_inline_ref(val)
                    && fid != "0"
                {
                    father_ref = Some(fid);
                } else if full_key == "m_Modification.m_TransformParent"
                    && let Some((fid, None)) = parse_inline_ref(val)
                    && fid != "0"
                {
                    transform_parent_ref = Some(fid);
                } else if full_key == "m_Script"
                    && let Some((_, Some(g))) = parse_inline_ref(val)
                {
                    script_guid = Some(g);
                }

                props.insert(
                    full_key,
                    UnityProp {
                        value: val.to_owned(),
                        line_idx,
                        indent,
                        key: key.to_owned(),
                        is_mod_override: false,
                    },
                );
            }
        }
    }
    flush_mod(&mut props, &mut mod_path, &mut mod_val, &mut mod_obj_ref);

    UnityDoc {
        file_id: file_id.to_owned(),
        class_id,
        stripped,
        start_line,
        end_line,
        root_type,
        props,
        name_hint,
        game_object_ref,
        father_ref,
        transform_parent_ref,
        script_guid,
    }
}

fn is_zero_ref(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed == "{fileID: 0}" || trimmed == "{fileID:0}"
}

fn parse_inline_ref(raw: &str) -> Option<(String, Option<String>)> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut fid: Option<String> = None;
    let mut guid: Option<String> = None;
    for part in inner.split(',') {
        let Some((k, v)) = part.split_once(':') else {
            continue;
        };
        match k.trim() {
            "fileID" => fid = Some(v.trim().to_owned()),
            "guid" => guid = Some(v.trim().to_ascii_lowercase()),
            _ => {}
        }
    }
    fid.map(|id| (id, guid))
}

fn class_name(id: u32) -> &'static str {
    match id {
        1 => "GameObject",
        4 => "Transform",
        20 => "Camera",
        21 => "Material",
        23 => "MeshRenderer",
        33 => "MeshFilter",
        54 => "Rigidbody",
        64 => "MeshCollider",
        65 => "BoxCollider",
        82 => "AudioSource",
        95 => "Animator",
        108 => "Light",
        114 => "MonoBehaviour",
        135 => "SphereCollider",
        136 => "CapsuleCollider",
        212 => "SpriteRenderer",
        222 => "CanvasRenderer",
        223 => "Canvas",
        224 => "RectTransform",
        225 => "CanvasGroup",
        1001 => "PrefabInstance",
        _ => "Object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scene(player_speed: &str, guid: &str) -> String {
        format!(
            "%YAML 1.1\n\
--- !u!1 &10\n\
GameObject:\n\
  m_Name: Root\n\
--- !u!4 &11\n\
Transform:\n\
  m_GameObject: {{fileID: 10}}\n\
  m_Father: {{fileID: 0}}\n\
--- !u!1 &20\n\
GameObject:\n\
  m_Name: Player\n\
--- !u!4 &21\n\
Transform:\n\
  m_GameObject: {{fileID: 20}}\n\
  m_Father: {{fileID: 11}}\n\
  m_LocalPosition: {{x: 0, y: 1, z: 0}}\n\
--- !u!114 &22\n\
MonoBehaviour:\n\
  m_GameObject: {{fileID: 20}}\n\
  m_Script: {{fileID: 11500000, guid: {guid}, type: 3}}\n\
  moveSpeed: {player_speed}\n"
        )
    }

    #[test]
    fn resolves_hierarchy_and_script_names_in_structural_diff() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("Assets/Scripts");
        fs::create_dir_all(&assets).unwrap();
        let script_guid = "abcdef0123456789abcdef0123456789";
        fs::write(assets.join("PlayerMover.cs"), "class PlayerMover {}").unwrap();
        fs::write(
            assets.join("PlayerMover.cs.meta"),
            format!("fileFormatVersion: 2\nguid: {script_guid}\n"),
        )
        .unwrap();

        let guids = GuidIndex::build(dir.path());
        let before = sample_scene("5", script_guid);
        let after = sample_scene("12", script_guid);

        let diff = diff_unity_yaml(
            "Assets/Scenes/Main.unity",
            Some(&before),
            Some(&after),
            &guids,
        );
        assert_eq!(diff.added_objects, 0);
        assert_eq!(diff.removed_objects, 0);
        assert_eq!(diff.modified_properties, 1);
        assert_eq!(diff.objects.len(), 1);
        assert_eq!(diff.objects[0].object_path, "Root / Player");
        assert_eq!(diff.objects[0].component, "PlayerMover.cs");
        assert_eq!(diff.objects[0].properties[0].property, "moveSpeed");
        assert_eq!(diff.objects[0].properties[0].before.as_deref(), Some("5"));
        assert_eq!(diff.objects[0].properties[0].after.as_deref(), Some("12"));
    }

    #[test]
    fn conflict_table_and_patch_preserve_auto_merges_and_apply_picks() {
        let base = "\
%YAML 1.1
--- !u!1 &10
GameObject:
  m_Name: Ship
--- !u!4 &11
Transform:
  m_GameObject: {fileID: 10}
  m_Father: {fileID: 0}
  m_LocalPosition: {x: 0, y: 0, z: 0}
--- !u!114 &12
MonoBehaviour:
  m_GameObject: {fileID: 10}
  health: 100
  speed: 10
";
        let ours = "\
%YAML 1.1
--- !u!1 &10
GameObject:
  m_Name: Ship
--- !u!4 &11
Transform:
  m_GameObject: {fileID: 10}
  m_Father: {fileID: 0}
  m_LocalPosition: {x: 4, y: 0, z: 0}
--- !u!114 &12
MonoBehaviour:
  m_GameObject: {fileID: 10}
  health: 120
  speed: 10
";
        let theirs = "\
%YAML 1.1
--- !u!1 &10
GameObject:
  m_Name: Ship
--- !u!4 &11
Transform:
  m_GameObject: {fileID: 10}
  m_Father: {fileID: 0}
  m_LocalPosition: {x: 0, y: 0, z: 0}
--- !u!114 &12
MonoBehaviour:
  m_GameObject: {fileID: 10}
  health: 150
  speed: 25
";
        let guids = GuidIndex::default();
        let table = build_conflict_table("Assets/Ship.prefab", base, ours, theirs, &guids).unwrap();
        assert_eq!(table.conflicts.len(), 1);
        assert_eq!(table.conflicts[0].property, "health");
        assert_eq!(table.conflicts[0].object_path, "Ship");
        assert_eq!(table.auto_merged.len(), 2);

        let merged = apply_unity_conflict_picks(
            base,
            ours,
            theirs,
            &[UnityPropertyPick {
                file_id: "12".into(),
                property: "health".into(),
                value: Some("135".into()),
            }],
        )
        .unwrap();
        let parsed = UnityScene::parse(&merged);
        assert_eq!(
            parsed.docs["11"].props["m_LocalPosition"].value,
            "{x: 4, y: 0, z: 0}"
        );
        assert_eq!(parsed.docs["12"].props["speed"].value, "25");
        assert_eq!(parsed.docs["12"].props["health"].value, "135");
    }

    #[test]
    fn parses_prefab_instance_modifications_and_checks_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("Assets");
        fs::create_dir_all(&assets).unwrap();
        let valid_guid = "11111111111111111111111111111111";
        fs::write(assets.join("Mat.mat"), "mat").unwrap();
        fs::write(
            assets.join("Mat.mat.meta"),
            format!("fileFormatVersion: 2\nguid: {valid_guid}\n"),
        )
        .unwrap();
        let guids = GuidIndex::build(dir.path());

        let yaml = format!(
            "\
--- !u!1001 &50
PrefabInstance:
  m_Modification:
    m_TransformParent: {{fileID: 0}}
    m_Modifications:
    - target: {{fileID: 1, guid: {valid_guid}, type: 3}}
      propertyPath: m_Name
      value: Boss
      objectReference: {{fileID: 0}}
--- !u!1 &100
GameObject:
  m_Name: DupA
--- !u!1 &100
GameObject:
  m_Name: Duplicate
  m_Component:
  - component: {{fileID: 999}}
--- !u!114 &200
MonoBehaviour:
  m_Script: {{fileID: 11500000, guid: 99999999999999999999999999999999, type: 3}}
"
        );
        let scene = UnityScene::parse(&yaml);
        assert_eq!(scene.docs["50"].props["override:m_Name"].value, "Boss");

        let issues = check_file_integrity("Assets/Arena.unity", &yaml, &guids);
        assert!(issues.iter().any(|i| i.kind == "duplicateFileId"));
        assert!(issues.iter().any(|i| i.kind == "missingFileId"));
        assert!(issues.iter().any(|i| i.kind == "missingGuid"));
    }
}
