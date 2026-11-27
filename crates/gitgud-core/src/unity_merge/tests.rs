use super::tree::{emit, parse_exact};
use super::{PickChoice, RowKind, RowPick, Source, analyze, merge};
use crate::unity_yaml::GuidIndex;

const SCENE: &str = "%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1 &100
GameObject:
  m_ObjectHideFlags: 0
  m_Component:
  - component: {fileID: 101}
  - component: {fileID: 102}
  m_Name: Lighthouse
--- !u!4 &101
Transform:
  m_GameObject: {fileID: 100}
  m_LocalPosition: {x: 0, y: 0, z: 0}
  m_Children:
  - {fileID: 201}
  m_Father: {fileID: 0}
--- !u!108 &102
Light:
  m_GameObject: {fileID: 100}
  m_Intensity: 1.2
  m_Range: 24
--- !u!1 &200
GameObject:
  m_Component:
  - component: {fileID: 201}
  - component: {fileID: 202}
  m_Name: OldPier
--- !u!4 &201
Transform:
  m_GameObject: {fileID: 200}
  m_LocalPosition: {x: 5, y: 0, z: 0}
  m_Children: []
  m_Father: {fileID: 101}
--- !u!108 &202
Light:
  m_GameObject: {fileID: 200}
  m_Range: 12
";

fn edit(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "fixture lacks {from:?}");
    text.replacen(from, to, 1)
}

fn pick(id: &str, choice: PickChoice) -> RowPick {
    RowPick {
        id: id.into(),
        choice,
        value: None,
    }
}

fn analyzed(base: &str, ours: &str, theirs: &str) -> super::PropertyMerge {
    analyze("a.unity", base, ours, theirs, &GuidIndex::default()).unwrap()
}

fn merged(base: &str, ours: &str, theirs: &str, picks: &[RowPick]) -> super::MergedFile {
    merge("a.unity", base, ours, theirs, picks, &GuidIndex::default()).unwrap()
}

/// SCENE without OldPier (200, 201, 202) and its link from Lighthouse.
fn without_old_pier(text: &str) -> String {
    let cut = text.find("--- !u!1 &200").unwrap();
    edit(
        &text[..cut],
        "  m_Children:\n  - {fileID: 201}\n",
        "  m_Children: []\n",
    )
}

#[test]
fn changes_on_different_properties_merge_cleanly() {
    let ours = edit(SCENE, "m_Intensity: 1.2", "m_Intensity: 1.6");
    let theirs = edit(SCENE, "m_Range: 24", "m_Range: 30");
    let m = analyzed(SCENE, &ours, &theirs);
    assert_eq!((m.yours, m.theirs, m.clashes), (1, 1, 0));
    let row = m.rows.iter().find(|r| r.source == Source::Theirs).unwrap();
    assert_eq!(row.object_path, "Lighthouse");
    assert_eq!(row.component, "Light");
    assert_eq!(row.property, "m_Range");
    assert_eq!(
        (row.base.as_deref(), row.theirs.as_deref()),
        (Some("24"), Some("30"))
    );

    let out = merged(SCENE, &ours, &theirs, &[]);
    assert_eq!(out.unresolved, 0);
    assert_eq!(out.text, edit(&ours, "m_Range: 24", "m_Range: 30"));
}

#[test]
fn each_axis_of_a_position_merges_on_its_own() {
    let ours = edit(SCENE, "{x: 0, y: 0, z: 0}", "{x: 1, y: 0, z: 0}");
    let theirs = edit(SCENE, "{x: 0, y: 0, z: 0}", "{x: 0, y: 0.35, z: 0}");
    let m = analyzed(SCENE, &ours, &theirs);
    let props: Vec<&str> = m.rows.iter().map(|r| r.property.as_str()).collect();
    assert_eq!(props, ["m_LocalPosition.x", "m_LocalPosition.y"]);
    assert!(
        merged(SCENE, &ours, &theirs, &[])
            .text
            .contains("{x: 1, y: 0.35, z: 0}")
    );
}

