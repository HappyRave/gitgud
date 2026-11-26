use gitgud_core::compare::{Mode, Point};
use gitgud_core::repo::{Change, Error};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn rev(r: &str) -> Point {
    Point::Rev { rev: r.into() }
}

fn files(c: &gitgud_core::compare::Comparison) -> Vec<(String, Change)> {
    c.files.iter().map(|f| (f.path.clone(), f.change)).collect()
}

/// main: a, then `shared.txt` changed after Feature split off; Feature: adds b, edits a.
fn diverged() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("shared.txt", "one\n")
        .commit_all("base");
    repo.git(&["tag", "v1"]);
    repo.git(&["checkout", "-q", "-b", "Feature"]);
    repo.write("b.txt", "b\n").commit_all("feature: b");
    repo.write("a.txt", "a by feature\n")
        .commit_all("feature: a");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("shared.txt", "two\n").commit_all("main: shared");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

#[test]
fn what_b_brings_ignores_what_a_did_since_they_split() {
    let (repo, session, id) = diverged();
    let c = session
        .compare(id, &rev("main"), &rev("Feature"), Mode::Brings)
        .unwrap();
    assert_eq!(c.mode, Mode::Brings);
    assert_eq!(
        c.base.as_deref(),
        Some(repo.git(&["rev-parse", "v1"]).trim())
    );
    assert_eq!(c.from, c.base.clone().unwrap());
    assert_eq!(
        files(&c),
        [
            ("a.txt".into(), Change::Modified),
            ("b.txt".into(), Change::Added)
        ]
    );
    assert_eq!(c.only_b.count, 2);
    assert_eq!(c.only_b.commits[0].subject, "feature: a");
    assert_eq!(c.only_a.count, 1);
    assert_eq!(c.only_a.commits[0].subject, "main: shared");

    let snap = session
        .compare(id, &rev("main"), &rev("Feature"), Mode::Snapshot)
        .unwrap();
    assert_eq!(snap.mode, Mode::Snapshot);
    assert_eq!(snap.from, snap.a);
    assert_eq!(
        files(&snap),
        [
            ("a.txt".into(), Change::Modified),
            ("b.txt".into(), Change::Added),
            ("shared.txt".into(), Change::Modified)
        ],
        "the snapshot difference includes main's own change"
    );
}

#[test]
fn a_merged_branch_brings_nothing() {
    let (repo, session, id) = diverged();
    repo.git(&["merge", "-q", "--no-edit", "Feature"]);
    let c = session
        .compare(id, &rev("main"), &rev("Feature"), Mode::Brings)
        .unwrap();
    assert!(c.files.is_empty());
    assert_eq!(c.only_b.count, 0);
    assert_eq!(
        c.only_a.count, 1,
        "main's own commit; the merge isn't listed"
    );
    let snap = session
        .compare(id, &rev("main"), &rev("Feature"), Mode::Snapshot)
        .unwrap();
    assert_eq!(files(&snap), [("shared.txt".into(), Change::Modified)]);
}

#[test]
fn the_working_copy_compares_as_a_snapshot_untracked_files_included() {
    let (repo, session, id) = diverged();
    repo.write("a.txt", "edited\n")
        .write("new.txt", "new\n")
        .remove("shared.txt");
    let c = session
        .compare(id, &rev("v1"), &Point::Worktree, Mode::Brings)
        .unwrap();
    assert_eq!(
        c.mode,
        Mode::Brings,
        "v1 is an ancestor of HEAD: brings = from v1"
    );
    assert_eq!(
        files(&c),
        [
            ("a.txt".into(), Change::Modified),
            ("new.txt".into(), Change::Added),
            ("shared.txt".into(), Change::Deleted)
        ]
    );
    let back = session
        .compare(id, &Point::Worktree, &rev("v1"), Mode::Brings)
        .unwrap();
    assert_eq!(back.mode, Mode::Snapshot, "a working copy A has no history");
    assert_eq!(
        files(&back),
        [
            ("a.txt".into(), Change::Modified),
            ("new.txt".into(), Change::Deleted),
            ("shared.txt".into(), Change::Added)
        ]
    );
    let diff = session
        .compare_file_diff(id, &rev("v1"), &Point::Worktree, "a.txt", false)
        .unwrap();
    let text: Vec<&str> = diff.hunks[0]
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(text, ["a", "edited"]);
}

