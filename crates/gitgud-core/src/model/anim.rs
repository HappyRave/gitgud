use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

use super::guids::Guids;
use super::meta::Meta;
use super::yaml::{docs, num, obj_ref, quat, scalar, vec3};
use crate::repo::Error;

const ANIMATION_CLIP: u32 = 74;
const ANIMATOR_STATE: u32 = 1102;
const BLEND_TREE: u32 = 206;

/// Where a clip's keys come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipSource {
    /// Keys read from a `.anim` file, in right-handed space.
    #[serde(rename_all = "camelCase")]
    Curves {
        duration: f32,
        looping: bool,
        tracks: Vec<Track>,
    },
    /// A take inside a model file, played by the viewer's model loader.
    #[serde(rename_all = "camelCase")]
    Model { path: String, take: String },
}

/// Keys for one property of the object at `path` (relative to the Animator, `/`-separated).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub path: String,
    /// `position`, `quaternion` or `scale`.
    pub property: String,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
}

/// A clip an Animator on node `node` can play.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneClip {
    pub node: usize,
    pub name: String,
    pub source: ClipSource,
}

/// Unity's `Quaternion.Euler`: Z, then X, then Y, in degrees.
fn euler(deg: [f32; 3]) -> [f32; 4] {
    let h = |d: f32| (d.to_radians() / 2.0).sin_cos();
    let (sx, cx) = h(deg[0]);
    let (sy, cy) = h(deg[1]);
    let (sz, cz) = h(deg[2]);
    let mul = |a: [f32; 4], b: [f32; 4]| {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    };
    mul(
        mul([0.0, sy, 0.0, cy], [sx, 0.0, 0.0, cx]),
        [0.0, 0.0, sz, cz],
    )
}

fn curves(list: &Yaml, property: &'static str, out: &mut Vec<Track>, end: &mut f32) {
    for c in list.as_vec().into_iter().flatten() {
        let path = scalar(&c["path"]).unwrap_or_default();
        let mut times = Vec::new();
        let mut values = Vec::new();
        for k in c["curve"]["m_Curve"].as_vec().into_iter().flatten() {
            let t = num(&k["time"]).unwrap_or(0.0);
            *end = end.max(t);
            times.push(t);
            let v = &k["value"];
            match property {
                "position" => {
                    let p = vec3(v, [0.0; 3]);
                    values.extend([-p[0], p[1], p[2]]);
                }
                "scale" => values.extend(vec3(v, [1.0; 3])),
                _ => {
                    let q = quat(v);
                    values.extend([q[0], -q[1], -q[2], q[3]]);
                }
            }
        }
        if !times.is_empty() {
            out.push(Track {
                path,
                property: property.to_owned(),
                times,
                values,
            });
        }
    }
}

fn euler_curves(list: &Yaml, out: &mut Vec<Track>, end: &mut f32) {
    for c in list.as_vec().into_iter().flatten() {
        let path = scalar(&c["path"]).unwrap_or_default();
        let mut times = Vec::new();
        let mut values = Vec::new();
        for k in c["curve"]["m_Curve"].as_vec().into_iter().flatten() {
            let t = num(&k["time"]).unwrap_or(0.0);
            *end = end.max(t);
            times.push(t);
            let q = euler(vec3(&k["value"], [0.0; 3]));
            values.extend([q[0], -q[1], -q[2], q[3]]);
        }
        if !times.is_empty() {
            out.push(Track {
                path,
                property: "quaternion".to_owned(),
                times,
                values,
            });
        }
    }
}

/// Reads a `.anim` file's transform curves.
pub(crate) fn anim_clip(text: &str) -> Option<(String, ClipSource)> {
    let doc = docs(text)
        .into_iter()
        .find(|d| d.class_id == ANIMATION_CLIP)?;
    let b = &doc.body;
    let mut tracks = Vec::new();
    let mut end = 0.0f32;
    curves(&b["m_PositionCurves"], "position", &mut tracks, &mut end);
    curves(&b["m_RotationCurves"], "quaternion", &mut tracks, &mut end);
    curves(&b["m_ScaleCurves"], "scale", &mut tracks, &mut end);
    euler_curves(&b["m_EulerCurves"], &mut tracks, &mut end);
    let settings = &b["m_AnimationClipSettings"];
    let duration = num(&settings["m_StopTime"])
        .filter(|t| *t > 0.0)
        .unwrap_or(end);
    let looping = num(&settings["m_LoopTime"]).is_some_and(|l| l != 0.0);
    Some((
        scalar(&b["m_Name"]).unwrap_or_default(),
        ClipSource::Curves {
            duration,
            looping,
            tracks,
        },
    ))
}

