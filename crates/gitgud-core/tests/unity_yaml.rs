use gitgud_core::repo::{self, Error};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::unity_merge::{MergeOffer, PickChoice, RowPick, Source};
use gitgud_core::unity_yaml::{UnityYamlDiff, commit_unity_diff, worktree_unity_diff};
use gitgud_core::worktree::Side;

#[test]
fn unity_yaml_structural_diff_property_merge_and_integrity_round_trip() {
    let repo = TestRepo::init();
    let guid = "deadbeefdeadbeefdeadbeefdeadbeef";
    let base = format!(
        "%YAML 1.1
--- !u!1 &1
GameObject:
  m_Name: Enemy
--- !u!4 &2
Transform:
  m_GameObject: {{fileID: 1}}
  m_Father: {{fileID: 0}}
  m_LocalPosition: {{x: 0, y: 0, z: 0}}
--- !u!114 &3
MonoBehaviour:
  m_GameObject: {{fileID: 1}}
  m_Script: {{fileID: 11500000, guid: {guid}, type: 3}}
  hp: 50
  speed: 4
"
    );

    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("Assets/Scripts/EnemyAI.cs", "class EnemyAI {}")
    .write(
        "Assets/Scripts/EnemyAI.cs.meta",
        &format!("fileFormatVersion: 2\nguid: {guid}\n"),
    )
    .write("Assets/Prefabs/Enemy.prefab", &base)
    .commit_all("initial enemy");

    let default_branch = repo
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned();

    repo.git(&["checkout", "-b", "feature"]);
    repo.write(
        "Assets/Prefabs/Enemy.prefab",
        &base
            .replace("hp: 50", "hp: 80")
            .replace("speed: 4", "speed: 9"),
    )
    .commit_all("feature tuning");

    repo.git(&["checkout", &default_branch]);
    repo.write(
        "Assets/Prefabs/Enemy.prefab",
        &base.replace("hp: 50", "hp: 75"),
    )
    .commit_all("master tuning");

    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let cdiff = session
        .commit_unity_diff(id, &head, "Assets/Prefabs/Enemy.prefab")
        .unwrap();
    assert_eq!(cdiff.objects[0].object_path, "Enemy");
    assert_eq!(cdiff.objects[0].component, "EnemyAI.cs");

    repo.write(
        "Assets/Prefabs/Enemy.prefab",
        &base
            .replace("hp: 50", "hp: 75")
            .replace("speed: 4", "speed: 6"),
    );
    let wdiff = session
        .worktree_unity_diff(id, "Assets/Prefabs/Enemy.prefab", Side::Unstaged)
        .unwrap();
    assert_eq!(wdiff.modified_properties, 1);
    assert_eq!(wdiff.objects[0].properties[0].property, "speed");

    repo.git(&["checkout", "--", "Assets/Prefabs/Enemy.prefab"]);
    session.merge(id, "feature").unwrap();

    let table = session
        .property_merge(id, "Assets/Prefabs/Enemy.prefab")
        .unwrap();
    let props: Vec<(&str, Source)> = table
        .rows
        .iter()
        .map(|r| (r.property.as_str(), r.source))
        .collect();
    assert_eq!(props, [("hp", Source::Clash), ("speed", Source::Theirs)]);
    assert_eq!(table.rows[0].component, "EnemyAI.cs");

    let picks = [RowPick {
        id: table.rows[0].id.clone(),
        choice: PickChoice::Custom,
        value: Some("90".into()),
    }];
    let check = session
        .property_merge_check(id, "Assets/Prefabs/Enemy.prefab", &picks)
        .unwrap();
    assert_eq!(check.unresolved, 0);
    assert!(matches!(
        session.resolve_property_merge(id, "Assets/Prefabs/Enemy.prefab", &[]),
        Err(Error::Refused(m)) if m.contains("1 clash still needs a pick")
    ));
    session
        .resolve_property_merge(id, "Assets/Prefabs/Enemy.prefab", &picks)
        .unwrap();

    let issues = session
        .unity_post_merge_check(id, &["Assets/Prefabs/Enemy.prefab".into()])
        .unwrap();
    assert!(issues.is_empty());

    let merged_disk =
        std::fs::read_to_string(repo.path().join("Assets/Prefabs/Enemy.prefab")).unwrap();
    assert!(merged_disk.contains("hp: 90"));
    assert!(merged_disk.contains("speed: 9"));
}