#[test]
fn unrelated_histories_fall_back_to_the_snapshot() {
    let (repo, session, id) = diverged();
    repo.git(&["checkout", "-q", "--orphan", "Other"]);
    repo.git(&["rm", "-q", "-rf", "."]);
    repo.write("z.txt", "z\n").commit_all("other root");
    repo.git(&["checkout", "-q", "main"]);
    let c = session
        .compare(id, &rev("main"), &rev("Other"), Mode::Brings)
        .unwrap();
    assert_eq!(c.base, None);
    assert_eq!(c.mode, Mode::Snapshot);
    assert!(files(&c).contains(&("z.txt".into(), Change::Added)));
}

#[test]
fn a_side_that_is_not_a_commit_is_refused() {
    let (_repo, session, id) = diverged();
    for bad in ["nope", "--help", ""] {
        assert!(
            matches!(
                session.compare(id, &rev(bad), &rev("main"), Mode::Brings),
                Err(Error::Refused(_))
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn file_diffs_between_any_two_commits() {
    let (_repo, session, id) = diverged();
    let d = session
        .compare_file_diff(id, &rev("main"), &rev("Feature"), "a.txt", false)
        .unwrap();
    let lines: Vec<&str> = d.hunks[0].lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(lines, ["a", "a by feature"]);
    assert!(matches!(
        session.compare_file_diff(id, &rev("main"), &rev("Feature"), "gone.txt", false),
        Err(Error::Read(_))
    ));
}

#[test]
fn unity_yaml_diffs_structurally_between_two_commits() {
    let repo = TestRepo::init();
    let doc =
        |v: u32| format!("%YAML 1.1\n--- !u!1 &100\nGameObject:\n  m_Name: Ship\n  m_Speed: {v}\n");
    repo.write("Ship.prefab", &doc(1)).commit_all("one");
    repo.git(&["tag", "one"]);
    repo.write("Ship.prefab", &doc(2)).commit_all("two");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let d = session
        .compare_unity_diff(id, &rev("one"), &rev("main"), "Ship.prefab")
        .unwrap();
    assert!(
        format!("{d:?}").contains("m_Speed"),
        "the changed property is in the structural diff: {d:?}"
    );
}

#[test]
fn taking_b_s_version_is_staged_and_undoable() {
    let (repo, session, id) = diverged();
    session.take_version(id, "Feature", "a.txt").unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "a by feature\n"
    );
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "M  a.txt");

    session.take_version(id, "v1", "shared.txt").unwrap();
    session
        .take_version(id, "main", "b.txt")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        std::fs::read_to_string(repo.path().join("shared.txt")).unwrap(),
        "one\n"
    );

    session.undo(id).unwrap();
    session.undo(id).unwrap();
    session.undo(id).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "a\n"
    );
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "");
}

#[test]
fn taking_a_version_without_the_file_deletes_it() {
    let (repo, session, id) = diverged();
    repo.git(&["checkout", "-q", "Feature"]);
    session.refresh(id).unwrap();
    session.take_version(id, "main", "b.txt").unwrap();
    assert!(!repo.path().join("b.txt").exists());
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "D  b.txt");
}

/// Times a real comparison: `GITGUD_COMPARE_CORPUS=<repo> GITGUD_COMPARE_A=<rev>
/// GITGUD_COMPARE_B=<rev> cargo test -p gitgud-core --test compare -- --ignored --nocapture`.
#[test]
#[ignore = "needs GITGUD_COMPARE_CORPUS"]
fn compares_a_real_repo() {
    let root = std::env::var("GITGUD_COMPARE_CORPUS").expect("set GITGUD_COMPARE_CORPUS");
    let a = std::env::var("GITGUD_COMPARE_A").expect("set GITGUD_COMPARE_A");
    let b = std::env::var("GITGUD_COMPARE_B").expect("set GITGUD_COMPARE_B");
    let session = Session::default();
    let id = session.open(&root).unwrap().id;
    for mode in [Mode::Brings, Mode::Snapshot] {
        let started = std::time::Instant::now();
        let c = session.compare(id, &rev(&a), &rev(&b), mode).unwrap();
        println!(
            "{root} {a} → {b} {mode:?}: {} files · only A {} · only B {} · {:?}",
            c.files.len(),
            c.only_a.count,
            c.only_b.count,
            started.elapsed()
        );
    }
}