#[test]
fn a_clash_waits_for_a_pick_and_takes_it() {
    let ours = edit(SCENE, "m_Intensity: 1.2", "m_Intensity: 1.6");
    let theirs = edit(SCENE, "m_Intensity: 1.2", "m_Intensity: 2.4");
    let m = analyzed(SCENE, &ours, &theirs);
    assert_eq!(m.clashes, 1);
    let row = &m.rows[0];
    assert!(row.editable);
    assert_eq!(
        (row.yours.as_deref(), row.theirs.as_deref()),
        (Some("1.6"), Some("2.4"))
    );

    let open = merged(SCENE, &ours, &theirs, &[]);
    assert_eq!(open.unresolved, 1);
    assert!(open.text.contains("m_Intensity: 1.6"));

    let theirs_pick = merged(SCENE, &ours, &theirs, &[pick(&row.id, PickChoice::Theirs)]);
    assert_eq!(theirs_pick.unresolved, 0);
    assert!(theirs_pick.text.contains("m_Intensity: 2.4"));

    let custom = RowPick {
        id: row.id.clone(),
        choice: PickChoice::Custom,
        value: Some("2".into()),
    };
    assert!(
        merged(SCENE, &ours, &theirs, &[custom])
            .text
            .contains("m_Intensity: 2\n")
    );
}

#[test]
fn a_typed_value_is_written_as_unity_reads_it() {
    let ours = edit(SCENE, "m_Name: OldPier", "m_Name: Pier A");
    let theirs = edit(SCENE, "m_Name: OldPier", "m_Name: Pier B");
    let row = analyzed(SCENE, &ours, &theirs).rows.remove(0);
    let typed = |value: &str| {
        let custom = RowPick {
            id: row.id.clone(),
            choice: PickChoice::Custom,
            value: Some(value.into()),
        };
        let text = merged(SCENE, &ours, &theirs, &[custom]).text;
        let line = text
            .lines()
            .find(|l| l.contains("m_Name: ") && !l.contains("Lighthouse"));
        line.unwrap().trim().to_owned()
    };
    assert_eq!(typed("Boss: Phase 2"), "m_Name: 'Boss: Phase 2'");
    assert_eq!(typed("#2"), "m_Name: '#2'");
    assert_eq!(typed("it's, here"), "m_Name: 'it''s, here'");
    assert_eq!(typed("'already quoted'"), "m_Name: 'already quoted'");
    assert_eq!(typed("-1.5"), "m_Name: -1.5");
    assert_eq!(typed("Pier C"), "m_Name: Pier C");
}

#[test]
fn a_documents_frame_is_not_editable() {
    let ours = edit(SCENE, "--- !u!108 &202\nLight:", "--- !u!108 &202\nLightA:");
    let theirs = edit(SCENE, "--- !u!108 &202\nLight:", "--- !u!108 &202\nLightB:");
    let m = analyzed(SCENE, &ours, &theirs);
    let row = m.rows.iter().find(|r| r.property == "(type)").unwrap();
    assert!(!row.editable);
}

#[test]
fn a_one_sided_change_can_be_turned_back() {
    let theirs = edit(SCENE, "m_Range: 24", "m_Range: 30");
    let m = analyzed(SCENE, SCENE, &theirs);
    let out = merged(
        SCENE,
        SCENE,
        &theirs,
        &[pick(&m.rows[0].id, PickChoice::Yours)],
    );
    assert_eq!(out.text, SCENE);
}

#[test]
fn deleting_an_object_the_other_side_edited_is_one_clash() {
    let ours = without_old_pier(SCENE);
    let theirs = edit(SCENE, "m_Range: 12", "m_Range: 18");
    let m = analyzed(SCENE, &ours, &theirs);
    assert_eq!(m.rows.len(), 1, "{:#?}", m.rows);
    let row = &m.rows[0];
    assert_eq!((row.kind, row.source), (RowKind::Object, Source::Clash));
    assert_eq!(row.object_path, "Lighthouse / OldPier");
    assert_eq!(row.yours.as_deref(), Some("deleted"));
    assert_eq!(row.theirs.as_deref(), Some("kept, 1 change"));
    assert_eq!(row.theirs_detail, ["Light › m_Range: 12 → 18"]);

    let gone = merged(SCENE, &ours, &theirs, &[pick(&row.id, PickChoice::Yours)]);
    assert_eq!(gone.text, ours);
    let restored = merged(SCENE, &ours, &theirs, &[pick(&row.id, PickChoice::Theirs)]);
    assert_eq!(restored.text, theirs);
    assert!(restored.issues.is_empty(), "{:?}", restored.issues);
}

#[test]
fn deleting_an_untouched_object_is_one_row() {
    let ours = without_old_pier(SCENE);
    let m = analyzed(SCENE, &ours, SCENE);
    assert_eq!(m.rows.len(), 1);
    assert_eq!(
        (m.rows[0].kind, m.rows[0].source),
        (RowKind::Object, Source::Yours)
    );
    assert_eq!(m.rows[0].base.as_deref(), Some("1 object, 2 components"));
    assert_eq!(merged(SCENE, &ours, SCENE, &[]).text, ours);
}