#[test]
fn complex_unity_prefab_variants_nested_instances_stripped_and_flow_collections() {
    let repo = TestRepo::init();
    let script_guid = "cafebabecafebabecafebabecafebabe";
    let nested_guid = "feedfacefeedfacefeedfacefeedface";

    let scene = |tint: &str, mod_x: &str, child_ref: &str| {
        format!(
            "%YAML 1.1
--- !u!1 &10
GameObject:
  m_Name: ArenaRoot
  m_TagString: Untagged
--- !u!4 &11
Transform:
  m_GameObject: {{fileID: 10}}
  m_Father: {{fileID: 0}}
  m_Children:
  - {{fileID: {child_ref}}}
  - {{fileID: 102}}
--- !u!1001 &200
PrefabInstance:
  m_ObjectHideFlags: 0
  serializedVersion: 2
  m_Modification:
    m_TransformParent: {{fileID: 11}}
    m_Modifications:
    - target: {{fileID: 100001, guid: {nested_guid}, type: 3}}
      propertyPath: m_Name
      value: BossVariant
      objectReference: {{fileID: 0}}
    - target: {{fileID: 100002, guid: {nested_guid}, type: 3}}
      propertyPath: m_LocalPosition.x
      value: {mod_x}
      objectReference: {{fileID: 0}}
    m_RemovedComponents: []
  m_SourcePrefab: {{fileID: 100100000, guid: {nested_guid}, type: 3}}
--- !u!1 &101 stripped
GameObject:
  m_CorrespondingSourceObject: {{fileID: 100001, guid: {nested_guid}, type: 3}}
  m_PrefabInstance: {{fileID: 200}}
--- !u!4 &102 stripped
Transform:
  m_CorrespondingSourceObject: {{fileID: 100002, guid: {nested_guid}, type: 3}}
  m_PrefabInstance: {{fileID: 200}}
  m_Father: {{fileID: 11}}
--- !u!114 &300
MonoBehaviour:
  m_GameObject: {{fileID: 10}}
  m_Script: {{fileID: 11500000, guid: {script_guid}, type: 3}}
  tint: {tint}
  tags:
  - Arena
  phases: [Intro, Enrage]
"
        )
    };

    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("Assets/Scripts/ArenaCtrl.cs", "class ArenaCtrl {}")
    .write(
        "Assets/Scripts/ArenaCtrl.cs.meta",
        &format!("fileFormatVersion: 2\nguid: {script_guid}\n"),
    )
    .write("Assets/Prefabs/BossBase.prefab", "base")
    .write(
        "Assets/Prefabs/BossBase.prefab.meta",
        &format!("fileFormatVersion: 2\nguid: {nested_guid}\n"),
    )
    .write(
        "Assets/Scenes/BossArena.unity",
        &scene("{r: 1, g: 0.5, b: 0, a: 1}", "1.5", "301"),
    )
    .write("Assets/Scenes/BossArena.unity~placeholder", "")
    .commit_all("base arena");

    // Add referenced child Transform &301 and merge branches with stripped/flow/mod diffs.
    let extra =
        "--- !u!4 &301\nTransform:\n  m_GameObject: {fileID: 10}\n  m_Father: {fileID: 11}\n";
    let base_full = format!(
        "{}{extra}",
        scene("{r: 1, g: 0.5, b: 0, a: 1}", "1.5", "301")
    );
    repo.write("Assets/Scenes/BossArena.unity", &base_full)
        .commit_all("valid refs");

    let default_branch = repo
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned();

    repo.git(&["checkout", "-q", "-b", "theirs"]);
    repo.write(
        "Assets/Scenes/BossArena.unity",
        &format!(
            "{}{extra}",
            scene("{r: 0.2, g: 0.8, b: 1, a: 1}", "3.25", "301")
        ),
    )
    .commit_all("theirs arena");

    repo.git(&["checkout", "-q", &default_branch]);
    repo.write(
        "Assets/Scenes/BossArena.unity",
        &format!(
            "{}{extra}",
            scene("{r: 0.9, g: 0.1, b: 0.1, a: 1}", "1.5", "102")
        ),
    )
    .commit_all("yours arena");

    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    // Structural diff parses nested PrefabInstance + flow collections + sequence item cleanly.
    repo.write(
        "Assets/Scenes/BossArena.unity",
        &format!(
            "{}{extra}",
            scene("{r: 0.9, g: 0.1, b: 0.1, a: 1}", "2.75", "102")
        ),
    );
    let wdiff = session
        .worktree_unity_diff(id, "Assets/Scenes/BossArena.unity", Side::Unstaged)
        .unwrap();
    assert_eq!(wdiff.objects.len(), 1, "{wdiff:#?}");
    assert_eq!(
        wdiff.objects[0].object_path, "ArenaRoot / BossVariant",
        "{wdiff:#?}"
    );
    assert_eq!(
        wdiff.objects[0].properties[0].property,
        "override:m_LocalPosition.x"
    );

    repo.git(&["checkout", "--", "Assets/Scenes/BossArena.unity"]);
    session.merge(id, "theirs").unwrap();

    let table = session
        .property_merge(id, "Assets/Scenes/BossArena.unity")
        .unwrap();
    let clashes: Vec<&str> = table
        .rows
        .iter()
        .filter(|r| r.source == Source::Clash)
        .map(|r| r.property.as_str())
        .collect();
    assert_eq!(clashes, ["tint.r", "tint.g", "tint.b"]);
    assert!(table.rows.iter().any(|r| r.property == "m_LocalPosition.x"
        && r.source == Source::Theirs
        && r.object_path == "ArenaRoot / BossVariant"));
    assert!(
        table
            .rows
            .iter()
            .any(|r| r.property == "m_Children" && r.source == Source::Yours)
    );

    let picks: Vec<RowPick> = table
        .rows
        .iter()
        .filter(|r| r.source == Source::Clash)
        .zip(["0.5", "0.5", "1"])
        .map(|(r, v)| RowPick {
            id: r.id.clone(),
            choice: PickChoice::Custom,
            value: Some(v.into()),
        })
        .collect();
    session
        .resolve_property_merge(id, "Assets/Scenes/BossArena.unity", &picks)
        .unwrap();

    let issues = session
        .unity_post_merge_check(id, &["Assets/Scenes/BossArena.unity".into()])
        .unwrap();
    assert!(issues.is_empty(), "{issues:?}");

    let resolved =
        std::fs::read_to_string(repo.path().join("Assets/Scenes/BossArena.unity")).unwrap();
    assert!(resolved.contains("tint: {r: 0.5, g: 0.5, b: 1, a: 1}"));
    assert!(resolved.contains("value: 3.25"));
    assert!(resolved.contains("  - {fileID: 102}"));
    assert!(resolved.contains("--- !u!1 &101 stripped"));
}

