use serde::{Deserialize, Serialize};

use super::guids::Guids;
use super::tree::base_name;
use super::yaml::{color, docs, keyed_list, num, obj_ref, scalar, vec2};
use crate::image::image_type;
use crate::repo::Error;

const MATERIAL_CLASS: u32 = 21;
const BUILTIN_EXTRA: &str = "0000000000000000f000000000000000";

/// Shaders the viewer's standard material renders faithfully, by GUID.
const KNOWN_SHADERS: [(&str, &str); 5] = [
    (
        "933532a4fcc9baf4fa0491de14d08ed7",
        "Universal Render Pipeline/Lit",
    ),
    (
        "8d2bb70cbf9db8d4da26e15b26e74248",
        "Universal Render Pipeline/Simple Lit",
    ),
    (
        "650dd9526735d5b46b79224bc6e94025",
        "Universal Render Pipeline/Unlit",
    ),
    ("6e4ae4064600d784cac1e41a9e6f2e59", "HDRP/Lit"),
    ("c4edd00ff2db5b24391a4fcb1762e459", "HDRP/Unlit"),
];
/// Built-in shaders (in `unity_builtin_extra`) by `fileID`.
const BUILTIN_SHADERS: [(&str, &str); 4] = [
    ("46", "Standard"),
    ("45", "Standard (Specular setup)"),
    ("7", "Legacy Shaders/Diffuse"),
    ("10755", "Unlit/Texture"),
];

/// A Unity material reduced to what a standard PBR material can show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub name: String,
    /// Repo path of the `.mat`, if it came from one.
    pub path: Option<String>,
    pub shader: String,
    /// The shader isn't one the viewer renders faithfully.
    pub approximated: bool,
    pub unlit: bool,
    pub color: [f32; 4],
    pub base_map: Option<String>,
    pub normal_map: Option<String>,
    /// Names for the maps' contents at this version (see `Files::key`), for reusing loaded textures.
    pub base_map_key: Option<String>,
    pub normal_map_key: Option<String>,
    pub tiling: [f32; 2],
    pub offset: [f32; 2],
    pub metallic: f32,
    pub smoothness: f32,
    /// The referenced `.mat` or its shader couldn't be found.
    pub missing: bool,
    /// For an approximated shader, every texture it assigns, so the viewer can
    /// offer which one to show as the base map.
    pub textures: Vec<TextureSlot>,
    /// The property `base_map` came from.
    pub base_property: Option<String>,
    /// For an approximated shader, the texture properties that look like a colour
    /// (likeliest first, the base map's included), for the viewer's base map choice.
    pub base_candidates: Vec<String>,
}

/// A texture property of a material and the image it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureSlot {
    pub property: String,
    pub path: String,
    pub key: Option<String>,
    pub tiling: [f32; 2],
    pub offset: [f32; 2],
}

