use std::collections::BTreeMap;

use serde::Serialize;
use yaml_rust2::Yaml;

use super::guids::Guids;
use super::meta::Meta;
use super::tree::base_name;
use super::yaml::{docs, num, quat, scalar, vec3};
use crate::repo::Error;

const ANIMATION_CLIP: u32 = 74;

/// One key of a curve as Unity's Curve Editor shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveKey {
    pub time: f32,
    pub value: f32,
    /// `None` for an infinite slope: the curve holds, then steps.
    pub in_slope: Option<f32>,
    pub out_slope: Option<f32>,
}

/// One animated property of one object, at both versions.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveTrack {
    /// Object path relative to the Animator (`""` for the root).
    pub path: String,
    /// "Position.y", "Rotation.x", "material._BaseColor.r"...
    pub label: String,
    pub before: Option<Vec<CurveKey>>,
    pub after: Option<Vec<CurveKey>>,
    /// "same", "changed", "added" or "removed".
    pub state: &'static str,
    /// Rotation stored as quaternions, shown as Euler angles at its keys joined by lines.
    pub approximated: bool,
}

/// A sprite (or other object) swap track: which asset shows from which time.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlipbookTrack {
    pub path: String,
    pub label: String,
    pub before: Option<Vec<(f32, String)>>,
    pub after: Option<Vec<(f32, String)>>,
    pub state: &'static str,
}

/// A `.anim` clip's curves compared across two versions.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipCurves {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub sample_rate: f32,
    pub tracks: Vec<CurveTrack>,
    pub flipbooks: Vec<FlipbookTrack>,
    pub keys_before: Option<usize>,
    pub keys_after: usize,
}

/// `(object path, property label)`.
type TrackId = (String, String);
/// The asset shown from each time on.
type Frames = Vec<(f32, String)>;

struct Clip {
    name: String,
    duration: f32,
    looping: bool,
    sample_rate: f32,
    /// Keys per track in file order, and whether the track is approximated.
    curves: Vec<(TrackId, Vec<CurveKey>, bool)>,
    flipbooks: Vec<(TrackId, Frames)>,
}

fn slope(y: &Yaml) -> Option<f32> {
    num(y).filter(|s| s.is_finite())
}

fn label(attribute: &str) -> String {
    let (head, axis) = attribute.rsplit_once('.').unwrap_or((attribute, ""));
    let name = match head {
        "m_LocalPosition" => "Position",
        "localEulerAnglesRaw" | "localEulerAngles" | "localEulerAnglesBaked" => "Rotation",
        "m_LocalScale" => "Scale",
        "m_LocalRotation" => "Rotation (quaternion)",
        "m_IsActive" => return "Active".into(),
        _ => return attribute.to_owned(),
    };
    format!("{name}.{axis}")
}

