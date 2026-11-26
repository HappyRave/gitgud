use image::{ImageEncoder, RgbaImage};

use super::guids::Guids;
use super::serialized::{SerializedFile, Value};
use super::terrain_glb::{bake, terrain_glb};
use super::tree::Files;
use super::yaml::{docs, obj_ref, vec2, vec3};
use crate::disk_cache::DiskCache;
use crate::image::Version;
use crate::repo::{Error, Repo};

/// Part of the kept GLB's key: bump it when the mesh or the bake changes.
const TERRAIN_VERSION: u32 = 1;
/// The baked layer texture's side when Settings › Previews asks for native size.
const NATIVE_BAKE: u32 = 2048;

const TERRAIN_DATA: i32 = 156;
const TERRAIN_LAYER: u32 = 1_953_259_897;
/// Unity stores heights as 16-bit values up to this, for the terrain's full height.
const MAX_HEIGHT: f32 = 32766.0;

/// One painted layer: its colour texture (a repo path), how it tiles, and its tint.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layer {
    pub texture: Option<String>,
    /// Metres one repeat of the texture covers, along x and z.
    pub tile: [f32; 2],
    pub offset: [f32; 2],
    pub tint: [f32; 3],
}

/// What a TerrainData asset holds that the 3D preview draws.
pub(crate) struct TerrainData {
    /// Heightmap samples along x and along z.
    pub width: usize,
    pub depth: usize,
    /// The terrain's size in metres: x, full height, z.
    pub size: [f32; 3],
    /// Heights from 0 to 1, `width` samples per row, rows along z.
    pub heights: Vec<f32>,
    pub layers: Vec<Layer>,
    /// Where each layer shows: four layers per image (RGBA), bottom row first.
    pub alpha: Vec<RgbaImage>,
    /// References that couldn't be followed (a layer or texture not in this version).
    pub missing: Vec<String>,
}

impl TerrainData {
    /// Height from 0 to 1 at sample (`x`, `z`).
    pub fn height(&self, x: usize, z: usize) -> f32 {
        self.heights[z * self.width + x]
    }
}

impl Repo {
    /// The TerrainData asset `path` at `version` as GLB: its heightmap as a mesh, its
    /// painted layers baked into a `max` px texture (0: native, 2048 px). Kept in the
    /// disk cache by the terrain's and its layer textures' content.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't in that version; [`Error::Read`] when it
    /// isn't a TerrainData this reader follows.
    pub fn terrain_glb(&self, path: &str, version: &Version, max: u32) -> Result<Vec<u8>, Error> {
        let files = Files::new(self, version)?;
        let guids = Guids::new(&files);
        let bytes = files
            .read(path)?
            .ok_or_else(|| Error::Refused(format!("{path} is not in that version")))?;
        let t = read_terrain(&bytes, &guids).map_err(|e| Error::Read(format!("{path}: {e}")))?;
        for m in &t.missing {
            eprintln!("gitgud: {path}: {m}");
        }
        let size = if max == 0 {
            NATIVE_BAKE
        } else {
            max.min(NATIVE_BAKE)
        };
        let disk = DiskCache::new(self);
        let key = files.key(path).map(|k| {
            let layers: Vec<String> = t
                .layers
                .iter()
                .map(|l| {
                    l.texture
                        .as_deref()
                        .and_then(|p| files.key(p))
                        .unwrap_or_default()
                })
                .collect();
            format!("terrain|{k}|{}|{size}|{TERRAIN_VERSION}", layers.join(","))
        });
        if let Some(kept) = key.as_deref().and_then(|k| disk.geometry(k)) {
            return Ok(kept);
        }
        let textures: Vec<Option<RgbaImage>> = t
            .layers
            .iter()
            .map(|l| {
                let p = l.texture.as_deref()?;
                match self.image_bytes(p, version).and_then(|b| {
                    image::load_from_memory(&b).map_err(|e| Error::Read(format!("{p}: {e}")))
                }) {
                    Ok(img) => Some(img.to_rgba8()),
                    Err(e) => {
                        eprintln!("gitgud: terrain layer texture {p} can't be read: {e}");
                        None
                    }
                }
            })
            .collect();
        let baked = bake(&t, &textures, size);
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut png,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(baked.as_raw(), size, size, image::ExtendedColorType::Rgba8)
        .map_err(|e| Error::Read(format!("{path}: can't encode the baked layers: {e}")))?;
        let name = super::tree::base_name(path).trim_end_matches(".asset");
        let out = terrain_glb(&t, name, Some(&png));
        if let Some(k) = key.as_deref() {
            disk.put_geometry(k, &out);
        }
        Ok(out)
    }
}

