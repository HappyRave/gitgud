use serde::Serialize;
use yaml_rust2::{Yaml, YamlLoader};

use super::yaml::{num, scalar};

/// One sprite of a texture, in Unity's pixel space (origin bottom-left).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sprite {
    pub name: String,
    /// `[x, y, width, height]`.
    pub rect: [f32; 4],
    /// Normalized within the rect, `[0, 0]` bottom-left.
    pub pivot: [f32; 2],
    /// 9-slice borders in pixels: `[left, bottom, right, top]`.
    pub border: [f32; 4],
}

/// How a sprite changed between two versions of the texture's import settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteChange {
    pub name: String,
    /// "added", "removed", "moved", "resized", "pivot", "border".
    pub kinds: Vec<&'static str>,
    pub note: String,
}

/// A texture's slicing at two versions.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteSheet {
    /// "single", "multiple", or "none" when the texture isn't imported as sprites.
    pub mode: String,
    pub before: Option<Vec<Sprite>>,
    pub after: Vec<Sprite>,
    pub changes: Vec<SpriteChange>,
}

fn vec4(y: &Yaml, keys: [&str; 4]) -> [f32; 4] {
    keys.map(|k| num(&y[k]).unwrap_or(0.0))
}

/// The sprites a texture's `.meta` text defines: one per entry of a Multiple
/// sheet, the whole texture for Single (rect left empty: its size is the image's).
pub(crate) fn read_sprites(meta: &str) -> (String, Vec<Sprite>) {
    let Ok(mut docs) = YamlLoader::load_from_str(meta) else {
        return ("none".into(), Vec::new());
    };
    let root = docs.pop().unwrap_or(Yaml::Null);
    let ti = &root["TextureImporter"];
    let mode = num(&ti["spriteMode"]).unwrap_or(0.0) as i32;
    match mode {
        1 => {
            let pivot = &ti["spritePivot"];
            let sprite = Sprite {
                name: String::new(),
                rect: [0.0; 4],
                pivot: [
                    num(&pivot["x"]).unwrap_or(0.5),
                    num(&pivot["y"]).unwrap_or(0.5),
                ],
                border: vec4(&ti["spriteBorder"], ["x", "y", "z", "w"]),
            };
            ("single".into(), vec![sprite])
        }
        2 => {
            let sprites = ti["spriteSheet"]["sprites"]
                .as_vec()
                .map(|list| {
                    list.iter()
                        .map(|s| Sprite {
                            name: scalar(&s["name"]).unwrap_or_default(),
                            rect: vec4(&s["rect"], ["x", "y", "width", "height"]),
                            pivot: [
                                num(&s["pivot"]["x"]).unwrap_or(0.5),
                                num(&s["pivot"]["y"]).unwrap_or(0.5),
                            ],
                            border: vec4(&s["border"], ["x", "y", "z", "w"]),
                        })
                        .collect()
                })
                .unwrap_or_default();
            ("multiple".into(), sprites)
        }
        _ => ("none".into(), Vec::new()),
    }
}

