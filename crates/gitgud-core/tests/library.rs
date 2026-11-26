use std::path::Path;
use std::time::Instant;

use gitgud_core::library::{
    AddRepoEntry, DEFAULT_SCAN_DEPTH, LibraryStore, OpenTabState, display_path, scan_folder,
};
use gitgud_core::remote_url::HostKind;
use gitgud_core::testing::TestRepo;

fn add(store: &LibraryStore, path: &Path, folder: &str) {
    store
        .add_repos(&[AddRepoEntry {
            path: path.to_string_lossy().into_owned(),
            folder_path: Some(folder.into()),
            name: None,
        }])
        .unwrap();
}

fn folder_id(store: &LibraryStore, name: &str) -> String {
    let snap = store.snapshot().unwrap();
    snap.state
        .folders
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .id
        .clone()
}

#[test]
fn rescan_finds_new_repos_and_gone_ones_and_applies_in_one_undo() {
    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("Projects");
    let seed = TestRepo::init();
    let init = |p: &Path| {
        seed.git(&["init", "-q", &p.to_string_lossy()]);
    };
    for name in ["keep", "doomed", "old/legacy"] {
        init(&projects.join(name));
    }
    let elsewhere = dir.path().join("Elsewhere/other");
    init(&elsewhere);

    let store = LibraryStore::load(dir.path().join("lib.json")).unwrap();
    add(&store, &projects.join("keep"), "Projects");
    add(&store, &projects.join("doomed"), "Projects");
    add(&store, &projects.join("old/legacy"), "Projects / Old");
    add(&store, &elsewhere, "Elsewhere");

    std::fs::remove_dir_all(projects.join("doomed")).unwrap();
    init(&projects.join("fresh"));
    init(&projects.join("old/newer"));

    let scan = store
        .rescan_folder(&folder_id(&store, "Projects"), DEFAULT_SCAN_DEPTH)
        .unwrap();
    assert_eq!(scan.root, display_path(&projects));
    assert_eq!(
        scan.gone
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["doomed"]
    );
    let fresh: Vec<(&str, &str)> = scan
        .groups
        .iter()
        .flat_map(|g| {
            g.repos
                .iter()
                .filter(|r| !r.already_in_library)
                .map(move |r| (r.name.as_str(), g.suggested_folder.as_str()))
        })
        .collect();
    assert_eq!(fresh, [("fresh", "Projects"), ("newer", "Projects / old")]);
    assert!(
        scan.groups
            .iter()
            .flat_map(|g| &g.repos)
            .all(|r| r.name != "other"),
        "a rescan stays inside the folder's own disk folder"
    );

    let entries: Vec<AddRepoEntry> = scan
        .groups
        .iter()
        .flat_map(|g| {
            g.repos
                .iter()
                .filter(|r| !r.already_in_library)
                .map(|r| AddRepoEntry {
                    path: r.path.clone(),
                    folder_path: Some(g.suggested_folder.clone()),
                    name: None,
                })
        })
        .collect();
    let gone: Vec<String> = scan.gone.iter().map(|g| g.id.clone()).collect();
    let snap = store.apply_scan(&entries, &gone).unwrap();
    let names: Vec<&str> = snap.state.repos.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"fresh") && names.contains(&"newer") && !names.contains(&"doomed"));
    assert_eq!(snap.undo_action.as_deref(), Some("Rescan: add 2, remove 1"));

    let undone = store.undo().unwrap();
    let names: Vec<&str> = undone.state.repos.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["keep", "doomed", "legacy", "other"]);
}

#[test]
fn rescan_of_a_deleted_folder_lists_everything_gone() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("Games");
    let seed = TestRepo::init();
    for name in ["a", "b"] {
        seed.git(&["init", "-q", &games.join(name).to_string_lossy()]);
    }
    let store = LibraryStore::load(dir.path().join("lib.json")).unwrap();
    add(&store, &games.join("a"), "Games");
    add(&store, &games.join("b"), "Games");
    std::fs::remove_dir_all(&games).unwrap();

    let scan = store
        .rescan_folder(&folder_id(&store, "Games"), DEFAULT_SCAN_DEPTH)
        .unwrap();
    assert!(scan.groups.is_empty());
    assert_eq!(scan.gone.len(), 2);

    let ids: Vec<String> = scan.gone.iter().map(|g| g.id.clone()).collect();
    let snap = store.apply_scan(&[], &ids).unwrap();
    assert!(snap.state.repos.is_empty());
    assert!(
        snap.state.folders.iter().any(|f| f.name == "Games"),
        "the emptied folder stays"
    );
    assert_eq!(snap.undo_action.as_deref(), Some("Remove 2 repositories"));
}

#[test]
fn rescan_refuses_folders_it_cannot_place_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = LibraryStore::load(dir.path().join("lib.json")).unwrap();
    store.create_folder("Empty", None).unwrap();
    let err = store
        .rescan_folder(&folder_id(&store, "Empty"), DEFAULT_SCAN_DEPTH)
        .unwrap_err();
    assert!(err.to_string().contains("no repositories"), "{err}");
    assert!(store.rescan_folder("f-nope", DEFAULT_SCAN_DEPTH).is_err());
}

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
    repo_b.write("b.txt", "edited");

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
    assert_eq!(sum_b.dirty_count, 2);
    assert_eq!(sum_b.untracked_count, 1);

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

fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn library_refreshes_150_repos_cleanly_and_fast() {
    let dir = tempfile::tempdir().unwrap();
    let template = TestRepo::init();
    template.write("README.md", "template\n").commit_all("init");

    let unity_template = TestRepo::init();
    unity_template
        .write("README.md", "unity\n")
        .write(
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.3.21f1\n",
        )
        .commit_all("init unity");

    let mut entries = Vec::with_capacity(150);
    for i in 0..150 {
        let repo_dir = dir.path().join(format!("repo-{i:03}"));
        let src = if i % 5 == 0 {
            unity_template.path()
        } else {
            template.path()
        };
        copy_dir_all(src, &repo_dir);
        if i % 10 == 0 {
            std::fs::write(repo_dir.join("dirty.txt"), "wip\n").unwrap();
        }
        entries.push(AddRepoEntry {
            path: repo_dir.to_string_lossy().into_owned(),
            folder_path: Some(format!("Studio / Group {}", i / 30)),
            name: Some(format!("repo-{i:03}")),
        });
    }

    let store = LibraryStore::load(dir.path().join("library.json")).unwrap();
    store.add_repos(&entries).unwrap();

    let t = Instant::now();
    let sums = store.refresh_summaries(&[]).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(sums.len(), 150);
    assert_eq!(sums.iter().filter(|s| s.missing).count(), 0);
    assert_eq!(sums.iter().filter(|s| s.is_unity).count(), 30);
    assert_eq!(sums.iter().filter(|s| s.dirty_count > 0).count(), 15);
    println!("perf 150-repo library refresh: {ms:.1} ms");
}
