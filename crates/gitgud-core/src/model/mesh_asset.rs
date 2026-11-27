use std::collections::HashMap;
use std::ops::Range;

use serde_json::json;

use super::yaml::{docs, num, scalar};
use crate::repo::Error;

pub(crate) const MESH_CLASS: u32 = 43;

/// Where each Mesh document of Unity YAML `text` is, by `fileID`.
pub(crate) fn mesh_documents(text: &str) -> HashMap<String, Range<usize>> {
    let starts: Vec<usize> = text
        .starts_with("--- !u!")
        .then_some(0)
        .into_iter()
        .chain(text.match_indices("\n--- !u!").map(|(i, _)| i + 1))
        .collect();
    let mut meshes = HashMap::new();
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(text.len());
        let line = text[start..end].lines().next().unwrap_or_default();
        if let Some((MESH_CLASS, id, _)) = crate::unity_yaml::parse_doc_header(line) {
            meshes.entry(id).or_insert(start..end);
        }
    }
    meshes
}

/// A name for a Mesh document's content, the same wherever the same mesh is stored:
/// its text without the header line (which holds the `fileID`).
pub(crate) fn mesh_key(doc: &str) -> String {
    let body = doc.split_once('\n').map_or("", |(_, b)| b);
    format!("{:016x}", xxhash_rust::xxh64::xxh64(body.as_bytes(), 0))
}

/// Unity meshes have at most four vertex streams.
const MAX_STREAMS: usize = 4;
const POSITION: usize = 0;
const NORMAL: usize = 1;
const TEXCOORD0: usize = 4;
/// Unity's `MeshTopology` values.
const TRIANGLES: u32 = 0;
const QUADS: u32 = 2;
const LINES: u32 = 3;
const LINE_STRIP: u32 = 4;
const POINTS: u32 = 5;

#[derive(Debug, Clone, Copy)]
struct Channel {
    stream: usize,
    offset: usize,
    format: u32,
    dimension: usize,
}

fn format_size(format: u32) -> Option<usize> {
    Some(match format {
        0 | 10 | 11 => 4,
        1 | 4 | 5 | 8 | 9 => 2,
        2 | 3 | 6 | 7 => 1,
        _ => return None,
    })
}

fn read(bytes: &[u8], at: usize, format: u32) -> f32 {
    let b = |n: usize| bytes.get(at..at + n);
    match format {
        0 => b(4).map_or(0.0, |s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])),
        1 => b(2).map_or(0.0, |s| half(u16::from_le_bytes([s[0], s[1]]))),
        2 => b(1).map_or(0.0, |s| f32::from(s[0]) / 255.0),
        3 => b(1).map_or(0.0, |s| (f32::from(s[0] as i8) / 127.0).max(-1.0)),
        4 => b(2).map_or(0.0, |s| {
            f32::from(u16::from_le_bytes([s[0], s[1]])) / 65535.0
        }),
        5 => b(2).map_or(0.0, |s| {
            (f32::from(i16::from_le_bytes([s[0], s[1]])) / 32767.0).max(-1.0)
        }),
        _ => 0.0,
    }
}

fn half(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let frac = f32::from(h & 0x3ff);
    sign * match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + frac / 1024.0) * 2f32.powi(e - 15),
    }
}

fn hex(s: &str) -> Vec<u8> {
    let s = s.trim().as_bytes();
    s.as_chunks::<2>()
        .0
        .iter()
        .filter_map(|p| {
            std::str::from_utf8(p)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        })
        .collect()
}

/// The raw text after `key:` on its own line; YAML would read long hex runs as numbers.
fn raw_value<'t>(text: &'t str, key: &str) -> &'t str {
    text.lines()
        .find_map(|l| l.trim_start().strip_prefix(key)?.strip_prefix(':'))
        .unwrap_or("")
        .trim()
}

