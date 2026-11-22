use gitgud_core::library::{
    AddRepoEntry, DEFAULT_SCAN_DEPTH, LibraryStore, OpenTabState, scan_folder,
};
use gitgud_core::remote_url::HostKind;
use gitgud_core::testing::TestRepo;

#[test]
fn library_crud_persistence_and_undo_redo() {
    let dir = tempfile::tempdir().unwrap();
    let lib_file = dir.path().join("library.json");

    let repo_a = TestRepo::init();
    repo_a.write("a.txt", "hello").commit_all("initial");
    repo_a.git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/org/alpha.git",
    ]);

    let repo_b = TestRepo::init();
    repo_b.write("b.txt", "world").commit_all("first");
    let unity_dir = repo_b.path().join("ProjectSettings");
    std::fs::create_dir_all(&unity_dir).unwrap();
    std::fs::write(
        unity_dir.join("ProjectVersion.txt"),
        "m_EditorVersion: 6000.3.21f1\n",
    )
    .unwrap();
    repo_b.commit_all("add unity settings");
    repo_b.write("dirty.txt", "wip");

    let store = LibraryStore::load(lib_file.clone()).unwrap();
    let snap = store
        .add_repos(&[
            AddRepoEntry {
                path: repo_a.path().to_string_lossy().into_owned(),
                folder_path: Some("Studio / Arcade".into()),
                name: Some("alpha".into()),
            },
            AddRepoEntry {
                path: repo_b.path().to_string_lossy().into_owned(),
                folder_path: Some("Studio / VR".into()),
                name: None,
            },
            // Duplicate with trailing slash should be ignored.
            AddRepoEntry {
                path: format!("{}/", repo_a.path().display()),
                folder_path: Some("Other".into()),
                name: None,
            },
        ])
        .unwrap();
    assert_eq!(snap.state.repos.len(), 2);
    assert_eq!(snap.state.folders.len(), 3); // Studio, Arcade, VR
    assert!(snap.undo_action.is_some());

    // Local-only summary refresh checks git + unity state accurately.
    let summaries = store.refresh_summaries(&[]).unwrap();
    assert_eq!(summaries.len(), 2);
    let sum_a = summaries
        .iter()
        .find(|s| s.host == HostKind::GitHub)
        .unwrap();
    assert!(!sum_a.missing);
    assert_eq!(sum_a.branch.as_deref(), Some("main"));
    assert_eq!(sum_a.dirty_count, 0);
    assert!(!sum_a.is_unity);

    let sum_b = summaries.iter().find(|s| s.is_unity).unwrap();
    assert!(!sum_b.missing);
    assert_eq!(sum_b.dirty_count, 1);

    // Pin, move, rename folder, delete folder re-parents repos, undo/redo restores.
    let id_a = snap.state.repos[0].id.clone();
    let vr_id = snap
        .state
        .folders
        .iter()
        .find(|f| f.name == "VR")
        .unwrap()
        .id
        .clone();
    let studio_id = snap
        .state
        .folders
        .iter()
        .find(|f| f.name == "Studio")
        .unwrap()
        .id
        .clone();

    assert!(
        store
            .move_folder(&studio_id, Some(vr_id.clone()), 0)
            .is_err()
    );

    let snap = store.pin_repo(&id_a, true).unwrap();
    assert!(
        snap.state
            .repos
            .iter()
            .find(|r| r.id == id_a)
            .unwrap()
            .pinned
    );

    let snap = store.rename_folder(&vr_id, "Virtual Reality").unwrap();
    assert!(
        snap.state
            .folders
            .iter()
            .any(|f| f.id == vr_id && f.name == "Virtual Reality")
    );

    let snap = store.delete_folder(&studio_id).unwrap();
    // Direct child Arcade & VR become top-level (`parent_id == None`).
    assert!(
        snap.state
            .folders
            .iter()
            .filter(|f| f.id == vr_id)
            .all(|f| f.parent_id.is_none())
    );

    // Save open tabs and reopen LibraryStore from disk to confirm persistence.
    store
        .save_tabs(OpenTabState {
            open_paths: vec![sum_a.path.clone()],
            active_path: Some(sum_a.path.clone()),
        })
        .unwrap();

    let undone = store.undo().unwrap();
    assert!(undone.state.folders.iter().any(|f| f.id == studio_id));
    assert_eq!(
        undone.state.tabs.active_path.as_deref(),
        Some(sum_a.path.as_str())
    );

    let redone = store.redo().unwrap();
    assert!(!redone.state.folders.iter().any(|f| f.id == studio_id));

    let reloaded = LibraryStore::load(lib_file).unwrap().snapshot().unwrap();
    assert_eq!(reloaded.state.repos.len(), 2);
    assert_eq!(reloaded.state.tabs.open_paths, vec![sum_a.path.clone()]);
}

#[test]
fn library_locate_missing_repo_and_scan_folder() {
    let dir = tempfile::tempdir().unwrap();
    let scan_root = dir.path().join("Work");
    let arcade = scan_root.join("arcade");
    std::fs::create_dir_all(&arcade).unwrap();

    // Create two repos under Work/arcade and one skipped node_modules/.git decoy.
    let r1 = TestRepo::init();
    r1.write("readme.md", "hi").commit_all("init");
    let dest_r1 = arcade.join("game-one");
    r1.git(&[
        "clone",
        "-q",
        &r1.path().to_string_lossy(),
        &dest_r1.to_string_lossy(),
    ]);

    let dest_r2 = arcade.join("game-two");
    r1.git(&[
        "clone",
        "-q",
        &r1.path().to_string_lossy(),
        &dest_r2.to_string_lossy(),
    ]);

    let ignored = scan_root.join("node_modules/fake-pkg");
    std::fs::create_dir_all(ignored.join(".git")).unwrap();

    let scan1 = scan_folder(&scan_root, &[], DEFAULT_SCAN_DEPTH).unwrap();
    assert_eq!(scan1.total_found, 2);
    assert_eq!(scan1.groups.len(), 1);
    assert_eq!(scan1.groups[0].suggested_folder, "Work / arcade");
    assert!(scan1.groups[0].repos.iter().all(|r| !r.already_in_library));

    // Add game-one, then verify scan marks it already_in_library and missing/locate works.
    let store = LibraryStore::load(dir.path().join("lib.json")).unwrap();
    let missing_path = dir.path().join("gone-repo");
    let added = store
        .add_repos(&[
            AddRepoEntry {
                path: dest_r1.to_string_lossy().into_owned(),
                folder_path: None,
                name: None,
            },
            AddRepoEntry {
                path: missing_path.to_string_lossy().into_owned(),
                folder_path: None,
                name: Some("gone".into()),
            },
        ])
        .unwrap();
    let missing_id = added
        .state
        .repos
        .iter()
        .find(|r| r.name == "gone")
        .unwrap()
        .id
        .clone();

    let sums = store
        .refresh_summaries(&[missing_path.to_string_lossy().into_owned()])
        .unwrap();
    assert!(sums[0].missing);

    let located = store
        .locate_repo(&missing_id, &dest_r2.to_string_lossy())
        .unwrap();
    let relocated = located
        .state
        .repos
        .iter()
        .find(|r| r.id == missing_id)
        .unwrap();
    assert_eq!(relocated.cached.as_ref().map(|c| c.missing), Some(false));

    let scan2 = scan_folder(
        &scan_root,
        &store.existing_paths().unwrap(),
        DEFAULT_SCAN_DEPTH,
    )
    .unwrap();
    assert!(scan2.groups[0].repos.iter().all(|r| r.already_in_library));
}