fn read_clip(text: &str, guids: &Guids<'_>) -> Result<Option<Clip>, Error> {
    let Some(doc) = docs(text)
        .into_iter()
        .find(|d| d.class_id == ANIMATION_CLIP)
    else {
        return Ok(None);
    };
    let b = &doc.body;
    let mut curves: Vec<((String, String), Vec<CurveKey>, bool)> = Vec::new();
    let mut end = 0.0f32;
    let mut push = |id: (String, String), keys: Vec<CurveKey>, approx: bool| {
        if !keys.is_empty() && !curves.iter().any(|(k, _, _)| *k == id) {
            curves.push((id, keys, approx));
        }
    };
    for list in ["m_EditorCurves", "m_FloatCurves"] {
        for c in b[list].as_vec().into_iter().flatten() {
            let path = scalar(&c["path"]).unwrap_or_default();
            let attr = scalar(&c["attribute"]).unwrap_or_default();
            let keys: Vec<CurveKey> = c["curve"]["m_Curve"]
                .as_vec()
                .into_iter()
                .flatten()
                .map(|k| CurveKey {
                    time: num(&k["time"]).unwrap_or(0.0),
                    value: num(&k["value"]).unwrap_or(0.0),
                    in_slope: slope(&k["inSlope"]),
                    out_slope: slope(&k["outSlope"]),
                })
                .collect();
            end = keys.iter().fold(end, |e, k| e.max(k.time));
            push((path, label(&attr)), keys, false);
        }
    }
    for (list, name, default) in [
        ("m_PositionCurves", "Position", 0.0),
        ("m_EulerCurves", "Rotation", 0.0),
        ("m_ScaleCurves", "Scale", 1.0),
    ] {
        for c in b[list].as_vec().into_iter().flatten() {
            let path = scalar(&c["path"]).unwrap_or_default();
            let raw: Vec<&Yaml> = c["curve"]["m_Curve"]
                .as_vec()
                .into_iter()
                .flatten()
                .collect();
            for (axis, i) in [("x", 0), ("y", 1), ("z", 2)] {
                let keys: Vec<CurveKey> = raw
                    .iter()
                    .map(|k| CurveKey {
                        time: num(&k["time"]).unwrap_or(0.0),
                        value: vec3(&k["value"], [default; 3])[i],
                        in_slope: slope(&k["inSlope"][axis]),
                        out_slope: slope(&k["outSlope"][axis]),
                    })
                    .collect();
                end = keys.iter().fold(end, |e, k| e.max(k.time));
                push((path.clone(), format!("{name}.{axis}")), keys, false);
            }
        }
    }
    for c in b["m_RotationCurves"].as_vec().into_iter().flatten() {
        let path = scalar(&c["path"]).unwrap_or_default();
        let raw: Vec<(f32, [f32; 3])> = c["curve"]["m_Curve"]
            .as_vec()
            .into_iter()
            .flatten()
            .map(|k| (num(&k["time"]).unwrap_or(0.0), to_euler(quat(&k["value"]))))
            .collect();
        let unwrapped = unwrap_angles(&raw);
        for (axis, i) in [("x", 0), ("y", 1), ("z", 2)] {
            // Tangents don't survive the conversion: the viewer joins these keys with lines.
            let keys: Vec<CurveKey> = unwrapped
                .iter()
                .map(|(t, e)| CurveKey {
                    time: *t,
                    value: e[i],
                    in_slope: None,
                    out_slope: None,
                })
                .collect();
            end = keys.iter().fold(end, |e, k| e.max(k.time));
            push((path.clone(), format!("Rotation.{axis}")), keys, true);
        }
    }
    let mut flipbooks = Vec::new();
    for c in b["m_PPtrCurves"].as_vec().into_iter().flatten() {
        let path = scalar(&c["path"]).unwrap_or_default();
        let attr = scalar(&c["attribute"]).unwrap_or_default();
        let mut keys = Vec::new();
        for k in c["curve"].as_vec().into_iter().flatten() {
            let t = num(&k["time"]).unwrap_or(0.0);
            end = end.max(t);
            keys.push((t, object_name(&k["value"], guids)?));
        }
        if !keys.is_empty() {
            let name = if attr == "m_Sprite" {
                "Sprite".to_owned()
            } else {
                attr
            };
            flipbooks.push(((path, name), keys));
        }
    }
    let settings = &b["m_AnimationClipSettings"];
    Ok(Some(Clip {
        name: scalar(&b["m_Name"]).unwrap_or_default(),
        duration: num(&settings["m_StopTime"])
            .filter(|t| *t > 0.0)
            .unwrap_or(end),
        looping: num(&settings["m_LoopTime"]).is_some_and(|l| l != 0.0),
        sample_rate: num(&b["m_SampleRate"]).unwrap_or(60.0),
        curves,
        flipbooks,
    }))
}

/// A referenced object's name: the sprite's name from its texture's `.meta`, else the file's.
fn object_name(r: &Yaml, guids: &Guids<'_>) -> Result<String, Error> {
    let id = scalar(&r["fileID"]).unwrap_or_default();
    let Some(guid) = scalar(&r["guid"]).filter(|g| !g.is_empty()) else {
        return Ok(if id == "0" {
            "none".into()
        } else {
            format!("&{id}")
        });
    };
    let Some(path) = guids.path(&guid)? else {
        return Ok(format!("missing ({guid})"));
    };
    let sub = guids
        .files
        .read_text(&format!("{path}.meta"))?
        .and_then(|t| Meta::parse(&t).names.get(&id).cloned());
    Ok(sub.unwrap_or_else(|| base_name(&path).to_owned()))
}