/// Sprites added, removed, moved, resized, re-pivoted or re-bordered, matched by name.
pub(crate) fn diff_sprites(before: &[Sprite], after: &[Sprite]) -> Vec<SpriteChange> {
    let fmt = |v: f32| {
        if v.fract() == 0.0 {
            format!("{v}")
        } else {
            format!("{v:.2}")
        }
    };
    let mut out = Vec::new();
    for a in after {
        let Some(b) = before.iter().find(|b| b.name == a.name) else {
            out.push(SpriteChange {
                name: a.name.clone(),
                kinds: vec!["added"],
                note: format!("{}×{}", fmt(a.rect[2]), fmt(a.rect[3])),
            });
            continue;
        };
        let mut kinds = Vec::new();
        let mut notes = Vec::new();
        if b.rect[..2] != a.rect[..2] {
            kinds.push("moved");
            notes.push(format!(
                "moved {}, {} → {}, {}",
                fmt(b.rect[0]),
                fmt(b.rect[1]),
                fmt(a.rect[0]),
                fmt(a.rect[1])
            ));
        }
        if b.rect[2..] != a.rect[2..] {
            kinds.push("resized");
            notes.push(format!(
                "resized {}×{} → {}×{}",
                fmt(b.rect[2]),
                fmt(b.rect[3]),
                fmt(a.rect[2]),
                fmt(a.rect[3])
            ));
        }
        if b.pivot != a.pivot {
            kinds.push("pivot");
            notes.push(format!(
                "pivot {}, {} → {}, {}",
                fmt(b.pivot[0]),
                fmt(b.pivot[1]),
                fmt(a.pivot[0]),
                fmt(a.pivot[1])
            ));
        }
        if b.border != a.border {
            kinds.push("border");
            notes.push("9-slice border changed".into());
        }
        if !kinds.is_empty() {
            out.push(SpriteChange {
                name: a.name.clone(),
                kinds,
                note: notes.join(" · "),
            });
        }
    }
    for b in before {
        if !after.iter().any(|a| a.name == b.name) {
            out.push(SpriteChange {
                name: b.name.clone(),
                kinds: vec!["removed"],
                note: String::new(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(sprites: &str) -> String {
        format!(
            "fileFormatVersion: 2\nguid: 1111\nTextureImporter:\n  spriteMode: 2\n  spriteSheet:\n    sprites:\n{sprites}"
        )
    }

    fn sprite(name: &str, x: u32, w: u32, pivot: f32, border: u32) -> String {
        format!(
            "    - serializedVersion: 2\n      name: {name}\n      rect:\n        serializedVersion: 2\n        x: {x}\n        y: 0\n        width: {w}\n        height: 32\n      alignment: 9\n      pivot: {{x: {pivot}, y: 0.5}}\n      border: {{x: {border}, y: 0, z: 0, w: 0}}\n"
        )
    }

    #[test]
    fn reads_a_multiple_sheet_and_diffs_it_by_name() {
        let (mode, before) = read_sprites(&meta(
            &[
                sprite("anchor", 0, 32, 0.5, 0),
                sprite("wheel", 32, 32, 0.5, 0),
                sprite("raft", 64, 32, 0.5, 0),
            ]
            .concat(),
        ));
        assert_eq!(mode, "multiple");
        assert_eq!(before[1].rect, [32.0, 0.0, 32.0, 32.0]);
        let (_, after) = read_sprites(&meta(
            &[
                sprite("anchor", 0, 40, 0.5, 0),
                sprite("wheel", 48, 32, 0.25, 4),
                sprite("fuel", 96, 32, 0.5, 0),
            ]
            .concat(),
        ));
        let changes = diff_sprites(&before, &after);
        let kinds: Vec<_> = changes
            .iter()
            .map(|c| (c.name.as_str(), c.kinds.clone()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("anchor", vec!["resized"]),
                ("wheel", vec!["moved", "pivot", "border"]),
                ("fuel", vec!["added"]),
                ("raft", vec!["removed"]),
            ]
        );
        assert_eq!(changes[0].note, "resized 32×32 → 40×32");
    }

    #[test]
    fn single_sprites_and_plain_textures() {
        let (mode, s) = read_sprites(
            "TextureImporter:\n  spriteMode: 1\n  spritePivot: {x: 0.5, y: 0}\n  spriteBorder: {x: 4, y: 4, z: 4, w: 4}\n",
        );
        assert_eq!(mode, "single");
        assert_eq!((s[0].pivot, s[0].border), ([0.5, 0.0], [4.0; 4]));
        assert_eq!(
            read_sprites("TextureImporter:\n  spriteMode: 0\n").0,
            "none"
        );
        assert_eq!(read_sprites("::: not yaml").0, "none");
    }
}