fn lfs_pointer(oid: &str) -> String {
    format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 100\n")
}

fn hydrate(repo: &TestRepo, oid: &str, data: &str) {
    let dir = repo
        .path()
        .join(".git/lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4]);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(oid), data).unwrap();
}

fn hp_change(diff: &UnityYamlDiff) -> (Option<String>, Option<String>) {
    let prop = diff
        .objects
        .iter()
        .flat_map(|o| &o.properties)
        .find(|p| p.property == "hp")
        .expect("hp change in structural diff");
    (prop.before.clone(), prop.after.clone())
}

#[test]
fn structural_diff_reads_lfs_stored_prefabs_from_the_local_cache() {
    let repo = TestRepo::init();
    let yaml = |hp: u32| {
        format!(
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Gun\n--- !u!114 &2\nMonoBehaviour:\n  m_GameObject: {{fileID: 1}}\n  hp: {hp}\n"
        )
    };
    let (a, b) = ("aa".repeat(32), "bb".repeat(32));
    let path = "Assets/Gun.prefab";
    repo.write(path, &lfs_pointer(&a)).commit_all("gun");
    repo.write(path, &lfs_pointer(&b)).commit_all("tune gun");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let r = repo::open(repo.path()).unwrap();

    assert!(matches!(
        commit_unity_diff(&r, &head, path),
        Err(Error::Refused(m)) if m.contains("isn't downloaded")
    ));

    hydrate(&repo, &a, &yaml(50));
    hydrate(&repo, &b, &yaml(80));
    let diff = commit_unity_diff(&r, &head, path).unwrap();
    assert_eq!(hp_change(&diff), (Some("50".into()), Some("80".into())));

    repo.write(path, &yaml(95));
    let diff = worktree_unity_diff(&r, path, Side::Unstaged).unwrap();
    assert_eq!(hp_change(&diff), (Some("80".into()), Some("95".into())));
}

#[test]
fn the_guid_index_is_reused_until_the_repo_refreshes() {
    let repo = TestRepo::init();
    let guid = "0123456789abcdef0123456789abcdef";
    let prefab = |hp: u32| {
        format!(
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Gun\n--- !u!114 &2\nMonoBehaviour:\n  m_GameObject: {{fileID: 1}}\n  m_Script: {{fileID: 11500000, guid: {guid}, type: 3}}\n  hp: {hp}\n"
        )
    };
    let meta = format!("fileFormatVersion: 2\nguid: {guid}\n");
    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("Assets/Old.cs.meta", &meta)
    .write("Assets/Gun.prefab", &prefab(50))
    .commit_all("gun");
    repo.write("Assets/Gun.prefab", &prefab(80));
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let component = || {
        session
            .worktree_unity_diff(id, "Assets/Gun.prefab", Side::Unstaged)
            .unwrap()
            .objects[0]
            .component
            .clone()
    };
    assert_eq!(component(), "Old.cs");

    repo.remove("Assets/Old.cs.meta")
        .write("Assets/New.cs.meta", &meta);
    assert_eq!(component(), "Old.cs", "cached until the repo refreshes");

    session.refresh(id).unwrap();
    assert_eq!(component(), "New.cs");
}

const LIGHTHOUSE: &str = "%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1 &100
GameObject:
  m_Component:
  - component: {fileID: 101}
  - component: {fileID: 102}
  m_Name: Lighthouse
--- !u!4 &101
Transform:
  m_GameObject: {fileID: 100}
  m_LocalPosition: {x: 0, y: 0, z: 0}
  m_Children: []
  m_Father: {fileID: 0}
--- !u!108 &102
Light:
  m_GameObject: {fileID: 100}
  m_Intensity: 1.2
  m_Range: 24
";

/// A repo whose scenes go through git-lfs's real filter, conflicted on `Level.unity`.
fn lfs_scene_conflict(ours: &str, theirs: &str) -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.git(&["lfs", "install", "--local"]);
    repo.write(
        ".gitattributes",
        "*.unity filter=lfs diff=lfs merge=lfs -text\n",
    )
    .write("Assets/Level.unity", LIGHTHOUSE)
    .commit_all("level");
    repo.git(&["checkout", "-q", "-b", "lighting"]);
    repo.write("Assets/Level.unity", theirs)
        .commit_all("relight");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("Assets/Level.unity", ours).commit_all("move");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "lighting").unwrap();
    (repo, session, id)
}

