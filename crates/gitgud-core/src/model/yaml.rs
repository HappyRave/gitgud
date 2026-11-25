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

/// As [`docs`], but only documents `read(class, stripped)` accepts are parsed; the
/// others keep their header with a `Null` body (a scene's meshes, particle systems
/// and scripts are most of its bytes).
pub(crate) fn docs_reading(text: &str, read: impl Fn(u32, bool) -> bool) -> Vec<Doc> {
    let mut out = Vec::new();
    let mut cur: Option<((u32, String, bool), Option<String>)> = None;
    for line in text.lines() {
        if let Some(header) = parse_doc_header(line) {
            if let Some((h, body)) = cur.take() {
                push(&mut out, h, body.as_deref());
            }
            let body = read(header.0, header.2).then(String::new);
            cur = Some((header, body));
        } else if let Some((_, Some(body))) = cur.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some((h, body)) = cur {
        push(&mut out, h, body.as_deref());
    }
    out
}

fn push(
    out: &mut Vec<Doc>,
    (class_id, file_id, stripped): (u32, String, bool),
    body: Option<&str>,
) {
    let body = match body {
        Some(text) => {
            let Ok(mut parsed) = YamlLoader::load_from_str(text) else {
                return;
            };
            let root = parsed.pop().unwrap_or(Yaml::Null);
            root.as_hash()
                .and_then(|h| h.values().next().cloned())
                .unwrap_or(Yaml::Null)
        }
        None => Yaml::Null,
    };
    out.push(Doc {
        class_id,
        file_id,
        stripped,
        body,
    });
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
    fn reads_keyed_lists() {
        let y = &YamlLoader::load_from_str("m_Floats:\n- _Metallic: 0.25\n- _Smoothness: 1\n")
            .unwrap()[0];
        let list = keyed_list(&y["m_Floats"]);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "_Metallic");
        assert_eq!(num(list[1].1), Some(1.0));
    }
}