/// Unity's Euler angles (degrees, Z then X then Y) of a quaternion.
fn to_euler(q: [f32; 4]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let sinx = 2.0 * (w * x - y * z);
    let ex = if sinx.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sinx)
    } else {
        sinx.asin()
    };
    let ey = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let ez = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (x * x + z * z));
    [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()]
}

/// Keeps each axis within 180° of its previous key, so a turn past ±180 doesn't jump.
fn unwrap_angles(keys: &[(f32, [f32; 3])]) -> Vec<(f32, [f32; 3])> {
    let mut out: Vec<(f32, [f32; 3])> = Vec::with_capacity(keys.len());
    for (t, e) in keys {
        let mut e = *e;
        if let Some((_, prev)) = out.last() {
            for i in 0..3 {
                while e[i] - prev[i] > 180.0 {
                    e[i] -= 360.0;
                }
                while e[i] - prev[i] < -180.0 {
                    e[i] += 360.0;
                }
            }
        }
        out.push((*t, e));
    }
    out
}

fn state<T: PartialEq>(b: Option<&T>, a: Option<&T>, compare: bool) -> &'static str {
    match (b, a) {
        _ if !compare => "same",
        (None, Some(_)) => "added",
        (Some(_), None) => "removed",
        (Some(x), Some(y)) if x != y => "changed",
        _ => "same",
    }
}