#[test]
fn an_lfs_scene_changed_on_both_sides_merges_by_property() {
    let ours = LIGHTHOUSE.replace("{x: 0, y: 0, z: 0}", "{x: 0, y: 0.35, z: 0}");
    let theirs = LIGHTHOUSE.replace("m_Range: 24", "m_Range: 30");
    let (repo, session, id) = lfs_scene_conflict(&ours, &theirs);
    let path = "Assets/Level.unity";
    let conflict = &session.merge_state(id).unwrap().unwrap().conflicts[0];
    assert_eq!(conflict.kind, gitgud_core::merge::ConflictKind::Binary);

    assert_eq!(
        session.property_merge_offer(id, path).unwrap(),
        MergeOffer::Ready {
            yours: 1,
            theirs: 1,
            clashes: 0,
            objects: 0
        }
    );
    session.resolve_property_merge(id, path, &[]).unwrap();

    let want = ours.replace("m_Range: 24", "m_Range: 30");
    assert_eq!(
        std::fs::read_to_string(repo.path().join(path)).unwrap(),
        want
    );
    let staged = repo.git(&["cat-file", "-p", &format!(":{path}")]);
    assert!(
        staged.starts_with("version https://git-lfs.github.com/spec/v1"),
        "{staged}"
    );
    assert!(
        repo.git(&["status", "--porcelain"])
            .trim()
            .starts_with("M  Assets/Level.unity")
    );

    session.undo(id).unwrap();
    assert!(!session.merge_state(id).unwrap().unwrap().conflicts[0].resolved);
}

#[test]
fn a_missing_lfs_version_is_named_for_fetching() {
    let ours = LIGHTHOUSE.replace("m_Intensity: 1.2", "m_Intensity: 1.6");
    let theirs = LIGHTHOUSE.replace("m_Range: 24", "m_Range: 30");
    let (repo, session, id) = lfs_scene_conflict(&ours, &theirs);
    let theirs_oid = repo.git(&["cat-file", "-p", ":3:Assets/Level.unity"]);
    let oid = theirs_oid
        .lines()
        .find_map(|l| l.strip_prefix("oid sha256:"))
        .unwrap()
        .to_owned();
    let object = repo
        .path()
        .join(".git/lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(&oid);
    std::fs::remove_file(object).unwrap();

    let MergeOffer::NeedsFetch { missing } = session
        .property_merge_offer(id, "Assets/Level.unity")
        .unwrap()
    else {
        panic!("expected NeedsFetch");
    };
    assert_eq!(missing.len(), 1);
    assert_eq!(
        (missing[0].side.as_str(), missing[0].oid.as_str()),
        ("theirs", oid.as_str())
    );
}

#[test]
fn a_binary_serialized_side_is_not_offered() {
    let ours = LIGHTHOUSE.replace("m_Range: 24", "m_Range: 30");
    let (_repo, session, id) = lfs_scene_conflict(&ours, "\u{0}\u{1}binary scene");
    let offer = session
        .property_merge_offer(id, "Assets/Level.unity")
        .unwrap();
    assert!(
        matches!(&offer, MergeOffer::Unavailable { reason } if reason.contains("binary")),
        "{offer:?}"
    );
}
