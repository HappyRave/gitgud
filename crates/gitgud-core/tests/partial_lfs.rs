use std::path::Path;

use gitgud_core::partial_lfs::PartialState;
use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

const POINTER: &str = "version https://git-lfs.github.com/spec/v1\n";

fn guid(n: u8) -> String {
    format!("{n:032x}")
}

fn meta(repo: &TestRepo, path: &str, n: u8) {
    repo.write(
        &format!("{path}.meta"),
        &format!("fileFormatVersion: 2\nguid: {}\n", guid(n)),
    );
}

fn refs(ids: &[u8]) -> String {
    let mut out = String::from("%YAML 1.1\n--- !u!1 &1\nGameObject:\n");
    for &n in ids {
        out.push_str(&format!(
            "  m_Ref: {{fileID: 2100000, guid: {}, type: 2}}\n",
            guid(n)
        ));
    }
    out
}

/// A Unity project pushed with its LFS files to a bare origin, and a clone of it whose
/// LFS files are all pointers, open in a session.
///
/// Main.unity (LFS) → Ship.prefab (LFS) → Hull.mat (text) → Hull.png, and Ship.prefab →
/// Pack/Used.png. Old.unity is disabled in Build Settings and alone uses Old.png.
/// Pack/Unused.png nothing uses. A DLL and a Resources texture Unity loads anyway.
fn partial_clone() -> (TestRepo, tempfile::TempDir, TestRepo, Session, u32) {
    let origin = TestRepo::init();
    origin.git(&["lfs", "install", "--local"]);
    origin
        .write(".gitattributes", "*.png filter=lfs diff=lfs merge=lfs -text\n*.unity filter=lfs diff=lfs merge=lfs -text\n*.prefab filter=lfs diff=lfs merge=lfs -text\n*.dll filter=lfs diff=lfs merge=lfs -text\n")
        .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 2022.3.0f1\n")
        .write(
            "ProjectSettings/EditorBuildSettings.asset",
            "EditorBuildSettings:\n  m_Scenes:\n  - enabled: 1\n    path: Assets/Scenes/Main.unity\n    guid: 00000000000000000000000000000001\n  - enabled: 0\n    path: Assets/Scenes/Old.unity\n    guid: 00000000000000000000000000000002\n",
        )
        .write("Assets/Scenes/Main.unity", &refs(&[3]))
        .write("Assets/Scenes/Old.unity", &refs(&[7]))
        .write("Assets/Ship.prefab", &refs(&[4, 6]))
        .write("Assets/Hull.mat", &refs(&[5]))
        .write("Assets/Hull.png", "hull pixels")
        .write("Assets/Pack/Used.png", "used pixels")
        .write("Assets/Pack/Unused.png", "unused pixels")
        .write("Assets/Old.png", "old pixels")
        .write("Assets/Plugins/native.dll", "native code")
        .write("Assets/UI/Resources/Logo.png", "logo pixels");
    for (path, n) in [
        ("Assets/Scenes/Main.unity", 1),
        ("Assets/Scenes/Old.unity", 2),
        ("Assets/Ship.prefab", 3),
        ("Assets/Hull.mat", 4),
        ("Assets/Hull.png", 5),
        ("Assets/Pack/Used.png", 6),
        ("Assets/Old.png", 7),
        ("Assets/Pack/Unused.png", 8),
        ("Assets/Plugins/native.dll", 9),
        ("Assets/UI/Resources/Logo.png", 10),
    ] {
        meta(&origin, path, n);
    }
    origin.commit_all("project");
    let bare_dir = tempfile::tempdir().unwrap();
    let bare = bare_dir.path().join("origin.git");
    std::process::Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&bare)
        .status()
        .unwrap();
    origin.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    origin.git(&["push", "-q", "origin", "main"]);

    let clone = TestRepo::init();
    clone.git(&["lfs", "install", "--local"]);
    clone.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    clone.git(&["fetch", "-q", "origin"]);
    clone.git(&[
        "-c",
        "filter.lfs.process=git-lfs filter-process --skip",
        "-c",
        "filter.lfs.smudge=git-lfs smudge --skip -- %f",
        "checkout",
        "-q",
        "-B",
        "main",
        "origin/main",
    ]);
    let session = Session::default();
    let id = session.open(clone.path()).unwrap().id;
    (origin, bare_dir, clone, session, id)
}

fn read(repo: &TestRepo, path: &str) -> String {
    std::fs::read_to_string(repo.path().join(path)).unwrap()
}

fn is_pointer(repo: &TestRepo, path: &str) -> bool {
    read(repo, path).starts_with(POINTER)
}