impl Material {
    pub(crate) fn missing(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            path: None,
            shader: String::new(),
            approximated: false,
            unlit: true,
            color: [1.0, 0.0, 1.0, 1.0],
            base_map: None,
            normal_map: None,
            base_map_key: None,
            normal_map_key: None,
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            metallic: 0.0,
            smoothness: 0.0,
            missing: true,
            textures: Vec::new(),
            base_property: None,
            base_candidates: Vec::new(),
        }
    }

    /// Reads the `.mat` at `path`; a file that isn't there gives a missing material.
    pub(crate) fn load(path: &str, guids: &Guids<'_>) -> Result<Self, Error> {
        let stem = base_name(path).trim_end_matches(".mat").to_owned();
        let Some(text) = guids.files.read_text(path)? else {
            return Ok(Self::missing(&stem));
        };
        let Some(doc) = docs(&text)
            .into_iter()
            .find(|d| d.class_id == MATERIAL_CLASS)
        else {
            return Ok(Self::missing(&stem));
        };
        let b = &doc.body;
        let (shader, approximated) = shader_name(b, guids)?;
        let props = &b["m_SavedProperties"];
        let mut m = Self {
            name: scalar(&b["m_Name"])
                .filter(|n| !n.is_empty())
                .unwrap_or(stem),
            path: Some(path.to_owned()),
            unlit: shader.contains("Unlit"),
            shader,
            approximated,
            color: [1.0; 4],
            base_map: None,
            normal_map: None,
            base_map_key: None,
            normal_map_key: None,
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            metallic: 0.0,
            smoothness: 0.5,
            missing: false,
            textures: Vec::new(),
            base_property: None,
            base_candidates: Vec::new(),
        };
        for (key, tex) in keyed_list(&props["m_TexEnvs"]) {
            let Some(guid) = obj_ref(&tex["m_Texture"]).and_then(|r| r.guid) else {
                continue;
            };
            let Some(p) = guids.path(&guid)?.filter(|p| image_type(p).is_some()) else {
                continue;
            };
            let slot = TextureSlot {
                key: guids.files.key(&p),
                property: key.clone(),
                path: p,
                tiling: vec2(&tex["m_Scale"], [1.0, 1.0]),
                offset: vec2(&tex["m_Offset"], [0.0, 0.0]),
            };
            match key.as_str() {
                "_BaseMap" | "_MainTex" | "_BaseColorMap" if m.base_map.is_none() => {
                    m.use_base(&slot)
                }
                "_BumpMap" | "_NormalMap" if m.normal_map.is_none() => {
                    m.normal_map = Some(slot.path.clone());
                }
                _ => {}
            }
            if approximated {
                m.textures.push(slot);
            }
        }
        if m.base_map.is_none() && !m.textures.is_empty() {
            let main = shader_properties(b, guids)?
                .and_then(|props| props.into_iter().find(|p| p.main_texture))
                .map(|p| p.name);
            let pick = main
                .and_then(|name| m.textures.iter().find(|t| t.property == name))
                .or_else(|| {
                    m.textures
                        .iter()
                        .filter(|t| Some(&t.path) != m.normal_map.as_ref())
                        .min_by_key(|t| colour_rank(&t.property))
                })
                .cloned();
            if let Some(slot) = pick {
                m.use_base(&slot);
            }
        }
        if approximated {
            m.base_candidates = base_candidates(&m);
        }
        for (key, v) in keyed_list(&props["m_Floats"]) {
            match key.as_str() {
                "_Metallic" => m.metallic = num(v).unwrap_or(m.metallic),
                "_Smoothness" | "_Glossiness" => m.smoothness = num(v).unwrap_or(m.smoothness),
                _ => {}
            }
        }
        let colors = keyed_list(&props["m_Colors"]);
        for want in ["_BaseColor", "_Color"] {
            if let Some((_, v)) = colors.iter().find(|(k, _)| k == want) {
                m.color = color(v);
                break;
            }
        }
        m.normal_map_key = m.normal_map.as_deref().and_then(|p| guids.files.key(p));
        Ok(m)
    }

    fn use_base(&mut self, slot: &TextureSlot) {
        self.base_map = Some(slot.path.clone());
        self.base_map_key.clone_from(&slot.key);
        self.tiling = slot.tiling;
        self.offset = slot.offset;
        self.base_property = Some(slot.property.clone());
    }
}

/// How likely a custom shader's texture property is its colour: 0 named like one,
/// 2 named like data (normals, masks, noise, ...), 1 anything else.
fn colour_rank(property: &str) -> u8 {
    const COLOUR: &[&str] = &[
        "albedo",
        "basecolor",
        "base_color",
        "diffuse",
        "maintex",
        "color",
        "colour",
    ];
    const DATA: &[&str] = &[
        "normal",
        "bump",
        "mask",
        "noise",
        "height",
        "rough",
        "metal",
        "smooth",
        "occlusion",
        "emiss",
        "splat",
        "control",
        "flow",
        "ramp",
        "lut",
        "matcap",
        "cube",
        "dissolve",
        "displace",
        "distort",
        "spec",
        "gloss",
    ];
    let p = property.to_ascii_lowercase();
    if DATA.iter().any(|w| p.contains(w)) {
        2
    } else if COLOUR.iter().any(|w| p.contains(w)) {
        0
    } else {
        1
    }
}