/// Converts a Unity `Mesh` asset (YAML with hex vertex data) into a GLB with one
/// primitive per sub-mesh, mirrored into right-handed space.
///
/// # Errors
/// [`Error::Read`] when the file holds no mesh or its vertex layout isn't one this reads.
pub(crate) fn mesh_asset_to_glb(text: &str) -> Result<Vec<u8>, Error> {
    // The first Mesh document alone, so its hex runs are the ones read.
    let range = mesh_documents(text)
        .into_values()
        .min_by_key(|r| r.start)
        .ok_or_else(|| Error::Read("this .asset holds no Mesh".into()))?;
    let text = &text[range];
    let doc = docs(text)
        .into_iter()
        .find(|d| d.class_id == MESH_CLASS)
        .ok_or_else(|| Error::Read("the Mesh document doesn't parse".into()))?;
    let m = &doc.body;
    let vd = &m["m_VertexData"];
    let count = num(&vd["m_VertexCount"]).unwrap_or(0.0) as usize;
    let channels: Vec<Channel> = vd["m_Channels"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|c| Channel {
            stream: num(&c["stream"]).unwrap_or(0.0) as usize,
            offset: num(&c["offset"]).unwrap_or(0.0) as usize,
            format: num(&c["format"]).unwrap_or(0.0) as u32,
            dimension: num(&c["dimension"]).unwrap_or(0.0) as usize & 0xf,
        })
        .collect();
    let data = hex(raw_value(text, "_typelessdata"));
    let indices_raw = hex(raw_value(text, "m_IndexBuffer"));
    let wide = num(&m["m_IndexFormat"]).unwrap_or(0.0) as u32 == 1;

    if let Some(c) = channels.iter().find(|c| c.stream >= MAX_STREAMS) {
        return Err(Error::Read(format!(
            "vertex stream {} doesn't exist",
            c.stream
        )));
    }
    let streams = channels.iter().map(|c| c.stream + 1).max().unwrap_or(0);
    let mut strides = vec![0usize; streams];
    for c in channels.iter().filter(|c| c.dimension > 0) {
        let size = format_size(c.format)
            .ok_or_else(|| Error::Read(format!("vertex format {} isn't supported", c.format)))?;
        strides[c.stream] = strides[c.stream].max(c.offset.saturating_add(size * c.dimension));
    }
    let mut starts = vec![0usize; streams];
    let mut at = 0usize;
    for s in 0..streams {
        starts[s] = at;
        at = strides[s]
            .checked_mul(count)
            .and_then(|n| n.checked_add(at))
            .ok_or_else(|| Error::Read(format!("{count} vertices don't fit in memory")))?;
        at = at.div_ceil(16) * 16;
    }
    // The last stream needn't be padded.
    let needed = starts
        .last()
        .map_or(0, |&s| s + strides[streams - 1] * count);
    if data.len() < needed {
        return Err(Error::Read(format!(
            "the vertex data holds {} bytes, {needed} expected (a compressed mesh?)",
            data.len()
        )));
    }
    let attr = |ix: usize, dims: usize| -> Option<Vec<f32>> {
        let c = channels.get(ix).filter(|c| c.dimension > 0)?;
        let size = format_size(c.format)?;
        let mut out = Vec::with_capacity(count * dims);
        for v in 0..count {
            let base = starts[c.stream] + v * strides[c.stream] + c.offset;
            for d in 0..dims {
                out.push(if d < c.dimension {
                    read(&data, base + d * size, c.format)
                } else {
                    0.0
                });
            }
        }
        Some(out)
    };
    let mut positions =
        attr(POSITION, 3).ok_or_else(|| Error::Read("the mesh has no positions".into()))?;
    let mut normals = attr(NORMAL, 3);
    let uvs = attr(TEXCOORD0, 2);
    for p in positions.as_chunks_mut::<3>().0 {
        p[0] = -p[0];
    }
    if let Some(n) = normals.as_mut() {
        for p in n.as_chunks_mut::<3>().0 {
            p[0] = -p[0];
        }
    }
    let all_indices: Vec<u32> = if wide {
        indices_raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    } else {
        indices_raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u32::from(u16::from_le_bytes(*b)))
            .collect()
    };

    let mut buf = GlbBuffers::default();
    let mut attributes = serde_json::Map::new();
    attributes.insert("POSITION".into(), json!(buf.positions(&positions)));
    if let Some(n) = &normals {
        attributes.insert(
            "NORMAL".into(),
            json!(buf.accessor(
                f32_bytes(n),
                json!({ "componentType": 5126, "count": count, "type": "VEC3" }),
                VERTICES
            )),
        );
    }
    if let Some(t) = &uvs {
        attributes.insert(
            "TEXCOORD_0".into(),
            json!(buf.accessor(
                f32_bytes(t),
                json!({ "componentType": 5126, "count": count, "type": "VEC2" }),
                VERTICES
            )),
        );
    }
    let width = if wide { 4 } else { 2 };
    let mut primitives = Vec::new();
    for sm in m["m_SubMeshes"].as_vec().into_iter().flatten() {
        let first = num(&sm["firstByte"]).unwrap_or(0.0) as usize / width;
        let n = num(&sm["indexCount"]).unwrap_or(0.0) as usize;
        let base = num(&sm["baseVertex"]).unwrap_or(0.0) as u32;
        let topology = num(&sm["topology"]).unwrap_or(0.0) as u32;
        let Some(slice) = first
            .checked_add(n)
            .and_then(|end| all_indices.get(first..end))
        else {
            return Err(Error::Read(format!(
                "a sub-mesh's {n} indices from {first} aren't in the index buffer"
            )));
        };
        let shifted = slice.iter().map(|i| i.wrapping_add(base));
        let (indices, mode): (Vec<u32>, u32) = match topology {
            // Triangles and quads: the mirror flips their winding.
            TRIANGLES => (
                shifted
                    .collect::<Vec<_>>()
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|t| [t[0], t[2], t[1]])
                    .collect(),
                4,
            ),
            QUADS => (
                shifted
                    .collect::<Vec<_>>()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|q| [q[0], q[2], q[1], q[0], q[3], q[2]])
                    .collect(),
                4,
            ),
            LINES => (shifted.collect(), 1),
            LINE_STRIP => (shifted.collect(), 3),
            POINTS => (shifted.collect(), 0),
            other => return Err(Error::Read(format!("mesh topology {other} is unknown"))),
        };
        let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let acc = buf.accessor(
            bytes,
            json!({ "componentType": 5125, "count": indices.len(), "type": "SCALAR" }),
            INDICES,
        );
        primitives.push(json!({ "attributes": attributes, "indices": acc, "mode": mode }));
    }
    let name = scalar(&m["m_Name"]).unwrap_or_default();
    let gltf = json!({
        "asset": { "version": "2.0", "generator": "gitgud" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": name, "mesh": 0 }],
        "meshes": [{ "name": name, "primitives": primitives }],
    });
    Ok(buf.finish(gltf))
}