fn with_child(text: &str, go: &str, name: &str) -> String {
    let tr = format!("{go}1");
    let text = edit(
        text,
        "  - {fileID: 201}\n  m_Father",
        &format!("  - {{fileID: 201}}\n  - {{fileID: {tr}}}\n  m_Father"),
    );
    format!(
        "{text}--- !u!1 &{go}
GameObject:
  m_Component:
  - component: {{fileID: {tr}}}
  m_Name: {name}
--- !u!4 &{tr}
Transform:
  m_GameObject: {{fileID: {go}}}
  m_Children: []
  m_Father: {{fileID: 101}}
"
    )
}

#[test]
fn children_added_on_both_sides_ask_for_their_order() {
    let ours = with_child(SCENE, "300", "Buoy (4)");
    let theirs = with_child(SCENE, "400", "Buoy_Red");
    let m = analyzed(SCENE, &ours, &theirs);
    let order = m.rows.iter().find(|r| r.kind == RowKind::Order).unwrap();
    assert_eq!(order.source, Source::Clash);
    assert_eq!(
        order.yours_detail,
        [
            "Lighthouse / OldPier",
            "Lighthouse / Buoy (4)",
            "Lighthouse / Buoy_Red"
        ]
    );
    assert_eq!(
        order.theirs_detail,
        [
            "Lighthouse / OldPier",
            "Lighthouse / Buoy_Red",
            "Lighthouse / Buoy (4)"
        ]
    );
    let added: Vec<_> = m
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Object)
        .collect();
    assert_eq!(added.len(), 2);
    assert_eq!(
        added[0].yours.as_deref(),
        Some("added: 1 object, 1 component")
    );

    let out = merged(
        SCENE,
        &ours,
        &theirs,
        &[pick(&order.id, PickChoice::Theirs)],
    );
    assert_eq!(out.unresolved, 0);
    assert!(
        out.text
            .contains("  - {fileID: 201}\n  - {fileID: 4001}\n  - {fileID: 3001}\n")
    );
    assert!(out.text.contains("m_Name: Buoy (4)") && out.text.contains("m_Name: Buoy_Red"));
    assert!(out.issues.is_empty(), "{:?}", out.issues);
}

#[test]
fn turning_back_an_added_object_drops_its_link_too() {
    let theirs = with_child(SCENE, "400", "Buoy_Red");
    let m = analyzed(SCENE, SCENE, &theirs);
    assert_eq!(m.rows.len(), 1, "{:#?}", m.rows);
    let out = merged(
        SCENE,
        SCENE,
        &theirs,
        &[pick(&m.rows[0].id, PickChoice::Yours)],
    );
    assert_eq!(out.text, SCENE);
}

const SO: &str = "%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!114 &11400000
MonoBehaviour:
  m_ObjectHideFlags: 0
  m_Script: {fileID: 11500000, guid: 0123456789abcdef0123456789abcdef, type: 3}
  m_Name: Waves
  stats:
    hp: 10
    speed: 5
  items:
  - id: 1
    name: crab
  - id: 2
    name: shark
  tags:
  - a
  - b
  intro: 'A long line Unity wrapped
    onto a second one'
";

#[test]
fn scriptable_object_fields_and_records_merge_per_field() {
    let ours = edit(SO, "name: crab", "name: king crab");
    let ours = edit(&ours, "onto a second one'", "onto a new second one'");
    let theirs = edit(SO, "name: shark", "name: tiger shark");
    let theirs = edit(&theirs, "speed: 5", "speed: 7");
    let m = analyzed(SO, &ours, &theirs);
    assert_eq!(m.clashes, 0, "{:#?}", m.rows);
    let props: Vec<&str> = m.rows.iter().map(|r| r.property.as_str()).collect();
    assert_eq!(
        props,
        ["stats.speed", "items[0].name", "items[1].name", "intro"]
    );
    let out = merged(SO, &ours, &theirs, &[]);
    for want in [
        "name: king crab",
        "name: tiger shark",
        "speed: 7",
        "onto a new second one'",
    ] {
        assert!(out.text.contains(want), "{want} missing from\n{}", out.text);
    }
}

#[test]
fn a_list_resized_on_both_sides_is_one_clash() {
    let ours = edit(SO, "  - b\n", "  - b\n  - c\n");
    let theirs = edit(SO, "  - b\n", "  - b\n  - d\n");
    let m = analyzed(SO, &ours, &theirs);
    assert_eq!(m.clashes, 1);
    assert_eq!(m.rows[0].property, "tags");
    assert!(!m.rows[0].editable);
}