/// `m`'s texture properties named like a colour or neutrally, likeliest first, with
/// its base map's property always in.
fn base_candidates(m: &Material) -> Vec<String> {
    let mut slots: Vec<_> = m
        .textures
        .iter()
        .filter(|t| Some(&t.path) != m.normal_map.as_ref())
        .map(|t| (colour_rank(&t.property), &t.property))
        .filter(|(rank, _)| *rank < 2)
        .collect();
    slots.sort_by_key(|(rank, _)| *rank);
    let mut out: Vec<String> = slots.into_iter().map(|(_, p)| p.clone()).collect();
    if let Some(base) = &m.base_property
        && !out.contains(base)
    {
        out.insert(0, base.clone());
    }
    out
}

/// Everything a `.mat` sets, for the material viewer's table.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialView {
    /// What the 3D stage renders.
    pub material: Material,
    pub properties: Vec<MaterialProperty>,
    pub keywords: Vec<String>,
    /// `m_CustomRenderQueue`; -1 means the shader's own.
    pub render_queue: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialProperty {
    /// The shader's name for it (`_BaseMap`).
    pub key: String,
    /// The Inspector's label for known shaders, else `key`.
    pub label: String,
    pub value: String,
    /// The colour of a colour property, for a swatch.
    pub color: Option<[f32; 4]>,
    /// Left over from an earlier shader: the current shader doesn't declare it.
    pub stale: bool,
}

/// Inspector labels for the properties of Unity's built-in, URP and HDRP shaders.
const LABELS: [(&str, &str); 34] = [
    ("_BaseMap", "Base Map"),
    ("_BaseColor", "Base Color"),
    ("_BaseColorMap", "Base Map"),
    ("_MainTex", "Albedo"),
    ("_Color", "Color"),
    ("_BumpMap", "Normal Map"),
    ("_BumpScale", "Normal Scale"),
    ("_NormalMap", "Normal Map"),
    ("_NormalScale", "Normal Scale"),
    ("_Metallic", "Metallic"),
    ("_MetallicGlossMap", "Metallic Map"),
    ("_MaskMap", "Mask Map"),
    ("_Smoothness", "Smoothness"),
    ("_Glossiness", "Smoothness"),
    ("_GlossMapScale", "Smoothness Scale"),
    ("_SpecColor", "Specular Color"),
    ("_SpecGlossMap", "Specular Map"),
    ("_OcclusionMap", "Occlusion Map"),
    ("_OcclusionStrength", "Occlusion"),
    ("_EmissionColor", "Emission Color"),
    ("_EmissionMap", "Emission Map"),
    ("_EmissiveColor", "Emission Color"),
    ("_EmissiveColorMap", "Emission Map"),
    ("_ParallaxMap", "Height Map"),
    ("_Parallax", "Height Scale"),
    ("_DetailAlbedoMap", "Detail Albedo"),
    ("_DetailNormalMap", "Detail Normal"),
    ("_DetailMask", "Detail Mask"),
    ("_Cutoff", "Alpha Cutoff"),
    ("_AlphaClip", "Alpha Clipping"),
    ("_Surface", "Surface Type"),
    ("_Blend", "Blending Mode"),
    ("_Cull", "Render Face"),
    ("_WorkflowMode", "Workflow"),
];

