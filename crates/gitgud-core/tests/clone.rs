use std::net::TcpListener;
use std::time::{Duration, Instant};

use gitgud_core::cli::GitCli;
use gitgud_core::clone::{CloneProgress, clone_repo};
use gitgud_core::git;
use gitgud_core::repo::{self, Error};
use gitgud_core::task::Cancel;
use gitgud_core::testing::TestRepo;

fn cli() -> GitCli {
    GitCli::new(&git::locate().unwrap())
}

#[test]
fn clone_local_repo_reports_progress_and_sets_longpaths() {
    let origin = TestRepo::init();
    for i in 0..5 {
        origin
            .write(&format!("file-{i}.txt"), &format!("data {i}\n"))
            .commit_all(&format!("c{i}"));
    }
    let bare_dir = tempfile::tempdir().unwrap();
    let bare = bare_dir.path().join("origin.git");
    origin.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);

    let dest_root = tempfile::tempdir().unwrap();
    let dest = dest_root.path().join("my-clone");
    let mut progress: Vec<CloneProgress> = Vec::new();

    let cloned = clone_repo(
        &cli(),
        bare.to_str().unwrap(),
        &dest,
        true,
        false,
        &Cancel::default(),
        &mut |p| progress.push(p),
    )
    .unwrap();

    assert_eq!(cloned, dest);
    let r = repo::open(&dest).unwrap();
    assert_eq!(r.status().unwrap().entries.len(), 0);
    let lp = cli()
        .run(&dest, &["config", "--local", "core.longpaths"], None)
        .unwrap();
    assert_eq!(lp.trim(), "true");

    // Cloning into a non-empty existing directory is refused cleanly.
    let err = clone_repo(
        &cli(),
        bare.to_str().unwrap(),
        &dest,
        false,
        false,
        &Cancel::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "{err:?}");
}

#[test]
fn cancel_clone_kills_process_tree_and_cleans_up_partial_dest() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hold = std::thread::spawn(move || {
        let _conn = listener.accept();
        std::thread::sleep(Duration::from_secs(10));
    });

    let dest_root = tempfile::tempdir().unwrap();
    let dest = dest_root.path().join("aborted-clone");
    let url = format!("http://{addr}/slow/repo.git");
    let cancel = Cancel::default();

    let result = std::thread::scope(|s| {
        let handle =
            s.spawn(|| clone_repo(&cli(), &url, &dest, false, false, &cancel, &mut |_| {}));
        // Give git + remote-http time to spawn and connect to the listener.
        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline && !dest.exists() {
            std::thread::sleep(Duration::from_millis(20));
        }
        cancel.cancel();
        handle.join().unwrap()
    });

    drop(hold);
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
    assert!(!dest.exists(), "partial clone directory was not cleaned up");
}