/// Every clip an Animator Controller's states and blend trees play.
pub(crate) fn controller_clips(
    path: &str,
    guids: &Guids<'_>,
) -> Result<Vec<(String, ClipSource)>, Error> {
    let Some(text) = guids.files.read_text(path)? else {
        return Ok(Vec::new());
    };
    let mut motions = BTreeSet::new();
    for d in docs(&text) {
        match d.class_id {
            ANIMATOR_STATE => motions
                .extend(obj_ref(&d.body["m_Motion"]).and_then(|r| Some((r.guid?, r.file_id)))),
            BLEND_TREE => {
                for child in d.body["m_Childs"].as_vec().into_iter().flatten() {
                    motions.extend(
                        obj_ref(&child["m_Motion"]).and_then(|r| Some((r.guid?, r.file_id))),
                    );
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (guid, file_id) in motions {
        let Some(p) = guids.path(&guid)? else {
            continue;
        };
        if p.to_ascii_lowercase().ends_with(".anim") {
            if let Some(clip) = guids.files.read_text(&p)?.and_then(|t| anim_clip(&t)) {
                out.push(clip);
            }
            continue;
        }
        let meta = guids
            .files
            .read_text(&format!("{p}.meta"))?
            .map(|t| Meta::parse(&t))
            .unwrap_or_default();
        if let Some(name) = meta.clips.get(&file_id) {
            let take = meta
                .takes
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.clone());
            out.push((name.clone(), ClipSource::Model { path: p, take }));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Version;
    use crate::model::tree::Files;
    use crate::testing::TestRepo;

    #[test]
    fn converts_unity_euler_angles() {
        let q = euler([0.0, 90.0, 0.0]);
        assert!(
            (q[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6
                && (q[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6
        );
        // Z first, then X, then Y: (90, 0, 90) = qx * qz.
        let q = euler([90.0, 0.0, 90.0]);
        for (got, want) in q.iter().zip([0.5, -0.5, 0.5, 0.5]) {
            assert!((got - want).abs() < 1e-6, "{q:?}");
        }
    }

    const ANIM: &str = "%YAML 1.1\n--- !u!74 &7400000\nAnimationClip:\n  m_Name: Bob\n  m_PositionCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: {x: 1, y: 0, z: 0}\n      - time: 1.5\n        value: {x: 1, y: 0.25, z: 0}\n    path: Hull\n  m_EulerCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: {x: 0, y: 90, z: 0}\n    path: \n  m_AnimationClipSettings:\n    m_StopTime: 2\n    m_LoopTime: 1\n";

    #[test]
    fn reads_anim_curves_into_right_handed_tracks() {
        let (name, src) = anim_clip(ANIM).unwrap();
        assert_eq!(name, "Bob");
        let ClipSource::Curves {
            duration,
            looping,
            tracks,
        } = src
        else {
            panic!("curves expected")
        };
        assert_eq!((duration, looping), (2.0, true));
        assert_eq!(tracks[0].path, "Hull");
        assert_eq!(tracks[0].values, vec![-1.0, 0.0, 0.0, -1.0, 0.25, 0.0]);
        assert_eq!(tracks[1].property, "quaternion");
        assert!(
            (tracks[1].values[1] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "y mirrored"
        );
    }

    #[test]
    fn lists_controller_motions_from_anim_files_and_models() {
        let (anim, fbx, ctrl) = (
            "a0000000000000000000000000000001",
            "f0000000000000000000000000000002",
            "c0000000000000000000000000000003",
        );
        let controller = format!(
            "%YAML 1.1\n--- !u!1102 &1\nAnimatorState:\n  m_Name: Idle\n  m_Motion: {{fileID: 7400000, guid: {anim}, type: 2}}\n--- !u!206 &2\nBlendTree:\n  m_Childs:\n  - m_Motion: {{fileID: -203655887218126122, guid: {fbx}, type: 3}}\n"
        );
        let repo = TestRepo::init();
        repo.write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.1f1\n")
            .write("Assets/Bob.anim", ANIM)
            .write("Assets/Bob.anim.meta", &format!("guid: {anim}\n"))
            .write("Assets/run.fbx", "fbx")
            .write(
                "Assets/run.fbx.meta",
                &format!("guid: {fbx}\nModelImporter:\n  internalIDToNameTable:\n  - first:\n      74: -203655887218126122\n    second: Run\n  animations:\n    clipAnimations:\n    - name: Run\n      takeName: Take 001\n"),
            )
            .write("Assets/Boat.controller", &controller)
            .write("Assets/Boat.controller.meta", &format!("guid: {ctrl}\n"))
            .commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let v = Version::Head;
        let files = Files::new(&r, &v).unwrap();
        let guids = Guids::new(&files);
        let clips = controller_clips("Assets/Boat.controller", &guids).unwrap();
        let names: Vec<&str> = clips.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["Bob", "Run"]);
        assert_eq!(
            clips[1].1,
            ClipSource::Model {
                path: "Assets/run.fbx".into(),
                take: "Take 001".into()
            }
        );
    }
}
