use std::collections::HashMap;

use yaml_rust2::{Yaml, YamlLoader};

use super::yaml::{ObjRef, num, obj_ref, scalar};

const MESH_CLASS: &str = "43";
const SPRITE_CLASS: &str = "213";
const CLIP_CLASS: &str = "74";
const TRANSFORM_CLASS: &str = "4";
const GAME_OBJECT_CLASS: &str = "1";
const ROOT_NODE: &str = "//RootNode";
const LEGACY_ROOT_TRANSFORM: &str = "400000";
const LEGACY_ROOT_GAME_OBJECT: &str = "100000";
/// The root ids every model import uses since Unity stopped writing the name table.
const ROOT_TRANSFORM: &str = "-8679921383154817045";
const ROOT_GAME_OBJECT: &str = "919132149155446097";

/// Unity's `fileID` for a model sub-asset that isn't in the name table:
/// xxHash64 of `Type:<Class>-><name><occurrence>`, read as signed.
pub(crate) fn hashed_id(class: &str, name: &str, occurrence: usize) -> String {
    let key = format!("Type:{class}->{name}{occurrence}");
    (xxhash_rust::xxh64::xxh64(key.as_bytes(), 0) as i64).to_string()
}

/// What gitgud needs from an asset's `.meta` file.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Meta {
    pub guid: Option<String>,
    /// Mesh and sprite sub-asset `fileID` → name, from `internalIDToNameTable`, the
    /// sprite sheet's `internalID`s or the older `fileIDToRecycleName`.
    pub names: HashMap<String, String>,
    /// Model material name → the `.mat` Unity uses instead (`externalObjects`).
    pub material_remaps: HashMap<String, ObjRef>,
    /// The importer's `globalScale`.
    pub global_scale: f32,
    /// `fileID`s of a model's root Transform and GameObject (`//RootNode`).
    pub root_transform: Option<String>,
    pub root_game_object: Option<String>,
    /// Animation clip `fileID` → clip name.
    pub clips: HashMap<String, String>,
    /// Clip name → the take in the model file it is cut from (`clipAnimations`).
    pub takes: HashMap<String, String>,
}