#[test]
fn downloads_what_the_build_scenes_use_and_what_unity_always_loads() {
    let (_origin, _bare, clone, session, id) = partial_clone();
    assert!(is_pointer(&clone, "Assets/Hull.png"));

    let plan = session.lfs_partial_plan(id, None).unwrap();
    assert!(!plan.partial);
    assert_eq!(plan.build_scenes, ["Assets/Scenes/Main.unity"]);
    assert_eq!(plan.total.files, 9);
    // Main.unity, Ship.prefab, Hull.png, Used.png.
    assert_eq!(plan.used.files, 4);
    // native.dll, Logo.png.
    assert_eq!(plan.always.files, 2);
    // Old.unity, Old.png, Unused.png.
    assert_eq!(plan.skipped.files, 3);
    assert_eq!(plan.on_disk.files, 0);
    assert_eq!(
        plan.fetched_to_read, 2,
        "Main.unity and Ship.prefab are read to follow them"
    );
    let folders: Vec<&str> = plan
        .skipped_folders
        .iter()
        .map(|f| f.folder.as_str())
        .collect();
    assert!(
        folders.contains(&"Assets/Pack") && folders.contains(&"Assets"),
        "{folders:?}"
    );
    assert!(
        is_pointer(&clone, "Assets/Scenes/Main.unity"),
        "planning leaves the work tree alone"
    );

    let applied = session
        .lfs_partial_apply(id, Some(PartialState::build()), |_| {})
        .unwrap();
    assert!(applied.partial);
    assert_eq!(applied.on_disk.files, 6);
    assert_eq!(read(&clone, "Assets/Hull.png"), "hull pixels");
    assert_eq!(read(&clone, "Assets/Pack/Used.png"), "used pixels");
    assert_eq!(read(&clone, "Assets/Plugins/native.dll"), "native code");
    assert_eq!(read(&clone, "Assets/UI/Resources/Logo.png"), "logo pixels");
    assert!(is_pointer(&clone, "Assets/Pack/Unused.png"));
    assert!(is_pointer(&clone, "Assets/Old.png"));
    assert_eq!(clone.git(&["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_downloaded_folder_is_kept_and_dropping_it_turns_its_files_back_into_pointers() {
    let (_origin, _bare, clone, session, id) = partial_clone();
    assert!(matches!(
        session.lfs_partial_download(id, &["Assets/Pack".into()], |_| {}),
        Err(Error::Refused(_))
    ));
    session
        .lfs_partial_apply(id, Some(PartialState::build()), |_| {})
        .unwrap();

    let plan = session
        .lfs_partial_download(id, &["Assets/Pack/".into()], |_| {})
        .unwrap();
    assert_eq!(plan.state.kept, ["Assets/Pack"]);
    assert_eq!(read(&clone, "Assets/Pack/Unused.png"), "unused pixels");
    assert_eq!(plan.kept.files, 1);
    let include = clone.git(&["config", "lfs.fetchinclude"]);
    assert!(include.contains("Assets/Pack"), "{include}");

    // A local edit is never turned into a pointer.
    clone.write("Assets/Old.png", "my edit");
    session
        .lfs_partial_apply(id, Some(PartialState::build()), |_| {})
        .unwrap();
    assert!(is_pointer(&clone, "Assets/Pack/Unused.png"));
    assert_eq!(read(&clone, "Assets/Pack/Used.png"), "used pixels");
    assert_eq!(read(&clone, "Assets/Old.png"), "my edit");
    assert_eq!(
        clone.git(&["status", "--porcelain"]).trim(),
        "M Assets/Old.png"
    );
}

#[test]
fn whole_again_downloads_everything_and_forgets_the_choice() {
    let (_origin, _bare, clone, session, id) = partial_clone();
    session
        .lfs_partial_apply(id, Some(PartialState::build()), |_| {})
        .unwrap();
    let plan = session.lfs_partial_apply(id, None, |_| {}).unwrap();
    assert!(!plan.partial);
    assert_eq!(plan.on_disk.files, 9);
    assert_eq!(read(&clone, "Assets/Old.png"), "old pixels");
    assert!(!clone.git_ok(&["config", "lfs.fetchinclude"]));
    assert!(!Path::new(&clone.path().join(".git/gitgud/lfs-partial.json")).exists());
}

/// Plans a real project without changing its work tree:
/// `GITGUD_LFS_CORPUS=<repo> cargo test -p gitgud-core --test partial_lfs -- --ignored --nocapture`.
#[test]
#[ignore = "needs GITGUD_LFS_CORPUS"]
fn plans_a_real_project() {
    let root = std::env::var("GITGUD_LFS_CORPUS").expect("set GITGUD_LFS_CORPUS");
    let session = Session::default();
    let id = session.open(&root).unwrap().id;
    let started = std::time::Instant::now();
    let plan = session.lfs_partial_plan(id, None).unwrap();
    let gib = |b: u64| b as f64 / f64::from(1u32 << 30);
    println!(
        "{root}: {} build scenes · total {:.2} GiB / {} · used {:.2} / {} · always {:.2} / {} · skipped {:.2} / {} · read {} from LFS · {:?}",
        plan.build_scenes.len(),
        gib(plan.total.bytes),
        plan.total.files,
        gib(plan.used.bytes),
        plan.used.files,
        gib(plan.always.bytes),
        plan.always.files,
        gib(plan.skipped.bytes),
        plan.skipped.files,
        plan.fetched_to_read,
        started.elapsed()
    );
    for f in plan.skipped_folders.iter().take(8) {
        println!(
            "  {:.2} of {:.2}  {}",
            gib(f.skipped.bytes),
            gib(f.total.bytes),
            f.folder
        );
    }
    let again = std::time::Instant::now();
    let second = session.lfs_partial_plan(id, None).unwrap();
    assert_eq!(second.used, plan.used);
    println!("  planned again in {:?}", again.elapsed());
}

#[test]
fn an_extra_scene_brings_its_files_and_a_missing_one_is_named() {
    let (_origin, _bare, clone, session, id) = partial_clone();
    let state = PartialState {
        build_scenes: true,
        scenes: vec!["Assets/Scenes/Old.unity".into(), "Assets/Gone.unity".into()],
        kept: Vec::new(),
    };
    let plan = session.lfs_partial_apply(id, Some(state), |_| {}).unwrap();
    assert_eq!(read(&clone, "Assets/Old.png"), "old pixels");
    assert_eq!(plan.missing, ["Assets/Gone.unity"]);
    assert_eq!(plan.skipped.files, 1);
}
