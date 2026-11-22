use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use gitgud_core::repo;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::watch::{DEBOUNCE, RepoWatcher};

const FIRES_WITHIN: Duration = Duration::from_secs(5);
/// Long enough that a missed debounce would have fired.
const QUIET: Duration = Duration::from_millis(800);

fn unity_like() -> TestRepo {
    let repo = TestRepo::init();
    repo.write(".gitignore", "Library/\nTemp/\n*.log\n")
        .write("Assets/Scenes/Main.unity", "scene")
        .write("ProjectSettings/ProjectVersion.txt", "6000.0")
        .commit_all("init");
    repo.write("Library/cache.bin", "x");
    repo
}

fn watch(repo: &TestRepo) -> (RepoWatcher, Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let r = Arc::new(repo::open(repo.path()).unwrap());
    let w = RepoWatcher::start(r, move || {
        let _ = tx.send(());
    })
    .unwrap();
    // Let the OS watches settle before the test touches anything.
    std::thread::sleep(Duration::from_millis(100));
    (w, rx)
}

fn fires(rx: &Receiver<()>) -> bool {
    rx.recv_timeout(FIRES_WITHIN).is_ok()
}

fn stays_quiet(rx: &Receiver<()>) -> bool {
    rx.recv_timeout(QUIET).is_err()
}

#[test]
fn a_file_edit_deep_in_the_tree_fires_once() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    repo.write("Assets/Scenes/Main.unity", "edited");

    assert!(fires(&rx));
    assert!(stays_quiet(&rx), "one edit, one callback");
}

#[test]
fn a_burst_of_writes_is_one_callback() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    for i in 0..50 {
        repo.write(&format!("Assets/gen/f{i}.txt"), "x");
    }

    assert!(fires(&rx));
    assert!(stays_quiet(&rx));
}

#[test]
fn ignored_top_level_folders_are_not_watched() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    for i in 0..20 {
        repo.write(&format!("Library/Artifacts/a{i}.bin"), "x");
        repo.write(&format!("Library/b{i}.bin"), "y");
    }

    assert!(stays_quiet(&rx), "Library/ is ignored");
}

#[test]
fn a_commit_from_outside_fires() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    repo.git(&["commit", "-q", "--allow-empty", "-m", "outside"]);

    assert!(fires(&rx));
}

#[test]
fn a_new_branch_or_tag_fires() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    repo.git(&["branch", "topic"]);
    assert!(fires(&rx));
    assert!(stays_quiet(&rx));

    repo.git(&["tag", "v1"]);
    assert!(fires(&rx));
}

#[test]
fn a_new_top_level_folder_is_watched_from_then_on() {
    let repo = unity_like();
    let (_w, rx) = watch(&repo);

    std::fs::create_dir(repo.path().join("Docs")).unwrap();
    assert!(fires(&rx));
    assert!(stays_quiet(&rx));

    repo.write("Docs/readme.md", "hi");
    assert!(fires(&rx), "a file inside the new folder is seen");
}

#[test]
fn paused_changes_are_not_reported() {
    let repo = unity_like();
    let (w, rx) = watch(&repo);

    {
        let _paused = w.pause();
        repo.write("Assets/a.txt", "x");
        std::thread::sleep(DEBOUNCE * 2);
    }

    assert!(stays_quiet(&rx));
}

#[test]
fn dropping_the_watcher_stops_callbacks() {
    let repo = unity_like();
    let (w, rx) = watch(&repo);
    drop(w);

    repo.write("Assets/a.txt", "x");

    assert!(rx.recv_timeout(QUIET).is_err());
}

#[test]
fn session_writes_do_not_echo_through_the_watcher() {
    let repo = unity_like();
    repo.write("Assets/a.txt", "x");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let (tx, rx) = mpsc::channel();
    session
        .watch(id, move || {
            let _ = tx.send(());
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let opened = session
        .write(id, "Stage Assets/a.txt", |r, git| {
            r.stage(git, &["Assets/a.txt".to_owned()])
        })
        .unwrap();

    assert_eq!(opened.id, id);
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "Assets/a.txt"
    );
    // git's own index write may land just after the pause ends; at most one
    // late, harmless callback, never a stream.
    let echoes = std::iter::from_fn(|| rx.recv_timeout(QUIET).ok())
        .take(5)
        .count();
    assert!(echoes <= 1, "{echoes} callbacks after a paused write");

    session.close(id).unwrap();
    repo.write("Assets/b.txt", "y");
    assert!(rx.recv_timeout(QUIET).is_err(), "closing stops the watch");
}