impl MaterialView {
    /// Reads every property of the `.mat` at `path`.
    ///
    /// # Errors
    /// [`Error::Refused`] when the file isn't in that version or holds no material.
    pub(crate) fn load(path: &str, guids: &Guids<'_>) -> Result<Self, Error> {
        let not_found = || Error::Refused(format!("{path} has no material in that version"));
        let text = guids.files.read_text(path)?.ok_or_else(not_found)?;
        let doc = docs(&text)
            .into_iter()
            .find(|d| d.class_id == MATERIAL_CLASS)
            .ok_or_else(not_found)?;
        let material = Material::load(path, guids)?;
        let b = &doc.body;
        let known: Option<Vec<String>> = material
            .approximated
            .then(|| shader_properties(b, guids))
            .transpose()?
            .flatten()
            .map(|props| props.into_iter().map(|p| p.name).collect());
        let props = &b["m_SavedProperties"];
        let mut properties = Vec::new();
        let mut add = |key: String, value: String, color: Option<[f32; 4]>| {
            let label = LABELS
                .iter()
                .find(|(k, _)| *k == key)
                .map_or_else(|| key.clone(), |(_, l)| (*l).to_owned());
            let stale = known.as_ref().is_some_and(|k| !k.contains(&key));
            properties.push(MaterialProperty {
                key,
                label,
                value,
                color,
                stale,
            });
        };
        for (key, tex) in keyed_list(&props["m_TexEnvs"]) {
            let name = match obj_ref(&tex["m_Texture"]).and_then(|r| r.guid) {
                Some(g) => guids
                    .path(&g)?
                    .map_or_else(|| format!("missing ({g})"), |p| base_name(&p).to_owned()),
                None => "none".to_owned(),
            };
            let scale = vec2(&tex["m_Scale"], [1.0, 1.0]);
            let offset = vec2(&tex["m_Offset"], [0.0, 0.0]);
            let mut value = name;
            if scale != [1.0, 1.0] || offset != [0.0, 0.0] {
                value.push_str(&format!(
                    " · tiling {}, {} · offset {}, {}",
                    scale[0], scale[1], offset[0], offset[1]
                ));
            }
            add(key, value, None);
        }
        for list in ["m_Ints", "m_Floats"] {
            for (key, v) in keyed_list(&props[list]) {
                add(key, scalar(v).unwrap_or_default(), None);
            }
        }
        for (key, v) in keyed_list(&props["m_Colors"]) {
            let c = color(v);
            add(key, hex(c), Some(c));
        }
        let mut keywords: Vec<String> = match b["m_ValidKeywords"].as_vec() {
            Some(list) => list.iter().filter_map(scalar).collect(),
            None => scalar(&b["m_ShaderKeywords"])
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        };
        keywords.sort();
        Ok(Self {
            material,
            properties,
            keywords,
            render_queue: num(&b["m_CustomRenderQueue"]).map(|q| q as i32),
        })
    }
}

/// `#RRGGBB`, or `#RRGGBBAA` when not opaque; HDR values above 1 show as floats.
fn hex(c: [f32; 4]) -> String {
    if c.iter().any(|v| *v > 1.0 || *v < 0.0) {
        return format!("({}, {}, {}, {})", c[0], c[1], c[2], c[3]);
    }
    let byte = |v: f32| (v * 255.0).round() as u8;
    let rgb = format!("#{:02X}{:02X}{:02X}", byte(c[0]), byte(c[1]), byte(c[2]));
    if c[3] < 1.0 {
        format!("{rgb}{:02X}", byte(c[3]))
    } else {
        rgb
    }
}

/// A property a shader's `Properties` block declares.
struct ShaderProperty {
    name: String,
    /// Marked `[MainTexture]`: what Unity treats as the material's main texture.
    main_texture: bool,
}

/// The properties a custom shader's source declares, when its source is in the repo.
fn shader_properties(
    material: &yaml_rust2::Yaml,
    guids: &Guids<'_>,
) -> Result<Option<Vec<ShaderProperty>>, Error> {
    let Some(guid) = obj_ref(&material["m_Shader"]).and_then(|r| r.guid) else {
        return Ok(None);
    };
    let Some(path) = guids.path(&guid)?.filter(|p| p.ends_with(".shader")) else {
        return Ok(None);
    };
    let Some(text) = guids.files.read_text(&path)? else {
        return Ok(None);
    };
    let Some(start) = text.find("Properties") else {
        return Ok(None);
    };
    let block = &text[start..];
    let Some(open) = block.find('{') else {
        return Ok(None);
    };
    let mut depth = 0;
    let mut end = block.len();
    for (i, ch) in block[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let props = block[open + 1..end]
        .lines()
        .filter_map(|l| {
            let mut l = l.trim();
            let mut main_texture = false;
            while let Some(rest) = l.strip_prefix('[') {
                let (attr, after) = rest.split_once(']').unwrap_or(("", ""));
                main_texture |= attr.trim() == "MainTexture";
                l = after.trim_start();
            }
            let name: String = l
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty() && l[name.len()..].trim_start().starts_with('('))
                .then_some(ShaderProperty { name, main_texture })
        })
        .collect();
    Ok(Some(props))
}