/// The binary chunk of a GLB being written, with its buffer views and accessors.
#[derive(Default)]
pub(crate) struct GlbBuffers {
    bin: Vec<u8>,
    views: Vec<serde_json::Value>,
    accessors: Vec<serde_json::Value>,
}

/// glTF buffer view targets.
pub(crate) const VERTICES: u32 = 34962;
pub(crate) const INDICES: u32 = 34963;

impl GlbBuffers {
    /// Appends `bytes` (4-byte aligned) as a buffer view, `target` naming vertex or
    /// index data; returns the view's index.
    pub(crate) fn view(&mut self, bytes: Vec<u8>, target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let mut view =
            json!({ "buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len() });
        if let Some(t) = target {
            view["target"] = json!(t);
        }
        self.bin.extend(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    /// As [`Self::view`], with `accessor` over it; returns the accessor's index.
    pub(crate) fn accessor(
        &mut self,
        bytes: Vec<u8>,
        mut accessor: serde_json::Value,
        target: u32,
    ) -> usize {
        accessor["bufferView"] = json!(self.view(bytes, Some(target)));
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    /// A float VEC3 accessor of `positions` (x, y, z, ...), with the bounds glTF
    /// requires of positions.
    pub(crate) fn positions(&mut self, positions: &[f32]) -> usize {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in positions.as_chunks::<3>().0 {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let count = positions.len() / 3;
        self.accessor(
            f32_bytes(positions),
            json!({ "componentType": 5126, "count": count, "type": "VEC3", "min": lo, "max": hi }),
            VERTICES,
        )
    }

    /// The GLB of `gltf` with these buffers, views and accessors.
    pub(crate) fn finish(self, mut gltf: serde_json::Value) -> Vec<u8> {
        gltf["buffers"] = json!([{ "byteLength": self.bin.len() }]);
        gltf["bufferViews"] = json!(self.views);
        gltf["accessors"] = json!(self.accessors);
        glb(&gltf, self.bin)
    }
}

pub(crate) fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub(crate) fn glb(json: &serde_json::Value, mut bin: Vec<u8>) -> Vec<u8> {
    let mut j = serde_json::to_vec(json).unwrap_or_default();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((j.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(j);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out
}

/// A one-triangle Unity `Mesh` document `&id` named "Tri", for tests.
#[cfg(test)]
pub(crate) fn tri_mesh(id: &str) -> String {
    let f32s = |v: &[f32]| -> String {
        v.iter()
            .flat_map(|f| f.to_le_bytes())
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    // One stream: position (3 × f32) + uv0 (2 × f16), stride 16.
    let mut data = String::new();
    for (p, uv) in [
        ([1.0f32, 0.0, 0.0], [0x3c00u16, 0x0000]),
        ([0.0, 1.0, 0.0], [0, 0x3c00]),
        ([0.0, 0.0, 1.0], [0x3800, 0x3800]),
    ] {
        data.push_str(&f32s(&p));
        for h in uv {
            data.push_str(
                &h.to_le_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            );
        }
    }
    let channels: String =
        (0..14)
            .map(|i| match i {
                0 => "    - stream: 0\n      offset: 0\n      format: 0\n      dimension: 3\n"
                    .to_owned(),
                4 => "    - stream: 0\n      offset: 12\n      format: 1\n      dimension: 2\n"
                    .to_owned(),
                _ => "    - stream: 0\n      offset: 0\n      format: 0\n      dimension: 0\n"
                    .to_owned(),
            })
            .collect();
    format!(
        "--- !u!43 &{id}\nMesh:\n  m_Name: Tri\n  m_SubMeshes:\n  - serializedVersion: 2\n    firstByte: 0\n    indexCount: 3\n    topology: 0\n    baseVertex: 0\n  m_IndexFormat: 0\n  m_IndexBuffer: 000001000200\n  m_VertexData:\n    serializedVersion: 3\n    m_VertexCount: 3\n    m_Channels:\n{channels}    m_DataSize: 48\n    _typelessdata: {data}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_unity_mesh_asset_to_glb() {
        let text = format!("%YAML 1.1\n{}", tri_mesh("4300000"));
        let glb = mesh_asset_to_glb(&text).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let gltf: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert_eq!(gltf["meshes"][0]["name"], "Tri");
        let prim = &gltf["meshes"][0]["primitives"][0];
        assert!(prim["attributes"]["TEXCOORD_0"].is_number());
        assert_eq!(gltf["accessors"][0]["min"], json!([-1.0, 0.0, 0.0]));

        let bin = &glb[20 + json_len + 8..];
        let at = |acc: usize, i: usize| {
            let off = gltf["bufferViews"]
                [gltf["accessors"][acc]["bufferView"].as_u64().unwrap() as usize]["byteOffset"]
                .as_u64()
                .unwrap() as usize;
            &bin[off + i * 4..off + i * 4 + 4]
        };
        assert_eq!(
            f32::from_le_bytes(at(0, 0).try_into().unwrap()),
            -1.0,
            "x mirrored"
        );
        assert_eq!(
            f32::from_le_bytes(at(1, 4).try_into().unwrap()),
            0.5,
            "half-float uv"
        );
        let idx = prim["indices"].as_u64().unwrap() as usize;
        let tri: Vec<u32> = (0..3)
            .map(|i| u32::from_le_bytes(at(idx, i).try_into().unwrap()))
            .collect();
        assert_eq!(tri, [0, 2, 1], "winding flipped with the mirror");
    }

    fn gltf_of(glb: &[u8]) -> serde_json::Value {
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        serde_json::from_slice(&glb[20..20 + json_len]).unwrap()
    }

    #[test]
    fn refuses_vertex_data_shorter_than_its_layout() {
        let short = tri_mesh("1").replace("m_VertexCount: 3", "m_VertexCount: 4000000000");
        let err = mesh_asset_to_glb(&short).unwrap_err().to_string();
        assert!(err.contains("48 bytes"), "{err}");
        let stream = tri_mesh("1").replacen("stream: 0", "stream: 1000000000", 1);
        assert!(mesh_asset_to_glb(&stream).is_err());
    }

    #[test]
    fn triangulates_quads_and_reads_the_first_mesh_only() {
        let quad = tri_mesh("1")
            .replace("topology: 0", "topology: 2")
            .replace("indexCount: 3", "indexCount: 4")
            .replace(
                "m_IndexBuffer: 000001000200",
                "m_IndexBuffer: 0000010002000000",
            );
        let other = tri_mesh("2").replace("m_Name: Tri", "m_Name: Other");
        let gltf = gltf_of(&mesh_asset_to_glb(&format!("%YAML 1.1\n{quad}{other}")).unwrap());
        assert_eq!(gltf["meshes"][0]["name"], "Tri");
        let prim = &gltf["meshes"][0]["primitives"][0];
        assert_eq!(prim["mode"], 4);
        assert_eq!(
            gltf["accessors"][prim["indices"].as_u64().unwrap() as usize]["count"],
            6
        );
        let lines = tri_mesh("1").replace("topology: 0", "topology: 3");
        assert_eq!(
            gltf_of(&mesh_asset_to_glb(&lines).unwrap())["meshes"][0]["primitives"][0]["mode"],
            1
        );
    }

    #[test]
    fn decodes_half_floats() {
        assert_eq!(half(0x3c00), 1.0);
        assert_eq!(half(0xc000), -2.0);
        assert_eq!(half(0x3800), 0.5);
    }
}