/// Reads a TerrainData asset (`bytes`, Unity binary), its layers resolved through `guids`.
///
/// # Errors
/// [`Error::Read`] when the file isn't a TerrainData Unity binary this reader follows.
pub(crate) fn read_terrain(bytes: &[u8], guids: &Guids<'_>) -> Result<TerrainData, Error> {
    let file = SerializedFile::parse(bytes)?;
    let object = file
        .first_of(TERRAIN_DATA)
        .ok_or_else(|| Error::Read("this file holds no TerrainData".into()))?;
    let v = file.read(object)?;
    let hm = v.get("m_Heightmap");
    let resolution = hm.get("m_Resolution").i64();
    let width = resolution.or_else(|| hm.get("m_Width").i64()).unwrap_or(0);
    let depth = resolution.or_else(|| hm.get("m_Height").i64()).unwrap_or(0);
    let (Ok(width), Ok(depth)) = (usize::try_from(width), usize::try_from(depth)) else {
        return Err(Error::Read("the heightmap has a negative size".into()));
    };
    let heights: Vec<f32> = hm
        .get("m_Heights")
        .numbers()
        .iter()
        .map(|h| *h as f32 / MAX_HEIGHT)
        .collect();
    if width < 2 || depth < 2 || heights.len() != width * depth {
        return Err(Error::Read(format!(
            "the heightmap is {width} × {depth} but holds {} heights",
            heights.len()
        )));
    }
    let scale = hm
        .get("m_Scale")
        .vec3()
        .ok_or_else(|| Error::Read("the heightmap has no scale".into()))?;
    let size = [
        scale[0] * (width - 1) as f32,
        scale[1],
        scale[2] * (depth - 1) as f32,
    ];
    let mut missing = Vec::new();
    let splat = v.get("m_SplatDatabase");
    let layers = layers(&file, splat, guids, &mut missing)?;
    let mut alpha = Vec::new();
    for p in splat.get("m_AlphaTextures").list() {
        let Some((0, id)) = p.pptr() else {
            continue;
        };
        match file.by_path_id(id).map(|o| file.read(o)) {
            Some(Ok(tex)) => alpha.push(texture(&tex)?),
            Some(Err(e)) => return Err(e),
            None => missing.push(format!("TerrainData › alpha map &{id} isn't in the file")),
        }
    }
    Ok(TerrainData {
        width,
        depth,
        size,
        heights,
        layers,
        alpha,
        missing,
    })
}

/// The painted layers: TerrainLayer assets (2018.3 and later), or the older splat prototypes.
fn layers(
    file: &SerializedFile<'_>,
    splat: &Value,
    guids: &Guids<'_>,
    missing: &mut Vec<String>,
) -> Result<Vec<Layer>, Error> {
    let external = |p: &Value| -> Option<String> {
        let (index, _) = p.pptr()?;
        Some(file.externals.get(index.checked_sub(1)?)?.guid.clone())
    };
    let mut out = Vec::new();
    let assets = splat.get("m_TerrainLayers").list();
    if !assets.is_empty() {
        for p in assets {
            let Some(guid) = external(p) else {
                out.push(blank());
                continue;
            };
            let Some(path) = guids.path(&guid)? else {
                missing.push(format!(
                    "TerrainData › TerrainLayer → guid {guid} (not in this version)"
                ));
                out.push(blank());
                continue;
            };
            out.push(terrain_layer(&path, guids, missing)?);
        }
        return Ok(out);
    }
    for s in splat.get("m_Splats").list() {
        let texture = match external(s.get("texture")) {
            Some(guid) => {
                let path = guids.path(&guid)?;
                if path.is_none() {
                    missing.push(format!(
                        "TerrainData › splat texture → guid {guid} (not in this version)"
                    ));
                }
                path
            }
            None => None,
        };
        let pair = |v: &Value, d: [f32; 2]| {
            [
                v.get("x").f64().map_or(d[0], |x| x as f32),
                v.get("y").f64().map_or(d[1], |y| y as f32),
            ]
        };
        out.push(Layer {
            texture,
            tile: pair(s.get("tileSize"), [15.0, 15.0]),
            offset: pair(s.get("tileOffset"), [0.0, 0.0]),
            tint: [1.0; 3],
        });
    }
    Ok(out)
}

fn blank() -> Layer {
    Layer {
        texture: None,
        tile: [15.0, 15.0],
        offset: [0.0, 0.0],
        tint: [1.0; 3],
    }
}

