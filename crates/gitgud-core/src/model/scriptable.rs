use yaml_rust2::Yaml;

use super::guids::Guids;
use super::tree::base_name;
use super::yaml::{docs, scalar};
use crate::repo::Error;
use crate::table::{Record, Table, key_values, records};

const MONO_BEHAVIOUR: u32 = 114;
const SPRITE_ATLAS: u32 = 687_078_895;

/// Unity's bookkeeping fields, not the asset's own data; for a sprite atlas also
/// what Unity regenerates on every pack.
const SKIP: [&str; 13] = [
    "m_PackedSprites",
    "m_PackedSpriteNamesToIndex",
    "m_RenderDataMap",
    "m_EditorData",
    "m_ObjectHideFlags",
    "m_CorrespondingSourceObject",
    "m_PrefabInstance",
    "m_PrefabAsset",
    "m_GameObject",
    "m_Enabled",
    "m_EditorHideFlags",
    "m_Script",
    "m_EditorClassIdentifier",
];

/// A ScriptableObject's (or sprite atlas's) lists of records as tables, its other
/// fields as a "fields" table, and what it is: "ScriptableObject · <script>" or
/// "Sprite Atlas". A list of references becomes a one-column "asset" table.
///
/// # Errors
/// [`Error::Refused`] when the file holds neither.
pub(crate) fn scriptable_tables(
    text: &str,
    path: &str,
    guids: &Guids<'_>,
) -> Result<(String, Vec<Table>), Error> {
    let doc = docs(text)
        .into_iter()
        .find(|d| d.class_id == MONO_BEHAVIOUR || d.class_id == SPRITE_ATLAS)
        .ok_or_else(|| Error::Refused(format!("{path} isn't a ScriptableObject")))?;
    let kind = if doc.class_id == SPRITE_ATLAS {
        "Sprite Atlas".to_owned()
    } else {
        match ref_guid(&doc.body["m_Script"]) {
            Some(g) => guids.path(&g)?.map_or_else(
                || "ScriptableObject".to_owned(),
                |p| {
                    format!(
                        "ScriptableObject · {}",
                        base_name(&p).trim_end_matches(".cs")
                    )
                },
            ),
            None => "ScriptableObject".to_owned(),
        }
    };
    let Some(body) = doc.body.as_hash() else {
        return Ok((kind, Vec::new()));
    };
    let editor = doc.body["m_EditorData"].as_hash().into_iter().flatten();
    let mut tables = Vec::new();
    let mut fields = Vec::new();
    for (k, v) in body.iter().chain(editor) {
        let Some(key) = scalar(k) else { continue };
        if SKIP.contains(&key.as_str()) {
            continue;
        }
        match v.as_vec() {
            Some(items) if !items.is_empty() && items.iter().all(is_ref) => {
                let mut rows = Vec::with_capacity(items.len());
                for (i, item) in items.iter().enumerate() {
                    let mut flat = Vec::new();
                    flatten(&format!("{i}"), item, guids, &mut flat)?;
                    let value = flat.pop().map(|(_, v)| v).unwrap_or_default();
                    rows.push(Record::Flat(vec![("asset".to_owned(), value)]));
                }
                tables.push(records(&key, rows.into_iter()));
            }
            Some(items)
                if !items.is_empty()
                    && items.iter().all(|i| i.as_hash().is_some() && !is_ref(i)) =>
            {
                let mut rows = Vec::with_capacity(items.len());
                for item in items {
                    let mut flat = Vec::new();
                    for (ik, iv) in item.as_hash().expect("checked above") {
                        if let Some(ik) = scalar(ik) {
                            flatten(&ik, iv, guids, &mut flat)?;
                        }
                    }
                    rows.push(Record::Flat(flat));
                }
                tables.push(records(&key, rows.into_iter()));
            }
            _ => flatten(&key, v, guids, &mut fields)?,
        }
    }
    if !fields.is_empty() {
        tables.insert(0, key_values(fields));
    }
    Ok((kind, tables))
}

fn is_ref(y: &Yaml) -> bool {
    y.as_hash()
        .is_some_and(|h| h.contains_key(&Yaml::String("fileID".into())))
}

fn ref_guid(y: &Yaml) -> Option<String> {
    scalar(&y["guid"]).filter(|g| !g.is_empty())
}

fn flatten(
    prefix: &str,
    y: &Yaml,
    guids: &Guids<'_>,
    out: &mut Vec<(String, String)>,
) -> Result<(), Error> {
    if is_ref(y) {
        let id = scalar(&y["fileID"]).unwrap_or_default();
        let value = match ref_guid(y) {
            _ if id == "0" => "none".to_owned(),
            Some(g) => guids
                .path(&g)?
                .map_or_else(|| format!("missing ({g})"), |p| base_name(&p).to_owned()),
            None => format!("&{id}"),
        };
        out.push((prefix.to_owned(), value));
        return Ok(());
    }
    match y {
        Yaml::Hash(h) if !h.is_empty() => {
            for (k, v) in h {
                if let Some(k) = scalar(k) {
                    flatten(&format!("{prefix}.{k}"), v, guids, out)?;
                }
            }
        }
        Yaml::Array(items) if !items.is_empty() => {
            for (i, v) in items.iter().enumerate() {
                flatten(&format!("{prefix}[{i}]"), v, guids, out)?;
            }
        }
        Yaml::Hash(_) | Yaml::Array(_) => out.push((prefix.to_owned(), String::new())),
        other => out.push((prefix.to_owned(), scalar(other).unwrap_or_default())),
    }
    Ok(())
}