fn shader_name(material: &yaml_rust2::Yaml, guids: &Guids<'_>) -> Result<(String, bool), Error> {
    let Some(r) = obj_ref(&material["m_Shader"]) else {
        return Ok(("(no shader)".into(), true));
    };
    let guid = r.guid.unwrap_or_default().to_ascii_lowercase();
    if guid == BUILTIN_EXTRA {
        return Ok(BUILTIN_SHADERS
            .iter()
            .find(|(id, _)| *id == r.file_id)
            .map_or_else(
                || (format!("Built-in shader {}", r.file_id), true),
                |(_, n)| ((*n).to_owned(), false),
            ));
    }
    if let Some((_, n)) = KNOWN_SHADERS.iter().find(|(g, _)| *g == guid) {
        return Ok(((*n).to_owned(), false));
    }
    let Some(path) = guids.path(&guid)? else {
        return Ok(("(missing shader)".into(), true));
    };
    if let Some(text) = guids.files.read_text(&path)?
        && let Some(name) = text.lines().find_map(|l| {
            let rest = l.trim().strip_prefix("Shader")?.trim();
            rest.strip_prefix('"')?.split('"').next().map(str::to_owned)
        })
    {
        return Ok((name, true));
    }
    Ok((base_name(&path).to_owned(), true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Version;
    use crate::model::tree::Files;
    use crate::testing::TestRepo;

    const TEX: &str = "1111aaaa000000000000000000000000";
    const SHADER: &str = "2222bbbb000000000000000000000000";

    fn mat(shader: &str) -> String {
        format!(
            "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!21 &2100000\nMaterial:\n  m_Name: Hull\n  m_Shader: {shader}\n  m_SavedProperties:\n    m_TexEnvs:\n    - _BaseMap:\n        m_Texture: {{fileID: 2800000, guid: {TEX}, type: 3}}\n        m_Scale: {{x: 2, y: 3}}\n        m_Offset: {{x: 0.5, y: 0}}\n    - _BumpMap:\n        m_Texture: {{fileID: 0}}\n    m_Floats:\n    - _Metallic: 0.25\n    - _Smoothness: 0.75\n    m_Colors:\n    - _BaseColor: {{r: 0.2, g: 0.4, b: 0.6, a: 1}}\n"
        )
    }

    #[test]
    fn reads_urp_lit_and_flags_custom_shaders() {
        let repo = TestRepo::init();
        repo.write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.1f1\n",
        )
        .write("Assets/T/hull.png", "x")
        .write("Assets/T/hull.png.meta", &format!("guid: {TEX}\n"))
        .write("Assets/S/Hull.shader", "Shader \"BoatAttack/Hull\"\n{\n}\n")
        .write("Assets/S/Hull.shader.meta", &format!("guid: {SHADER}\n"))
        .write(
            "Assets/M/urp.mat",
            &mat("{fileID: 4800000, guid: 933532a4fcc9baf4fa0491de14d08ed7, type: 3}"),
        )
        .write(
            "Assets/M/custom.mat",
            &mat(&format!("{{fileID: 4800000, guid: {SHADER}, type: 3}}")),
        )
        .write(
            "Assets/M/std.mat",
            &mat("{fileID: 46, guid: 0000000000000000f000000000000000, type: 0}"),
        )
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let v = Version::Head;
        let files = Files::new(&r, &v).unwrap();
        let g = Guids::new(&files);

        let m = Material::load("Assets/M/urp.mat", &g).unwrap();
        assert_eq!(m.name, "Hull");
        assert_eq!(m.shader, "Universal Render Pipeline/Lit");
        assert!(!m.approximated && !m.missing && !m.unlit);
        assert_eq!(m.base_map.as_deref(), Some("Assets/T/hull.png"));
        assert_eq!(m.base_map_key, files.key("Assets/T/hull.png"));
        assert!(m.base_map_key.is_some());
        assert_eq!(m.normal_map, None);
        assert_eq!((m.tiling, m.offset), ([2.0, 3.0], [0.5, 0.0]));
        assert_eq!((m.metallic, m.smoothness), (0.25, 0.75));
        assert_eq!(m.color, [0.2, 0.4, 0.6, 1.0]);

        let c = Material::load("Assets/M/custom.mat", &g).unwrap();
        assert_eq!(c.shader, "BoatAttack/Hull");
        assert!(c.approximated);

        assert_eq!(
            Material::load("Assets/M/std.mat", &g).unwrap().shader,
            "Standard"
        );
        let gone = Material::load("Assets/M/gone.mat", &g).unwrap();
        assert!(gone.missing);
        assert_eq!(gone.name, "gone");
    }

    #[test]
    fn a_custom_shader_without_a_standard_slot_offers_its_textures() {
        const MASK: &str = "3333cccc000000000000000000000000";
        let mat = format!(
            "--- !u!21 &2100000\nMaterial:\n  m_Name: Mozaic\n  m_Shader: {{fileID: 4800000, guid: {SHADER}, type: 3}}\n  m_SavedProperties:\n    m_TexEnvs:\n    - _Mask:\n        m_Texture: {{fileID: 2800000, guid: {MASK}, type: 3}}\n        m_Scale: {{x: 1, y: 1}}\n        m_Offset: {{x: 0, y: 0}}\n    - _Drawing:\n        m_Texture: {{fileID: 2800000, guid: {TEX}, type: 3}}\n        m_Scale: {{x: 2, y: 2}}\n        m_Offset: {{x: 0, y: 0}}\n    - _MainTex:\n        m_Texture: {{fileID: 0}}\n"
        );
        let shader = |drawing_attr: &str| {
            format!(
                "Shader \"Mozaic\"\n{{\n  Properties\n  {{\n    _Mask(\"Mask\", 2D) = \"white\" {{}}\n    {drawing_attr}_Drawing(\"Drawing\", 2D) = \"white\" {{}}\n  }}\n}}\n"
            )
        };
        let repo = TestRepo::init();
        repo.write("Assets/T/drawing.png", "x")
            .write("Assets/T/drawing.png.meta", &format!("guid: {TEX}\n"))
            .write("Assets/T/mask.png", "x")
            .write("Assets/T/mask.png.meta", &format!("guid: {MASK}\n"))
            .write("Assets/S/Mozaic.shader", &shader(""))
            .write("Assets/S/Mozaic.shader.meta", &format!("guid: {SHADER}\n"))
            .write("Assets/M/mozaic.mat", &mat)
            .commit_all("one");
        repo.write("Assets/S/Mozaic.shader", &shader("[MainTexture] "));
        let r = crate::repo::open(repo.path()).unwrap();
        let load = |v: Version| {
            let files = Files::new(&r, &v).unwrap();
            Material::load("Assets/M/mozaic.mat", &Guids::new(&files)).unwrap()
        };

        let head = load(Version::Head);
        let props: Vec<_> = head.textures.iter().map(|t| t.property.as_str()).collect();
        assert_eq!(props, ["_Mask", "_Drawing"]);
        assert_eq!(
            head.base_property.as_deref(),
            Some("_Drawing"),
            "a mask isn't a colour"
        );
        assert_eq!(head.base_map.as_deref(), Some("Assets/T/drawing.png"));

        let marked = load(Version::Worktree);
        assert_eq!(
            marked.base_property.as_deref(),
            Some("_Drawing"),
            "[MainTexture] wins"
        );
        assert_eq!(marked.tiling, [2.0, 2.0]);
    }

    #[test]
    fn a_custom_shader_s_base_map_skips_masks_normals_and_noise_and_prefers_colour_names() {
        let slots = |props: &[&str]| {
            props
                .iter()
                .enumerate()
                .map(|(i, p)| format!("    - {p}:\n        m_Texture: {{fileID: 2800000, guid: {:032x}, type: 3}}\n        m_Scale: {{x: 1, y: 1}}\n        m_Offset: {{x: 0, y: 0}}\n", 0xabc0 + i))
                .collect::<String>()
        };
        let mat = |props: &[&str]| {
            format!(
                "--- !u!21 &2100000\nMaterial:\n  m_Name: Ground\n  m_Shader: {{fileID: 4800000, guid: {SHADER}, type: 3}}\n  m_SavedProperties:\n    m_TexEnvs:\n{}",
                slots(props)
            )
        };
        let repo = TestRepo::init();
        repo.write("Assets/S/Ground.shader", "Shader \"Custom/Ground\"\n{\n}\n")
            .write("Assets/S/Ground.shader.meta", &format!("guid: {SHADER}\n"));
        for i in 0..5 {
            repo.write(&format!("Assets/T/{i}.png"), "x").write(
                &format!("Assets/T/{i}.png.meta"),
                &format!("guid: {:032x}\n", 0xabc0 + i),
            );
        }
        repo.write(
            "Assets/M/a.mat",
            &mat(&[
                "_EdgeNoise",
                "_DirtNormal",
                "_SplatMask",
                "_Grass",
                "_AlbedoMap",
            ]),
        )
        .write(
            "Assets/M/b.mat",
            &mat(&["_EdgeNoise", "_DirtNormal", "_SpecGlossMap", "_Grass"]),
        )
        .write("Assets/M/c.mat", &mat(&["_Noise", "_HeightMap"]))
        .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let files = Files::new(&r, &Version::Head).unwrap();
        let g = Guids::new(&files);
        let load = |p: &str| Material::load(p, &g).unwrap();
        let base = |p: &str| load(p).base_property;

        assert_eq!(base("Assets/M/a.mat").as_deref(), Some("_AlbedoMap"));
        assert_eq!(base("Assets/M/b.mat").as_deref(), Some("_Grass"));
        assert_eq!(
            base("Assets/M/c.mat").as_deref(),
            Some("_Noise"),
            "nothing better: the first one"
        );
        assert_eq!(
            load("Assets/M/a.mat").base_candidates,
            ["_AlbedoMap", "_Grass"]
        );
        assert_eq!(load("Assets/M/c.mat").base_candidates, ["_Noise"]);
    }

    #[test]
    fn lists_every_property_with_labels_keywords_and_stale_marks() {
        let repo = TestRepo::init();
        let body = format!(
            "{}    m_Ints:\n    - _Cull: 2\n  m_ValidKeywords:\n  - _NORMALMAP\n  - _ALPHATEST_ON\n  m_CustomRenderQueue: 2450\n",
            mat(&format!("{{fileID: 4800000, guid: {SHADER}, type: 3}}"))
                .replace(
                    "    - _BaseColor: {r: 0.2, g: 0.4, b: 0.6, a: 1}\n",
                    "    - _BaseColor: {r: 1, g: 0.5, b: 0, a: 1}\n    - _EmissionColor: {r: 2, g: 0, b: 0, a: 1}\n"
                )
        );
        repo.write("Assets/T/hull.png", "x")
            .write("Assets/T/hull.png.meta", &format!("guid: {TEX}\n"))
            .write(
                "Assets/S/Hull.shader",
                "Shader \"BoatAttack/Hull\"\n{\n  Properties\n  {\n    [MainTexture] _BaseMap (\"Albedo\", 2D) = \"white\" {}\n    [HDR]_BaseColor(\"Color\", Color) = (1,1,1,1)\n    _Smoothness (\"Smooth\", Range(0,1)) = 0.5\n  }\n  SubShader { Pass { } }\n}\n",
            )
            .write("Assets/S/Hull.shader.meta", &format!("guid: {SHADER}\n"))
            .write("Assets/M/hull.mat", &body)
            .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let files = Files::new(&r, &Version::Head).unwrap();
        let v = MaterialView::load("Assets/M/hull.mat", &Guids::new(&files)).unwrap();
        let get = |k: &str| v.properties.iter().find(|p| p.key == k).unwrap();
        assert_eq!(get("_BaseMap").label, "Base Map");
        assert_eq!(
            get("_BaseMap").value,
            "hull.png · tiling 2, 3 · offset 0.5, 0"
        );
        assert_eq!(get("_BumpMap").value, "none");
        assert_eq!(get("_BaseColor").value, "#FF8000");
        assert_eq!(get("_BaseColor").color, Some([1.0, 0.5, 0.0, 1.0]));
        assert_eq!(get("_EmissionColor").value, "(2, 0, 0, 1)");
        assert_eq!(get("_Cull").value, "2");
        assert!(!get("_BaseMap").stale && !get("_Smoothness").stale);
        assert!(get("_Metallic").stale && get("_BumpMap").stale);
        assert_eq!(v.keywords, ["_ALPHATEST_ON", "_NORMALMAP"]);
        assert_eq!(v.render_queue, Some(2450));
        assert_eq!(v.material.shader, "BoatAttack/Hull");
        assert!(MaterialView::load("Assets/M/gone.mat", &Guids::new(&files)).is_err());
    }
}