/// A `.terrainlayer` (YAML): its diffuse texture, tiling and tint.
fn terrain_layer(path: &str, guids: &Guids<'_>, missing: &mut Vec<String>) -> Result<Layer, Error> {
    let Some(text) = guids.files.read_text(path)? else {
        return Ok(blank());
    };
    let Some(doc) = docs(&text)
        .into_iter()
        .find(|d| d.class_id == TERRAIN_LAYER)
    else {
        missing.push(format!("{path} holds no TerrainLayer"));
        return Ok(blank());
    };
    let b = &doc.body;
    let texture = match obj_ref(&b["m_DiffuseTexture"]).and_then(|r| r.guid) {
        Some(guid) => {
            let p = guids.path(&guid)?;
            if p.is_none() {
                missing.push(format!(
                    "{path} › m_DiffuseTexture → guid {guid} (not in this version)"
                ));
            }
            p
        }
        None => None,
    };
    let tint = vec3(&b["m_DiffuseRemapMax"], [1.0; 3]);
    Ok(Layer {
        texture,
        tile: vec2(&b["m_TileSize"], [15.0, 15.0]),
        offset: vec2(&b["m_TileOffset"], [0.0, 0.0]),
        tint,
    })
}

/// An uncompressed Texture2D's first mip level as RGBA, bottom row first as Unity stores it.
fn texture(tex: &Value) -> Result<RgbaImage, Error> {
    let name = tex.get("m_Name").str().unwrap_or("texture").to_owned();
    let (Some(w), Some(h)) = (tex.get("m_Width").i64(), tex.get("m_Height").i64()) else {
        return Err(Error::Read(format!("{name} has no size")));
    };
    let (w, h) = (u32::try_from(w).unwrap_or(0), u32::try_from(h).unwrap_or(0));
    let data = tex.get("image data").bytes().unwrap_or_default();
    let n = w as usize * h as usize * 4;
    if data.len() < n {
        return Err(Error::Read(format!(
            "{name}'s pixels aren't in the file ({} of {n} bytes)",
            data.len()
        )));
    }
    let pixels: Vec<u8> = match tex.get("m_TextureFormat").i64() {
        Some(4) => data[..n].to_vec(),
        Some(5) => data[..n]
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[1], p[2], p[3], p[0]])
            .collect(),
        other => {
            return Err(Error::Read(format!(
                "{name} uses texture format {other:?}, which isn't read (RGBA32 and ARGB32 are)"
            )));
        }
    };
    RgbaImage::from_raw(w, h, pixels).ok_or_else(|| Error::Read(format!("{name} has a bad size")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::serialized::write::{Bytes, N, Object, Type, file};

    /// A 2 × 2 TerrainData (20 m × 10 m × 20 m) with one TerrainLayer (external 1) and
    /// one 1 × 1 alpha map showing that layer everywhere.
    fn terrain_data(layer_guid: &[u8; 16]) -> Vec<u8> {
        let data_type = Type {
            class_id: TERRAIN_DATA,
            nodes: vec![
                N("TerrainData", "Base", 0, false),
                N("string", "m_Name", 1, true),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("char", "data", 3, false),
                N("SplatDatabase", "m_SplatDatabase", 1, false),
                N("vector", "m_TerrainLayers", 2, false),
                N("Array", "Array", 3, false),
                N("int", "size", 4, false),
                N("PPtr<TerrainLayer>", "data", 4, false),
                N("int", "m_FileID", 5, false),
                N("SInt64", "m_PathID", 5, false),
                N("vector", "m_AlphaTextures", 2, false),
                N("Array", "Array", 3, false),
                N("int", "size", 4, false),
                N("PPtr<Texture2D>", "data", 4, false),
                N("int", "m_FileID", 5, false),
                N("SInt64", "m_PathID", 5, false),
                N("Heightmap", "m_Heightmap", 1, false),
                N("vector", "m_Heights", 2, true),
                N("Array", "Array", 3, false),
                N("int", "size", 4, false),
                N("SInt16", "data", 4, false),
                N("int", "m_Resolution", 2, false),
                N("Vector3f", "m_Scale", 2, false),
                N("float", "x", 3, false),
                N("float", "y", 3, false),
                N("float", "z", 3, false),
            ],
        };
        let texture_type = Type {
            class_id: 28,
            nodes: vec![
                N("Texture2D", "Base", 0, false),
                N("string", "m_Name", 1, true),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("char", "data", 3, false),
                N("int", "m_Width", 1, false),
                N("int", "m_Height", 1, false),
                N("int", "m_TextureFormat", 1, false),
                N("TypelessData", "image data", 1, true),
            ],
        };
        let data = Bytes::default()
            .string("Island")
            .i32(1)
            .i32(1)
            .i64(8_574_412_962_073_106_934)
            .i32(1)
            .i32(0)
            .i64(2_800_000)
            .i16s(&[0, 0, 32766, 32766])
            .align()
            .i32(2)
            .f32(20.0)
            .f32(10.0)
            .f32(20.0)
            .0;
        let alpha = Bytes::default()
            .string("SplatAlpha 0")
            .i32(1)
            .i32(1)
            .i32(4)
            .i32(4)
            .raw(&[255, 0, 0, 0])
            .0;
        file(
            22,
            &[data_type, texture_type],
            &[
                Object {
                    path_id: 2_800_000,
                    type_ix: 1,
                    data: alpha,
                },
                Object {
                    path_id: 15_600_000,
                    type_ix: 0,
                    data,
                },
            ],
            &[(layer_guid, "")],
        )
    }

    #[test]
    fn serves_a_terrain_as_glb_with_its_layer_baked_and_keeps_it() {
        // Unity writes each GUID byte's hex digits swapped: these bytes are guid "1234…".
        let raw = [0x21u8, 0x43, 0x65, 0x87, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let layer_guid = "12345678000000000000000000000000";
        let tex_guid = "abcd0000000000000000000000000000";
        let mut png = std::io::Cursor::new(Vec::new());
        RgbaImage::from_pixel(4, 4, image::Rgba([10, 200, 30, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let repo = crate::testing::TestRepo::init();
        std::fs::write(repo.path().join("Island.asset"), terrain_data(&raw)).unwrap();
        std::fs::write(repo.path().join("grass.png"), png.get_ref()).unwrap();
        repo.write("Island.asset.meta", "guid: 99990000000000000000000000000000\n")
            .write("grass.png.meta", &format!("guid: {tex_guid}\n"))
            .write(
                "Grass.terrainlayer",
                &format!("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1953259897 &8574412962073106934\nTerrainLayer:\n  m_Name: Grass\n  m_DiffuseTexture: {{fileID: 2800000, guid: {tex_guid}, type: 3}}\n  m_TileSize: {{x: 5, y: 5}}\n  m_TileOffset: {{x: 0, y: 0}}\n"),
            )
            .write("Grass.terrainlayer.meta", &format!("guid: {layer_guid}\n"))
            .commit_all("terrain");
        let r = crate::repo::open(repo.path()).unwrap();

        let glb = r.terrain_glb("Island.asset", &Version::Head, 64).unwrap();
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert_eq!(json["accessors"][0]["count"], 4, "2 × 2 heightmap");
        assert_eq!(
            json["accessors"][0]["max"][1], 10.0,
            "the far row at full height"
        );
        assert_eq!(json["images"][0]["mimeType"], "image/png");
        let bin = &glb[20 + json_len + 8..];
        let view = &json["bufferViews"][json["images"][0]["bufferView"].as_u64().unwrap() as usize];
        let off = view["byteOffset"].as_u64().unwrap() as usize;
        let baked =
            image::load_from_memory(&bin[off..off + view["byteLength"].as_u64().unwrap() as usize])
                .unwrap()
                .to_rgba8();
        assert_eq!(baked.dimensions(), (64, 64));
        assert_eq!(
            baked.get_pixel(30, 30).0,
            [10, 200, 30, 255],
            "the grass layer everywhere"
        );

        let disk = DiskCache::new(&r);
        assert_eq!(disk.stats().geometry, 1, "kept");
        assert_eq!(
            r.terrain_glb("Island.asset", &Version::Head, 64).unwrap(),
            glb
        );
        assert!(
            r.terrain_glb("grass.png", &Version::Head, 64).is_err(),
            "not a TerrainData"
        );
    }

    /// Writes the baked texture of `GITGUD_TERRAIN_PATH` in repo `GITGUD_TERRAIN_REPO`
    /// (work tree) to `GITGUD_TERRAIN_OUT`, to compare with Unity's base map by eye.
    #[test]
    #[ignore = "reads GITGUD_TERRAIN_REPO / _PATH / _OUT"]
    fn bake_a_real_terrain() {
        let repo = crate::repo::open(std::path::Path::new(
            &std::env::var("GITGUD_TERRAIN_REPO").unwrap(),
        ))
        .unwrap();
        let path = std::env::var("GITGUD_TERRAIN_PATH").unwrap();
        let t0 = std::time::Instant::now();
        let glb = repo.terrain_glb(&path, &Version::Worktree, 1024).unwrap();
        println!("glb {} bytes in {:?}", glb.len(), t0.elapsed());
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let bin = &glb[20 + json_len + 8..];
        let view = &json["bufferViews"][json["images"][0]["bufferView"].as_u64().unwrap() as usize];
        let off = view["byteOffset"].as_u64().unwrap() as usize;
        let len = view["byteLength"].as_u64().unwrap() as usize;
        std::fs::write(
            std::env::var("GITGUD_TERRAIN_OUT").unwrap(),
            &bin[off..off + len],
        )
        .unwrap();
    }

    /// Compares a TerrainData's heightmap against the same terrain converted to a mesh
    /// (VacuumShaders' Terrain To Mesh), in both index orders.
    #[test]
    #[ignore = "reads GITGUD_TERRAIN and GITGUD_TERRAIN_MESH"]
    fn heightmap_order_matches_a_converted_mesh() {
        let td = std::fs::read(std::env::var("GITGUD_TERRAIN").unwrap()).unwrap();
        let mesh = std::fs::read_to_string(std::env::var("GITGUD_TERRAIN_MESH").unwrap()).unwrap();
        let file = SerializedFile::parse(&td).unwrap();
        let v = file.read(file.first_of(TERRAIN_DATA).unwrap()).unwrap();
        let hm = v.get("m_Heightmap");
        let w = hm
            .get("m_Resolution")
            .i64()
            .or(hm.get("m_Width").i64())
            .unwrap() as usize;
        let heights: Vec<f32> = hm
            .get("m_Heights")
            .numbers()
            .iter()
            .map(|h| *h as f32 / MAX_HEIGHT)
            .collect();
        let glb = super::super::mesh_asset::mesh_asset_to_glb(&mesh).unwrap();
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let bin = &glb[20 + json_len + 8..];
        let acc = &json["accessors"][0];
        let view = &json["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
        let off = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let n = acc["count"].as_u64().unwrap() as usize;
        let pos: Vec<[f32; 3]> = (0..n)
            .map(|i| {
                let f = |k: usize| {
                    f32::from_le_bytes(
                        bin[off + i * 12 + k * 4..off + i * 12 + k * 4 + 4]
                            .try_into()
                            .unwrap(),
                    )
                };
                [-f(0), f(1), f(2)]
            })
            .collect();
        let (lo, hi) = pos
            .iter()
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(mut lo, mut hi), p| {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
                (lo, hi)
            });
        let hmax = heights.iter().copied().fold(f32::MIN, f32::max);
        let hmin = heights.iter().copied().fold(f32::MAX, f32::min);
        let err = |row_major_z: bool| {
            pos.iter()
                .map(|p| {
                    let x = ((p[0] - lo[0]) / (hi[0] - lo[0]) * (w - 1) as f32).round() as usize;
                    let z = ((p[2] - lo[2]) / (hi[2] - lo[2]) * (w - 1) as f32).round() as usize;
                    let h = if row_major_z {
                        heights[z * w + x]
                    } else {
                        heights[x * w + z]
                    };
                    ((p[1] - lo[1]) / (hi[1] - lo[1]) - (h - hmin) / (hmax - hmin)).abs()
                })
                .sum::<f32>()
                / pos.len() as f32
        };
        if let Ok(png) = std::env::var("GITGUD_TERRAIN_ALPHA") {
            let theirs = image::open(png).unwrap().to_rgba8();
            let splat = v.get("m_SplatDatabase");
            let (_, id) = splat.get("m_AlphaTextures").list()[0].pptr().unwrap();
            let ours = texture(&file.read(file.by_path_id(id).unwrap()).unwrap()).unwrap();
            let (aw, ah) = ours.dimensions();
            println!("alpha {aw}x{ah}, exported {:?}", theirs.dimensions());
            let diff = |f: &dyn Fn(u32, u32) -> (u32, u32)| {
                let mut sum = 0u64;
                for y in 0..ah {
                    for x in 0..aw {
                        let (sx, sy) = f(x, y);
                        let a = ours.get_pixel(sx, sy).0;
                        let b = theirs.get_pixel(x, y).0;
                        sum += (0..4).map(|k| u64::from(a[k].abs_diff(b[k]))).sum::<u64>();
                    }
                }
                sum as f64 / f64::from(aw * ah * 4)
            };
            println!("alpha diff as stored: {}", diff(&|x, y| (x, y)));
            println!(
                "alpha diff flipped (png top = last row): {}",
                diff(&|x, y| (x, ah - 1 - y))
            );
            println!(
                "alpha diff transposed + flipped: {}",
                diff(&|x, y| (ah - 1 - y, x))
            );
        }
        println!("resolution {w}, mesh vertices {n}");
        println!("mean error, rows along z (index z*w+x): {}", err(true));
        println!("mean error, rows along x (index x*w+z): {}", err(false));
    }
}
