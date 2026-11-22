use std::fs;

use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::unity::{
    MetaIssueKind, check_meta_issues, configure_smart_merge, detect_unity, range_touches_assets,
};

fn fake_merge_tool(dir: &std::path::Path) -> std::path::PathBuf {
    let exe = dir.join("fake-unity-merge.cmd");
    let script = "@echo off\r\nrem %1=merge %2=-p %3=base %4=theirs %5=ours %6=out\r\ncopy /Y \"%~5\" \"%~6\" >nul\r\nexit /b 0\r\n";
    let _ = fs::write(&exe, script);
    exe
}

#[test]
fn unity_detection_lockfile_meta_guard_and_smart_merge_setup() {
    let repo = TestRepo::init();
    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("Assets/Scripts/Player.cs", "class Player {}")
    .write(
        "Assets/Scripts/Player.cs.meta",
        "fileFormatVersion: 2\nguid: 11111111111111111111111111111111\n",
    )
    .commit_all("init unity project");

    // Seed a tracked .gitattributes to prove Smart Merge setup leaves it untouched.
    repo.write(
        ".gitattributes",
        "*.png filter=lfs diff=lfs merge=lfs -text\n",
    )
    .commit_all("gitattributes");

    let info_dir = tempfile::tempdir().unwrap();
    let fake_exe = fake_merge_tool(info_dir.path());
    let git = GitCli::new(&locate().unwrap());
    let opened = gitgud_core::repo::open(repo.path()).unwrap();

    let before = detect_unity(&git, &opened).unwrap();
    assert!(before.is_unity);
    assert_eq!(before.editor_version.as_deref(), Some("6000.0.12f1"));
    assert!(!before.editor_open);
    assert!(!before.smart_merge_configured);

    // Lockfile flips editor_open to true.
    repo.write("Temp/UnityLockfile", "");
    assert!(detect_unity(&git, &opened).unwrap().editor_open);

    // Smart Merge configures .git/config + .git/info/attributes ONLY.
    let configured =
        configure_smart_merge(&git, &opened, Some(fake_exe.to_str().unwrap())).unwrap();
    assert!(configured.smart_merge_configured);
    assert_eq!(
        fs::read_to_string(repo.path().join(".gitattributes")).unwrap(),
        "*.png filter=lfs diff=lfs merge=lfs -text\n"
    );
    let info_attr = fs::read_to_string(repo.path().join(".git/info/attributes")).unwrap();
    assert!(info_attr.contains("*.unity merge=unityyamlmerge"));
    assert!(info_attr.contains("*.prefab merge=unityyamlmerge"));

    // .meta guard detects missing .meta, orphan .meta, and GUID collision.
    repo.write("Assets/NoMeta.cs", "class NoMeta {}")
        .write(
            "Assets/Orphan.cs.meta",
            "fileFormatVersion: 2\nguid: 22222222222222222222222222222222\n",
        )
        .write("Assets/Duplicate.cs", "class Duplicate {}")
        .write(
            "Assets/Duplicate.cs.meta",
            "fileFormatVersion: 2\nguid: 11111111111111111111111111111111\n",
        );

    let status = opened.status().unwrap();
    let issues = check_meta_issues(&opened, &status);
    assert!(
        issues
            .iter()
            .any(|i| i.kind == MetaIssueKind::MissingMeta && i.path == "Assets/NoMeta.cs")
    );
    assert!(
        issues
            .iter()
            .any(|i| i.kind == MetaIssueKind::OrphanMeta && i.path == "Assets/Orphan.cs.meta")
    );
    assert!(issues.iter().any(|i| i.kind == MetaIssueKind::GuidCollision && i.path == "Assets/Duplicate.cs.meta"));

    // Auto-pairing on Session::stage stages Foo.cs + Foo.cs.meta together and records it.
    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;
    session
        .stage(sid, &["Assets/Duplicate.cs".to_owned()])
        .unwrap();
    let log = session.op_log(sid).unwrap();
    assert!(log[0].action.contains("Assets/Duplicate.cs.meta"));
    let st = session.get(sid).unwrap().repo.status().unwrap();
    assert!(
        st.entries
            .iter()
            .any(|e| e.path == "Assets/Duplicate.cs.meta" && e.index.is_some())
    );

    assert!(range_touches_assets(&git, repo.path(), "HEAD~1..HEAD").is_ok());
}

#[test]
fn smart_merge_remerge_and_v1_property_verification_gate() {
    let repo = TestRepo::init();
    let doc = |x: &str, speed: &str| {
        format!(
            "--- !u!4 &100\nTransform:\n  m_Name: Ship\n  m_LocalPosition: {{x: {x}, y: 0, z: 0}}\n--- !u!114 &200\nMonoBehaviour:\n  m_Name: Engine\n  speed: {speed}\n"
        )
    };
    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("Assets/Ship.prefab", &doc("0", "10"))
    .commit_all("base prefab");

    repo.git(&["checkout", "-b", "theirs"]);
    repo.write("Assets/Ship.prefab", &doc("0", "30"))
        .commit_all("theirs speed");

    repo.git(&["checkout", "main"]);
    repo.write("Assets/Ship.prefab", &doc("1", "20"))
        .commit_all("ours move + speed");

    // Set up a simple Unity YAML merge driver that merges transform pos=1 and speed=30 cleanly without markers.
    let tool_dir = tempfile::tempdir().unwrap();
    let tool = tool_dir.path().join("smart-merge-sim.cmd");
    let merged_out = doc("1", "30");
    let merged_file = tool_dir.path().join("merged_fixture.prefab");
    fs::write(&merged_file, &merged_out).unwrap();
    fs::write(
        &tool,
        format!(
            "@echo off\r\ncopy /Y \"{}\" \"%~6\" >nul\r\nexit /b 0\r\n",
            merged_file.to_string_lossy()
        ),
    )
    .unwrap();

    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;

    // Force a real conflict first (Smart Merge not configured yet).
    session.merge(sid, "theirs").unwrap();
    let st = session.merge_state(sid).unwrap().unwrap();
    assert_eq!(st.conflicts[0].path, "Assets/Ship.prefab");

    // Configure Smart Merge and remerge the conflicted prefab.
    session
        .unity_configure_smart_merge(sid, Some(tool.to_str().unwrap()))
        .unwrap();
    session.unity_remerge_conflicts(sid).unwrap();

    let st2 = session.merge_state(sid).unwrap().unwrap();
    assert!(st2.conflicts[0].resolved);
    assert_eq!(st2.unity_verify.len(), 1);
    assert_eq!(st2.unity_verify[0].auto_merged.len(), 1);
    assert_eq!(st2.unity_verify[0].verify.len(), 1);
    assert_eq!(st2.unity_verify[0].verify[0].property, "speed");

    // Committing merge is refused until the verify item is confirmed.
    let err = session.merge_commit(sid, "Merge theirs").unwrap_err();
    assert!(matches!(err, Error::Refused(_)));

    // Confirming makes merge_commit succeed.
    session
        .unity_confirm_verify(sid, "Assets/Ship.prefab", Some("200"), Some("speed"), true)
        .unwrap();
    session.merge_commit(sid, "Merge theirs").unwrap();
    assert!(session.merge_state(sid).unwrap().is_none());
}
