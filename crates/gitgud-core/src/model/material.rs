use serde::Serialize;

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
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    pub tiling: [f32; 2],
    pub offset: [f32; 2],
    pub metallic: f32,
    pub smoothness: f32,
    /// The referenced `.mat` or its shader couldn't be found.
    pub missing: bool,
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
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            metallic: 0.0,
            smoothness: 0.0,
            missing: true,
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
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            metallic: 0.0,
            smoothness: 0.5,
            missing: false,
        };
        for (key, tex) in keyed_list(&props["m_TexEnvs"]) {
            let slot = match key.as_str() {
                "_BaseMap" | "_MainTex" | "_BaseColorMap" => &mut m.base_map,
                "_BumpMap" | "_NormalMap" => &mut m.normal_map,
                _ => continue,
            };
            if slot.is_some() {
                continue;
            }
            let Some(guid) = obj_ref(&tex["m_Texture"]).and_then(|r| r.guid) else {
                continue;
            };
            if let Some(p) = guids.path(&guid)?.filter(|p| image_type(p).is_some()) {
                *slot = Some(p);
                if matches!(key.as_str(), "_BaseMap" | "_MainTex" | "_BaseColorMap") {
                    m.tiling = vec2(&tex["m_Scale"], [1.0, 1.0]);
                    m.offset = vec2(&tex["m_Offset"], [0.0, 0.0]);
                }
            }
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
        Ok(m)
    }
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
}