const PREFAB_INSTANCE: &str = "%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1001 &500
PrefabInstance:
  m_ObjectHideFlags: 0
  serializedVersion: 2
  m_Modification:
    m_TransformParent: {fileID: 0}
    m_Modifications:
    - target: {fileID: 400000, guid: 11112222333344445555666677778888, type: 3}
      propertyPath: m_LocalPosition.x
      value: 1
      objectReference: {fileID: 0}
    - target: {fileID: 100000, guid: 11112222333344445555666677778888, type: 3}
      propertyPath: m_Name
      value: Crate
      objectReference: {fileID: 0}
    m_RemovedComponents: []
  m_SourcePrefab: {fileID: 100100000, guid: 11112222333344445555666677778888, type: 3}
";

#[test]
fn prefab_overrides_merge_by_target_and_property() {
    let ours = edit(PREFAB_INSTANCE, "value: 1\n", "value: 4\n");
    let theirs = edit(
        PREFAB_INSTANCE,
        "    m_RemovedComponents",
        "    - target: {fileID: 400000, guid: 11112222333344445555666677778888, type: 3}
      propertyPath: m_LocalPosition.y
      value: 2
      objectReference: {fileID: 0}
    m_RemovedComponents",
    );
    let m = analyzed(PREFAB_INSTANCE, &ours, &theirs);
    assert_eq!(m.clashes, 0, "{:#?}", m.rows);
    let props: Vec<&str> = m.rows.iter().map(|r| r.property.as_str()).collect();
    assert_eq!(props, ["m_LocalPosition.x", "m_LocalPosition.y"]);
    let out = merged(PREFAB_INSTANCE, &ours, &theirs, &[]);
    assert_eq!(out.text, edit(&theirs, "value: 1\n", "value: 4\n"));
}

#[test]
fn refuses_text_it_cannot_write_back() {
    let odd = "%YAML 1.1\n--- !u!1 &1\nGameObject:\n a: 1\n";
    let err = analyze("x.prefab", SCENE, odd, SCENE, &GuidIndex::default()).unwrap_err();
    assert!(err.to_string().contains("by property"), "{err}");
}

#[test]
fn refuses_a_pick_for_a_row_that_no_longer_exists() {
    let theirs = edit(SCENE, "m_Range: 24", "m_Range: 30");
    let picks = [pick("102/m_Nope", PickChoice::Yours)];
    let err = merge(
        "a.unity",
        SCENE,
        SCENE,
        &theirs,
        &picks,
        &GuidIndex::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("changed since"), "{err}");
}

#[test]
fn keeps_crlf_line_ends() {
    let crlf = SCENE.replace('\n', "\r\n");
    let theirs = crlf.replacen("m_Range: 24", "m_Range: 30", 1);
    assert_eq!(merged(&crlf, &crlf, &theirs, &[]).text, theirs);
}

/// Round-trips every Unity YAML file under `GITGUD_UNITY_CORPUS` (a Unity project).
#[test]
#[ignore = "needs GITGUD_UNITY_CORPUS"]
fn corpus_round_trips() {
    let root = std::env::var("GITGUD_UNITY_CORPUS").expect("GITGUD_UNITY_CORPUS");
    let mut stack = vec![std::path::PathBuf::from(root)];
    let (mut ok, mut refused, mut skipped) = (0, Vec::new(), 0);
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                if !matches!(name.as_str(), "Library" | "Temp" | ".git" | "Logs" | "obj") {
                    stack.push(p);
                }
                continue;
            }
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !matches!(
                ext.as_str(),
                "unity" | "prefab" | "asset" | "mat" | "controller" | "anim"
            ) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                skipped += 1;
                continue;
            };
            if !text.starts_with("%YAML") {
                skipped += 1;
                continue;
            }
            match parse_exact(&text) {
                Ok(f) => {
                    assert_eq!(emit(&f), text);
                    // A file merged with itself on every side comes back unchanged.
                    let m = merge("x", &text, &text, &text, &[], &GuidIndex::default()).unwrap();
                    assert_eq!(m.text, text, "{}", p.display());
                    ok += 1;
                }
                Err(e) => refused.push(format!("{}: {e}", p.display())),
            }
        }
    }
    eprintln!("ok {ok}, skipped {skipped}, refused {}", refused.len());
    for r in refused.iter().take(30) {
        eprintln!("  {r}");
    }
    assert!(refused.is_empty());
}
