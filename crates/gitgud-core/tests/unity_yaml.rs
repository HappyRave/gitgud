use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::unity_yaml::UnityPropertyPick;
use gitgud_core::worktree::Side;

#[test]
fn unity_yaml_structural_diff_property_merge_and_integrity_round_trip() {
    let repo = TestRepo::init();
    let guid = "deadbeefdeadbeefdeadbeefdeadbeef";
    let base = format!(
        "%YAML 1.1\n\
--- !u!1 &1\n\
GameObject:\n\
  m_Name: Enemy\n\
--- !u!4 &2\n\
Transform:\n\
  m_GameObject: {{fileID: 1}}\n\
  m_Father: {{fileID: 0}}\n\
  m_LocalPosition: {{x: 0, y: 0, z: 0}}\n\
--- !u!114 &3\n\
MonoBehaviour:\n\
  m_GameObject: {{fileID: 1}}\n\
  m_Script: {{fileID: 11500000, guid: {guid}, type: 3}}\n\
  hp: 50\n\
  speed: 4\n"
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
        .unity_conflict_table(id, "Assets/Prefabs/Enemy.prefab")
        .unwrap();
    assert_eq!(table.conflicts.len(), 1);
    assert_eq!(table.conflicts[0].property, "hp");
    assert_eq!(table.auto_merged.len(), 1);
    assert_eq!(table.auto_merged[0].property, "speed");
    assert_eq!(table.auto_merged[0].side, "theirs");

    session
        .resolve_unity_conflict(
            id,
            "Assets/Prefabs/Enemy.prefab",
            &[UnityPropertyPick {
                file_id: "3".into(),
                property: "hp".into(),
                value: Some("90".into()),
            }],
        )
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