/// The curves of the `.anim` texts, the earlier one when the file was there.
pub(crate) fn clip_curves(
    before: Option<(&str, &Guids<'_>)>,
    after: (&str, &Guids<'_>),
    path: &str,
) -> Result<ClipCurves, Error> {
    let not_clip = || Error::Refused(format!("{path} isn't an animation clip"));
    let a = read_clip(after.0, after.1)?.ok_or_else(not_clip)?;
    let b = match before {
        Some((text, g)) => Some(read_clip(text, g)?.ok_or_else(not_clip)?),
        None => None,
    };
    let compare = b.is_some();
    let mut order: Vec<(String, String)> = a.curves.iter().map(|(id, _, _)| id.clone()).collect();
    let before_curves: BTreeMap<_, _> = b
        .iter()
        .flat_map(|c| {
            c.curves
                .iter()
                .map(|(id, k, ap)| (id.clone(), (k.clone(), *ap)))
        })
        .collect();
    for id in before_curves.keys() {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    let after_curves: BTreeMap<_, _> = a
        .curves
        .iter()
        .map(|(id, k, ap)| (id.clone(), (k.clone(), *ap)))
        .collect();
    let tracks = order
        .into_iter()
        .map(|id| {
            let bk = before_curves.get(&id);
            let ak = after_curves.get(&id);
            CurveTrack {
                state: state(bk.map(|x| &x.0), ak.map(|x| &x.0), compare),
                approximated: bk.or(ak).is_some_and(|x| x.1),
                before: bk.map(|x| x.0.clone()),
                after: ak.map(|x| x.0.clone()),
                path: id.0,
                label: id.1,
            }
        })
        .collect();
    let mut flip_order: Vec<(String, String)> =
        a.flipbooks.iter().map(|(id, _)| id.clone()).collect();
    let before_flips: BTreeMap<_, _> = b.iter().flat_map(|c| c.flipbooks.iter().cloned()).collect();
    for id in before_flips.keys() {
        if !flip_order.contains(id) {
            flip_order.push(id.clone());
        }
    }
    let after_flips: BTreeMap<_, _> = a.flipbooks.iter().cloned().collect();
    let flipbooks = flip_order
        .into_iter()
        .map(|id| {
            let bk = before_flips.get(&id);
            let ak = after_flips.get(&id);
            FlipbookTrack {
                state: state(bk, ak, compare),
                before: bk.cloned(),
                after: ak.cloned(),
                path: id.0,
                label: id.1,
            }
        })
        .collect();
    let count = |c: &Clip| {
        c.curves.iter().map(|(_, k, _)| k.len()).sum::<usize>()
            + c.flipbooks.iter().map(|(_, k)| k.len()).sum::<usize>()
    };
    Ok(ClipCurves {
        name: a.name.clone(),
        duration: a.duration.max(b.as_ref().map_or(0.0, |c| c.duration)),
        looping: a.looping,
        sample_rate: a.sample_rate,
        keys_before: b.as_ref().map(count),
        keys_after: count(&a),
        tracks,
        flipbooks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Version;
    use crate::model::tree::Files;
    use crate::testing::TestRepo;

    fn anim(y: &str, extra: &str) -> String {
        format!(
            "%YAML 1.1\n--- !u!74 &7400000\nAnimationClip:\n  m_Name: Bob\n  m_SampleRate: 30\n  m_EditorCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: 0\n        inSlope: 0\n        outSlope: 0\n      - time: 1\n        value: {y}\n        inSlope: Infinity\n        outSlope: 0\n    attribute: m_LocalPosition.y\n    path: Hull\n{extra}  m_AnimationClipSettings:\n    m_StopTime: 2\n    m_LoopTime: 1\n"
        )
    }

    #[test]
    fn reads_editor_curves_and_diffs_tracks() {
        let flag = "  - curve:\n      m_Curve:\n      - time: 0\n        value: 0\n        inSlope: 0\n        outSlope: 0\n    attribute: localEulerAnglesRaw.y\n    path: Flag\n";
        let repo = TestRepo::init();
        repo.write("a.txt", "x").commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let files = Files::new(&r, &Version::Head).unwrap();
        let g = Guids::new(&files);
        let before = anim("0.25", "");
        let after = anim("0.4", flag);
        let c = clip_curves(Some((&before, &g)), (&after, &g), "Bob.anim").unwrap();
        assert_eq!(
            (c.name.as_str(), c.duration, c.looping, c.sample_rate),
            ("Bob", 2.0, true, 30.0)
        );
        assert_eq!(c.tracks[0].label, "Position.y");
        assert_eq!(c.tracks[0].state, "changed");
        assert_eq!(
            c.tracks[0].after.as_ref().unwrap()[1].in_slope,
            None,
            "Infinity steps"
        );
        assert_eq!(
            (c.tracks[1].label.as_str(), c.tracks[1].state),
            ("Rotation.y", "added")
        );
        assert_eq!((c.keys_before, c.keys_after), (Some(2), 3));
        let alone = clip_curves(None, (&after, &g), "Bob.anim").unwrap();
        assert!(alone.tracks.iter().all(|t| t.state == "same"));
        assert!(clip_curves(None, ("--- !u!1 &1\nGameObject:\n", &g), "x.anim").is_err());
    }

    #[test]
    fn splits_vector_curves_and_shows_quaternions_as_unwrapped_euler() {
        let text = "--- !u!74 &7400000\nAnimationClip:\n  m_Name: Spin\n  m_PositionCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: {x: 1, y: 2, z: 3}\n        inSlope: {x: 0, y: 1, z: 0}\n        outSlope: {x: 0, y: 1, z: 0}\n    path: Hull\n  m_RotationCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: {x: 0, y: 0.9659258, z: 0, w: 0.2588190}\n      - time: 1\n        value: {x: 0, y: 0.9659258, z: 0, w: -0.2588190}\n    path: Hull\n";
        let repo = TestRepo::init();
        repo.write("a.txt", "x").commit_all("one");
        let r = crate::repo::open(repo.path()).unwrap();
        let files = Files::new(&r, &Version::Head).unwrap();
        let g = Guids::new(&files);
        let c = clip_curves(None, (text, &g), "Spin.anim").unwrap();
        let labels: Vec<_> = c.tracks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels[..3], ["Position.x", "Position.y", "Position.z"]);
        assert_eq!(c.tracks[1].after.as_ref().unwrap()[0].in_slope, Some(1.0));
        let ry = c.tracks.iter().find(|t| t.label == "Rotation.y").unwrap();
        assert!(ry.approximated);
        let keys = ry.after.as_ref().unwrap();
        assert!((keys[0].value - 150.0).abs() < 0.01, "{keys:?}");
        assert!(
            (keys[1].value - 210.0).abs() < 0.01,
            "unwrapped past 180: {keys:?}"
        );
    }
}
