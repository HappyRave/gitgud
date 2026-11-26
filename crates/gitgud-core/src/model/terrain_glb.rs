use image::{Rgba, RgbaImage, imageops::FilterType};
use rayon::prelude::*;
use serde_json::json;

use super::mesh_asset::glb;
use super::terrain::TerrainData;

/// The most heightmap samples drawn along a side: a 2049 heightmap is thinned to 513.
pub(crate) const MAX_GRID: usize = 513;

/// Sample indices along a side of `n` samples, every `step`, the last one always in.
fn samples(n: usize, step: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (0..n).step_by(step).collect();
    if out.last() != Some(&(n - 1)) {
        out.push(n - 1);
    }
    out
}

/// The terrain as GLB: its heightmap as a grid mesh in three.js's space (x mirrored),
/// textured with `texture` (PNG) when given.
pub(crate) fn terrain_glb(t: &TerrainData, name: &str, texture: Option<&[u8]>) -> Vec<u8> {
    let step = (t.width.max(t.depth) - 1).div_ceil(MAX_GRID - 1).max(1);
    let (xs, zs) = (samples(t.width, step), samples(t.depth, step));
    let cell = [
        t.size[0] / (t.width - 1) as f32,
        t.size[2] / (t.depth - 1) as f32,
    ];
    let mut positions = Vec::with_capacity(xs.len() * zs.len() * 3);
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(xs.len() * zs.len() * 2);
    for &z in &zs {
        for &x in &xs {
            let h = |x: usize, z: usize| t.height(x, z) * t.size[1];
            positions.extend([-(x as f32 * cell[0]), h(x, z), z as f32 * cell[1]]);
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(t.width - 1));
            let (z0, z1) = (z.saturating_sub(1), (z + 1).min(t.depth - 1));
            let dx = (h(x1, z) - h(x0, z)) / ((x1 - x0) as f32 * cell[0]);
            let dz = (h(x, z1) - h(x, z0)) / ((z1 - z0) as f32 * cell[1]);
            // Unity's normal is (-dx, 1, -dz); mirroring x flips its x.
            let len = (dx * dx + 1.0 + dz * dz).sqrt();
            normals.extend([dx / len, 1.0 / len, -dz / len]);
            uvs.extend([
                x as f32 / (t.width - 1) as f32,
                1.0 - z as f32 / (t.depth - 1) as f32,
            ]);
        }
    }
    let row = xs.len() as u32;
    let mut indices = Vec::with_capacity((xs.len() - 1) * (zs.len() - 1) * 6);
    for j in 0..zs.len() as u32 - 1 {
        for i in 0..row - 1 {
            let a = j * row + i;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            // Seen from above in three.js's space, these wind counter-clockwise.
            indices.extend([a, b, c, b, d, c]);
        }
    }

    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut push = |bytes: Vec<u8>, accessor: Option<serde_json::Value>, target: Option<u32>| {
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut view = json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len() });
        if let Some(t) = target {
            view["target"] = json!(t);
        }
        bin.extend(bytes);
        views.push(view);
        let view_ix = views.len() - 1;
        match accessor {
            Some(mut a) => {
                a["bufferView"] = json!(view_ix);
                accessors.push(a);
                accessors.len() - 1
            }
            None => view_ix,
        }
    };
    let floats = |v: &[f32]| v.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    let count = positions.len() / 3;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in positions.as_chunks::<3>().0 {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let pos = push(
        floats(&positions),
        Some(
            json!({ "componentType": 5126, "count": count, "type": "VEC3", "min": lo, "max": hi }),
        ),
        Some(34962),
    );
    let nor = push(
        floats(&normals),
        Some(json!({ "componentType": 5126, "count": count, "type": "VEC3" })),
        Some(34962),
    );
    let uv = push(
        floats(&uvs),
        Some(json!({ "componentType": 5126, "count": count, "type": "VEC2" })),
        Some(34962),
    );
    let idx = push(
        indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
        Some(json!({ "componentType": 5125, "count": indices.len(), "type": "SCALAR" })),
        Some(34963),
    );
    let mut material = json!({ "name": name, "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 1.0 } });
    let mut extra = serde_json::Map::new();
    if let Some(png) = texture {
        let view = push(png.to_vec(), None, None);
        material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": 0 });
        extra.insert(
            "images".into(),
            json!([{ "bufferView": view, "mimeType": "image/png" }]),
        );
        extra.insert(
            "samplers".into(),
            json!([{ "magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071 }]),
        );
        extra.insert("textures".into(), json!([{ "source": 0, "sampler": 0 }]));
    }
    let mut gltf = json!({
        "asset": { "version": "2.0", "generator": "gitgud" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": name, "mesh": 0 }],
        "meshes": [{ "name": name, "primitives": [{
            "attributes": { "POSITION": pos, "NORMAL": nor, "TEXCOORD_0": uv },
            "indices": idx,
            "material": 0,
        }] }],
        "materials": [material],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": views,
        "accessors": accessors,
    });
    for (k, v) in extra {
        gltf[k] = v;
    }
    glb(&gltf, bin)
}

/// A layer texture ready to sample: shrunk so one tile has about as many texels as the
/// baked image gives it, so distant tiles average out instead of shimmering.
struct Sampler {
    image: RgbaImage,
    tile: [f32; 2],
    offset: [f32; 2],
    tint: [f32; 3],
}

impl Sampler {
    /// The colour at terrain position (`x`, `z`) metres.
    fn at(&self, x: f32, z: f32) -> [f32; 3] {
        let (w, h) = self.image.dimensions();
        let u = ((x + self.offset[0]) / self.tile[0]).rem_euclid(1.0) * w as f32 - 0.5;
        // Image rows run top down; a tile's v runs bottom up.
        let v = (1.0 - ((z + self.offset[1]) / self.tile[1]).rem_euclid(1.0)) * h as f32 - 0.5;
        let c = bilinear(&self.image, u, v, true);
        [
            c[0] * self.tint[0],
            c[1] * self.tint[1],
            c[2] * self.tint[2],
        ]
    }
}

/// `image` at fractional pixel (`x`, `y`), wrapping or clamping at the edges; 0 to 255 per channel.
fn bilinear(image: &RgbaImage, x: f32, y: f32, wrap: bool) -> [f32; 4] {
    let (w, h) = (image.width() as i64, image.height() as i64);
    let fix = |i: i64, n: i64| if wrap { i.rem_euclid(n) } else { i.clamp(0, n - 1) } as u32;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let mut out = [0.0; 4];
    for (dx, dy, wgt) in [
        (0, 0, (1.0 - fx) * (1.0 - fy)),
        (1, 0, fx * (1.0 - fy)),
        (0, 1, (1.0 - fx) * fy),
        (1, 1, fx * fy),
    ] {
        let p: &Rgba<u8> = image.get_pixel(fix(x0 as i64 + dx, w), fix(y0 as i64 + dy, h));
        for (o, c) in out.iter_mut().zip(p.0) {
            *o += f32::from(c) * wgt;
        }
    }
    out
}

/// The terrain's painted layers blended into one `size` × `size` image, top row at the
/// terrain's far edge (z at its largest), for the mesh's UVs. `textures[i]` is layer i's
/// colour texture, decoded; a layer without one paints grey.
pub(crate) fn bake(t: &TerrainData, textures: &[Option<RgbaImage>], size: u32) -> RgbaImage {
    let per_metre = size as f32 / t.size[0].max(t.size[2]).max(f32::EPSILON);
    let samplers: Vec<Sampler> = t
        .layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let image = match textures.get(i).and_then(Option::as_ref) {
                Some(img) => {
                    let want = (layer.tile[0].max(layer.tile[1]) * per_metre)
                        .round()
                        .clamp(1.0, 1024.0) as u32;
                    if want < img.width().max(img.height()) {
                        image::imageops::resize(img, want, want, FilterType::Triangle)
                    } else {
                        img.clone()
                    }
                }
                None => RgbaImage::from_pixel(1, 1, Rgba([128, 128, 128, 255])),
            };
            Sampler {
                image,
                tile: [layer.tile[0].max(0.01), layer.tile[1].max(0.01)],
                offset: layer.offset,
                tint: layer.tint,
            }
        })
        .collect();
    let mut out = RgbaImage::new(size, size);
    out.par_chunks_mut(size as usize * 4)
        .enumerate()
        .for_each(|(py, row)| {
            let v = 1.0 - (py as f32 + 0.5) / size as f32;
            for px in 0..size as usize {
                let u = (px as f32 + 0.5) / size as f32;
                let (x, z) = (u * t.size[0], v * t.size[2]);
                let mut color = [0.0f32; 3];
                let mut total = 0.0;
                for (i, s) in samplers.iter().enumerate() {
                    let Some(alpha) = t.alpha.get(i / 4) else {
                        break;
                    };
                    // Alpha maps are stored bottom row first: row r is at v = r / height.
                    let a = bilinear(
                        alpha,
                        u * alpha.width() as f32 - 0.5,
                        v * alpha.height() as f32 - 0.5,
                        false,
                    )[i % 4]
                        / 255.0;
                    if a <= 0.0 {
                        continue;
                    }
                    let c = s.at(x, z);
                    for k in 0..3 {
                        color[k] += c[k] * a;
                    }
                    total += a;
                }
                if total <= 0.0 {
                    color = samplers.first().map_or([128.0; 3], |s| s.at(x, z));
                    total = 1.0;
                }
                let p = &mut row[px * 4..px * 4 + 4];
                for k in 0..3 {
                    p[k] = (color[k] / total).round().clamp(0.0, 255.0) as u8;
                }
                p[3] = 255;
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::super::terrain::Layer;
    use super::*;

    fn terrain(width: usize, heights: Vec<f32>) -> TerrainData {
        TerrainData {
            width,
            depth: heights.len() / width,
            size: [10.0, 100.0, 20.0],
            heights,
            layers: Vec::new(),
            alpha: Vec::new(),
            missing: Vec::new(),
        }
    }

    fn accessor_floats(glb: &[u8], ix: usize) -> Vec<f32> {
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let bin = &glb[20 + json_len + 8..];
        let a = &json["accessors"][ix];
        let view = &json["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        let off = view["byteOffset"].as_u64().unwrap() as usize;
        let len = view["byteLength"].as_u64().unwrap() as usize;
        bin[off..off + len]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    }

    #[test]
    fn draws_the_heightmap_mirrored_into_three_space_facing_up() {
        // 2 × 3 samples, rows along z; the far row is raised.
        let t = terrain(2, vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5]);
        let glb = terrain_glb(&t, "Island", None);
        assert_eq!(&glb[0..4], b"glTF");
        let pos = accessor_floats(&glb, 0);
        assert_eq!(pos.len(), 6 * 3);
        assert_eq!(&pos[0..3], [0.0, 0.0, 0.0]);
        assert_eq!(&pos[3..6], [-10.0, 0.0, 0.0], "x mirrored");
        assert_eq!(
            &pos[12..15],
            [0.0, 50.0, 20.0],
            "far row raised to half the height"
        );
        let normals = accessor_floats(&glb, 1);
        assert!(normals.chunks(3).all(|n| n[1] > 0.0), "normals point up");
        let uv = accessor_floats(&glb, 2);
        assert_eq!(&uv[0..2], [0.0, 1.0]);
        assert_eq!(&uv[10..12], [1.0, 0.0]);
    }

    #[test]
    fn thins_big_heightmaps_to_the_grid_limit() {
        let n = 1025;
        let t = terrain(n, vec![0.0; n * n]);
        let pos = accessor_floats(&terrain_glb(&t, "Big", None), 0);
        assert_eq!(pos.len() / 3, MAX_GRID * MAX_GRID);
        assert_eq!(pos[(MAX_GRID - 1) * 3], -10.0, "the last sample stays in");
    }

    #[test]
    fn bakes_layers_by_their_alpha_maps_and_tiling() {
        let mut t = terrain(2, vec![0.0; 4]);
        t.size = [10.0, 1.0, 10.0];
        let red = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        let blue = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
        t.layers = vec![
            Layer {
                texture: Some("r.png".into()),
                tile: [5.0, 5.0],
                offset: [0.0, 0.0],
                tint: [1.0; 3],
            },
            Layer {
                texture: Some("b.png".into()),
                tile: [5.0, 5.0],
                offset: [0.0, 0.0],
                tint: [1.0, 1.0, 0.5],
            },
        ];
        // Bottom row (near z = 0) all red, top row all blue.
        let mut alpha = RgbaImage::new(2, 2);
        for x in 0..2 {
            alpha.put_pixel(x, 0, Rgba([255, 0, 0, 0]));
            alpha.put_pixel(x, 1, Rgba([0, 255, 0, 0]));
        }
        t.alpha = vec![alpha];
        let out = bake(&t, &[Some(red), Some(blue)], 8);
        assert_eq!(
            out.get_pixel(0, 7).0,
            [255, 0, 0, 255],
            "near edge red (bottom of the image)"
        );
        assert_eq!(
            out.get_pixel(0, 0).0,
            [0, 0, 128, 255],
            "far edge blue, tinted"
        );
        let mid = out.get_pixel(4, 4).0;
        assert!(mid[0] > 0 && mid[2] > 0, "blended in between: {mid:?}");
    }
}
