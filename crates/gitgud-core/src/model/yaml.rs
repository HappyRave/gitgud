use rayon::prelude::*;
use yaml_rust2::{Yaml, YamlLoader};

use crate::unity_yaml::parse_doc_header;

/// One `--- !u!<class> &<fileID>` document of a Unity YAML file, parsed.
#[derive(Debug, Clone)]
pub(crate) struct Doc {
    pub class_id: u32,
    pub file_id: String,
    pub stripped: bool,
    pub body: Yaml,
}

/// A `{fileID: …, guid: …}` reference; `fileID: 0` is no reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjRef {
    pub file_id: String,
    pub guid: Option<String>,
}

/// Splits Unity YAML into documents and parses each body. Documents that don't
/// parse are skipped.
pub(crate) fn docs(text: &str) -> Vec<Doc> {
    docs_reading(text, |_, _| true)
}

/// Bodies to parse, in bytes, from which they are parsed on several threads.
const PARALLEL_FROM: usize = 1 << 20;

/// A document's header and, when it is to be parsed, its body.
type Part<'t> = ((u32, String, bool), Option<&'t str>);

/// As [`docs`], but only documents `read(class, stripped)` accepts are parsed; the
/// others keep their header with a `Null` body (a scene's meshes, particle systems
/// and scripts are most of its bytes). A big file's documents are parsed in parallel.
pub(crate) fn docs_reading(text: &str, read: impl Fn(u32, bool) -> bool) -> Vec<Doc> {
    let mut parts: Vec<Part<'_>> = Vec::new();
    let mut cur: Option<((u32, String, bool), Option<usize>)> = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if let Some(header) = parse_doc_header(line.trim_end_matches(['\n', '\r'])) {
            if let Some((h, start)) = cur.take() {
                parts.push((h, start.map(|s| &text[s..at])));
            }
            let start = read(header.0, header.2).then_some(at + line.len());
            cur = Some((header, start));
        }
        at += line.len();
    }
    if let Some((h, start)) = cur {
        parts.push((h, start.map(|s| &text[s..])));
    }

    let size: usize = parts.iter().filter_map(|p| p.1).map(str::len).sum();
    if size < PARALLEL_FROM {
        return parts.into_iter().filter_map(parse).collect();
    }
    parts.into_par_iter().filter_map(parse).collect()
}

/// The document `part` describes; `None` when its body doesn't parse.
fn parse(((class_id, file_id, stripped), body): Part<'_>) -> Option<Doc> {
    let body = match body {
        Some(text) => {
            let mut parsed = YamlLoader::load_from_str(text).ok()?;
            let root = parsed.pop().unwrap_or(Yaml::Null);
            root.as_hash()
                .and_then(|h| h.values().next().cloned())
                .unwrap_or(Yaml::Null)
        }
        None => Yaml::Null,
    };
    Some(Doc {
        class_id,
        file_id,
        stripped,
        body,
    })
}

/// A scalar as text, whatever YAML typed it as.
pub(crate) fn scalar(y: &Yaml) -> Option<String> {
    match y {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        Yaml::Null => Some(String::new()),
        _ => None,
    }
}