impl Meta {
    pub fn parse(text: &str) -> Self {
        let mut meta = Meta {
            global_scale: 1.0,
            ..Meta::default()
        };
        let Ok(mut docs) = YamlLoader::load_from_str(text) else {
            return meta;
        };
        let root = docs.pop().unwrap_or(Yaml::Null);
        meta.guid = scalar(&root["guid"]).filter(|g| !g.is_empty());
        let Some(importer) = root
            .as_hash()
            .and_then(|h| {
                h.iter()
                    .find(|(k, _)| k.as_str().is_some_and(|k| k.ends_with("Importer")))
            })
            .map(|(_, v)| v)
        else {
            return meta;
        };
        if let Some(table) = importer["internalIDToNameTable"].as_vec() {
            for entry in table {
                let Some(name) = scalar(&entry["second"]) else {
                    continue;
                };
                let Some((class, id)) = entry["first"].as_hash().and_then(|h| h.iter().next())
                else {
                    continue;
                };
                let (Some(class), Some(id)) = (scalar(class), scalar(id)) else {
                    continue;
                };
                match class.as_str() {
                    MESH_CLASS | SPRITE_CLASS => {
                        meta.names.insert(id, name);
                    }
                    CLIP_CLASS => {
                        meta.clips.insert(id, name);
                    }
                    TRANSFORM_CLASS if name == ROOT_NODE => meta.root_transform = Some(id),
                    GAME_OBJECT_CLASS if name == ROOT_NODE => meta.root_game_object = Some(id),
                    _ => {}
                }
            }
        }
        if let Some(old) = importer["fileIDToRecycleName"].as_hash() {
            for (id, name) in old {
                if let (Some(id), Some(name)) = (scalar(id), scalar(name)) {
                    if name == ROOT_NODE {
                        if id.starts_with('4') {
                            meta.root_transform.get_or_insert(id.clone());
                        } else if id.starts_with('1') {
                            meta.root_game_object.get_or_insert(id.clone());
                        }
                    }
                    if id.starts_with("74") && id.len() == 7 {
                        meta.clips.entry(id.clone()).or_insert_with(|| name.clone());
                    }
                    meta.names.entry(id).or_insert(name);
                }
            }
        }
        for clip in importer["animations"]["clipAnimations"]
            .as_vec()
            .into_iter()
            .flatten()
        {
            let Some(name) = scalar(&clip["name"]).filter(|n| !n.is_empty()) else {
                continue;
            };
            meta.clips
                .entry(hashed_id("AnimationClip", &name, 0))
                .or_insert_with(|| name.clone());
            if let Some(take) = scalar(&clip["takeName"]).filter(|t| !t.is_empty()) {
                meta.takes.insert(name, take);
            }
        }
        if meta.root_transform.is_none() && importer.as_hash().is_some_and(|h| !h.is_empty()) {
            let legacy = !importer["fileIDToRecycleName"].is_badvalue();
            let (t, g) = if legacy {
                (LEGACY_ROOT_TRANSFORM, LEGACY_ROOT_GAME_OBJECT)
            } else {
                (ROOT_TRANSFORM, ROOT_GAME_OBJECT)
            };
            meta.root_transform = Some(t.to_owned());
            meta.root_game_object.get_or_insert_with(|| g.to_owned());
        }
        if let Some(list) = importer["externalObjects"].as_vec() {
            for entry in list {
                let first = &entry["first"];
                if scalar(&first["type"]).as_deref() != Some("UnityEngine:Material") {
                    continue;
                }
                if let (Some(name), Some(r)) = (scalar(&first["name"]), obj_ref(&entry["second"])) {
                    meta.material_remaps.insert(name, r);
                }
            }
        }
        for sprite in importer["spriteSheet"]["sprites"]
            .as_vec()
            .into_iter()
            .flatten()
        {
            if let (Some(id), Some(name)) = (scalar(&sprite["internalID"]), scalar(&sprite["name"]))
                && id != "0"
            {
                meta.names.entry(id).or_insert(name);
            }
        }
        // Unity treats a zero or negative scale as unset.
        if let Some(s) = num(&importer["meshes"]["globalScale"]).filter(|s| *s > 0.0) {
            meta.global_scale = s;
        }
        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_model_importer_settings() {
        let m = Meta::parse(
            "fileFormatVersion: 2\nguid: 5a1b2c3d4e5f60718293a4b5c6d7e8f9\nModelImporter:\n  serializedVersion: 22200\n  internalIDToNameTable:\n  - first:\n      74: 1827226128182048838\n    second: Take 001\n  - first:\n      43: -4216859302048453862\n    second: Hull\n  - first:\n      4: -8679921383154817045\n    second: //RootNode\n  externalObjects:\n  - first:\n      type: UnityEngine:Material\n      assembly: UnityEngine.CoreModule\n      name: HullMat\n    second: {fileID: 2100000, guid: 99999999999999999999999999999999aa, type: 2}\n  meshes:\n    globalScale: 0.5\n",
        );
        assert_eq!(m.guid.as_deref(), Some("5a1b2c3d4e5f60718293a4b5c6d7e8f9"));
        assert_eq!(
            m.names.get("-4216859302048453862").map(String::as_str),
            Some("Hull")
        );
        assert_eq!(m.names.len(), 1);
        assert_eq!(m.material_remaps["HullMat"].file_id, "2100000");
        assert_eq!(m.global_scale, 0.5);
        assert_eq!(m.root_transform.as_deref(), Some("-8679921383154817045"));
    }

    #[test]
    fn hashes_ids_like_unity() {
        // fileIDs a real project's prefabs use for this FBX's two meshes.
        assert_eq!(
            hashed_id("Mesh", "MagicWand_ToolGun", 0),
            "8742364090748965452"
        );
        assert_eq!(
            hashed_id("Mesh", "MagicWand_ToolGun_trigger", 0),
            "-8767939539822045562"
        );
    }

    #[test]
    fn falls_back_to_unitys_constant_root_ids() {
        let m = Meta::parse(
            "guid: abc\nModelImporter:\n  internalIDToNameTable: []\n  animations:\n    clipAnimations:\n    - name: Run\n      takeName: Take 001\n",
        );
        assert_eq!(m.root_transform.as_deref(), Some(ROOT_TRANSFORM));
        assert_eq!(m.root_game_object.as_deref(), Some(ROOT_GAME_OBJECT));
        assert_eq!(m.clips[&hashed_id("AnimationClip", "Run", 0)], "Run");
        assert_eq!(m.takes["Run"], "Take 001");
    }

    #[test]
    fn reads_sprite_names_and_ignores_a_bad_scale() {
        let m = Meta::parse(
            "guid: abc\nTextureImporter:\n  internalIDToNameTable:\n  - first:\n      213: 21300000\n    second: hero_0\n  spriteSheet:\n    sprites:\n    - name: hero_1\n      internalID: -1234567890\n    - name: hero_2\n      internalID: 0\n",
        );
        assert_eq!(m.names["21300000"], "hero_0");
        assert_eq!(m.names["-1234567890"], "hero_1");
        assert_eq!(m.names.len(), 2);
        let m = Meta::parse("guid: abc\nModelImporter:\n  meshes:\n    globalScale: 0\n");
        assert_eq!(m.global_scale, 1.0);
    }

    #[test]
    fn reads_legacy_recycle_names() {
        let m = Meta::parse(
            "guid: abc\nModelImporter:\n  fileIDToRecycleName:\n    4300000: Hull\n    4300002: Cabin\n",
        );
        assert_eq!(m.names["4300002"], "Cabin");
        assert_eq!(m.global_scale, 1.0);
        assert_eq!(m.root_transform.as_deref(), Some("400000"));
    }
}