pub(crate) fn num(y: &Yaml) -> Option<f32> {
    match y {
        Yaml::Integer(i) => Some(*i as f32),
        Yaml::Real(_) => y.as_f64().map(|f| f as f32),
        Yaml::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn axes<const N: usize>(y: &Yaml, keys: [&str; N], default: [f32; N]) -> [f32; N] {
    let mut out = default;
    for (o, k) in out.iter_mut().zip(keys) {
        if let Some(v) = num(&y[k]) {
            *o = v;
        }
    }
    out
}

pub(crate) fn vec3(y: &Yaml, default: [f32; 3]) -> [f32; 3] {
    axes(y, ["x", "y", "z"], default)
}

pub(crate) fn quat(y: &Yaml) -> [f32; 4] {
    axes(y, ["x", "y", "z", "w"], [0.0, 0.0, 0.0, 1.0])
}

pub(crate) fn vec2(y: &Yaml, default: [f32; 2]) -> [f32; 2] {
    axes(y, ["x", "y"], default)
}

pub(crate) fn color(y: &Yaml) -> [f32; 4] {
    axes(y, ["r", "g", "b", "a"], [1.0; 4])
}

pub(crate) fn obj_ref(y: &Yaml) -> Option<ObjRef> {
    let file_id = scalar(&y["fileID"])?;
    if file_id.is_empty() || file_id == "0" {
        return None;
    }
    let guid = scalar(&y["guid"]).filter(|g| !g.is_empty());
    Some(ObjRef { file_id, guid })
}

/// Unity's serialized `[- key: value]` lists (`m_Floats`, `m_Colors`, `m_TexEnvs`)
/// as `(key, value)` pairs.
pub(crate) fn keyed_list(y: &Yaml) -> Vec<(String, &Yaml)> {
    let Some(items) = y.as_vec() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let (k, v) = item.as_hash()?.iter().next()?;
            Some((scalar(k)?, v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFAB: &str = "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1 &100\nGameObject:\n  m_Name: Boat\n  m_Component:\n  - component: {fileID: 400}\n--- !u!4 &400\nTransform:\n  m_GameObject: {fileID: 100}\n  m_LocalPosition: {x: 1, y: 2.5, z: -3}\n  m_LocalRotation: {x: 0.5, y: 0.5, z: -0.5, w: 0.5}\n  m_Father: {fileID: 0}\n--- !u!1001 &900 stripped\nPrefabInstance:\n  m_SourcePrefab: {fileID: 100100000, guid: 0123456789abcdef0123456789abcdef, type: 3}\n";

    #[test]
    fn splits_documents_and_reads_values() {
        let d = docs(PREFAB);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].class_id, d[0].file_id.as_str()), (1, "100"));
        assert_eq!(scalar(&d[0].body["m_Name"]).as_deref(), Some("Boat"));
        let some = docs_reading(PREFAB, |class, stripped| class == 1 || stripped);
        assert_eq!(some.len(), 3);
        assert_eq!((some[1].class_id, some[1].file_id.as_str()), (4, "400"));
        assert!(
            some[1].body.is_null(),
            "an unread class keeps its header only"
        );
        assert_eq!(scalar(&some[0].body["m_Name"]).as_deref(), Some("Boat"));
        assert!(obj_ref(&some[2].body["m_SourcePrefab"]).is_some());
        assert_eq!(
            vec3(&d[1].body["m_LocalPosition"], [0.0; 3]),
            [1.0, 2.5, -3.0]
        );
        assert_eq!(quat(&d[1].body["m_LocalRotation"])[1], 0.5);
        assert_eq!(obj_ref(&d[1].body["m_Father"]), None);
        assert!(d[2].stripped);
        assert_eq!(
            obj_ref(&d[2].body["m_SourcePrefab"]),
            Some(ObjRef {
                file_id: "100100000".into(),
                guid: Some("0123456789abcdef0123456789abcdef".into())
            })
        );
    }

    #[test]
    fn parses_a_big_file_on_threads_in_order() {
        let mut text = String::from("%YAML 1.1\n");
        for i in 0..20_000 {
            text.push_str(&format!(
                "--- !u!1 &{i}\r\nGameObject:\r\n  m_Name: Object number {i} with a longer name\r\n  m_IsActive: 1\r\n"
            ));
            if i % 1000 == 0 {
                text.push_str(&format!(
                    "--- !u!43 &{}\nMesh:\n  m_Name: skipped\n",
                    i + 1_000_000
                ));
            }
        }
        assert!(text.len() > PARALLEL_FROM);
        let d = docs_reading(&text, |class, _| class == 1);
        assert_eq!(d.len(), 20_020);
        let objects: Vec<&Doc> = d.iter().filter(|x| x.class_id == 1).collect();
        assert!(
            objects
                .iter()
                .enumerate()
                .all(|(i, x)| x.file_id == i.to_string())
        );
        assert_eq!(
            scalar(&objects[19_999].body["m_Name"]).as_deref(),
            Some("Object number 19999 with a longer name")
        );
        assert!(
            d.iter()
                .filter(|x| x.class_id == 43)
                .all(|x| x.body.is_null())
        );
    }

    #[test]
    fn reads_keyed_lists() {
        let y = &YamlLoader::load_from_str("m_Floats:\n- _Metallic: 0.25\n- _Smoothness: 1\n")
            .unwrap()[0];
        let list = keyed_list(&y["m_Floats"]);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "_Metallic");
        assert_eq!(num(list[1].1), Some(1.0));
    }
}
