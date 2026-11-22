use std::time::Instant;

use gitgud_core::session::Session;
use gitgud_core::snapshot::SNAPSHOT_REF;
use gitgud_core::testing::TestRepo;
use gitgud_core::worktree::{HunkHeader, Side};

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

fn setup() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write(".gitignore", "*.log\n")
        .write("a.txt", "a\n")
        .write("b.txt", "b\n")
        .commit_all("init");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

fn discard(session: &Session, id: u32, ps: &[&str]) {
    let ps = paths(ps);
    session
        .write_destructive(id, "Discard", &ps, |r, g| r.discard(g, &ps))
        .unwrap();
}

#[test]
fn discard_a_change_then_undo_and_redo_it() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "precious edit\n");

    discard(&session, id, &["a.txt"]);
    assert_eq!(read(&repo, "a.txt").unwrap(), "a\n");

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "precious edit\n");

    session.redo(id).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "a\n");
}

#[test]
fn discard_keeps_what_is_staged() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "staged\n").git(&["add", "a.txt"]);
    repo.write("a.txt", "staged plus more\n");

    discard(&session, id, &["a.txt"]);

    assert_eq!(read(&repo, "a.txt").unwrap(), "staged\n");
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "a.txt"
    );
}

#[test]
fn discard_an_untracked_file_deletes_it_and_undo_brings_it_back() {
    let (repo, session, id) = setup();
    repo.write("notes/idea.txt", "only copy\n");

    discard(&session, id, &["notes/idea.txt"]);
    assert_eq!(read(&repo, "notes/idea.txt"), None);

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "notes/idea.txt").unwrap(), "only copy\n");
}

#[test]
fn discard_a_deletion_restores_the_file() {
    let (repo, session, id) = setup();
    repo.remove("b.txt");

    discard(&session, id, &["b.txt"]);
    assert_eq!(read(&repo, "b.txt").unwrap(), "b\n");

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "b.txt"), None);
}

#[test]
fn discard_one_hunk_and_undo_it() {
    let (repo, session, id) = setup();
    let lines: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    repo.write("f.txt", &lines).commit_all("f");
    let edited = lines
        .replace("line 2\n", "two\n")
        .replace("line 28\n", "twenty-eight\n");
    repo.write("f.txt", &edited);
    let open = session.get(id).unwrap();
    let hunk = HunkHeader::from(
        &open
            .repo
            .worktree_file_diff("f.txt", Side::Unstaged)
            .unwrap()
            .hunks[0],
    );

    let ps = paths(&["f.txt"]);
    session
        .write_destructive(id, "Discard hunk", &ps, |r, g| {
            r.discard_hunk(g, "f.txt", hunk)
        })
        .unwrap();
    assert_eq!(
        read(&repo, "f.txt").unwrap(),
        lines.replace("line 28\n", "twenty-eight\n")
    );

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "f.txt").unwrap(), edited);
}

#[test]
fn discard_a_hunk_in_an_autocrlf_file_keeps_crlf() {
    let repo = TestRepo::init();
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.write("w.txt", "one\r\ntwo\r\nthree\r\n")
        .commit_all("init");
    repo.write("w.txt", "one\r\nTWO\r\nthree\r\n");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let open = session.get(id).unwrap();
    let hunk = HunkHeader::from(
        &open
            .repo
            .worktree_file_diff("w.txt", Side::Unstaged)
            .unwrap()
            .hunks[0],
    );

    let ps = paths(&["w.txt"]);
    session
        .write_destructive(id, "Discard hunk", &ps, |r, g| {
            r.discard_hunk(g, "w.txt", hunk)
        })
        .unwrap();

    assert_eq!(read(&repo, "w.txt").unwrap(), "one\r\ntwo\r\nthree\r\n");
}

#[test]
fn refuses_to_undo_a_discard_once_the_file_was_edited_again() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "first edit\n");
    discard(&session, id, &["a.txt"]);
    repo.write("a.txt", "a newer edit\n");

    let err = session.undo(id).unwrap_err();

    assert!(err.to_string().contains("changed outside gitgud"), "{err}");
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "a newer edit\n",
        "the newer edit is kept"
    );
}

#[test]
fn snapshots_leave_the_real_index_alone_and_skip_ignored_files() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "edit\n")
        .write("debug.log", "ignored noise\n");
    let index_before = std::fs::read(repo.path().join(".git/index")).unwrap();

    discard(&session, id, &["a.txt"]);

    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]).trim(), "");
    let snapshot_tree = repo.git(&[
        "ls-tree",
        "-r",
        "--name-only",
        &format!("{SNAPSHOT_REF}^{{tree}}"),
    ]);
    assert!(!snapshot_tree.contains("debug.log"), "{snapshot_tree}");
    assert!(snapshot_tree.contains("a.txt"));
    let count = repo.git(&["rev-list", "--count", SNAPSHOT_REF]);
    assert_eq!(count.trim(), "2", "a snapshot before and after");
    assert_ne!(index_before, Vec::<u8>::new());
    assert_eq!(read(&repo, "debug.log").unwrap(), "ignored noise\n");
}

#[test]
fn snapshot_content_survives_gc() {
    let (repo, session, id) = setup();
    repo.write("draft.txt", "unique draft content 8f2c\n");
    discard(&session, id, &["draft.txt"]);

    repo.git(&["reflog", "expire", "--expire=now", "--all"]);
    repo.git(&["gc", "-q", "--prune=now"]);

    session.undo(id).unwrap();
    assert_eq!(
        read(&repo, "draft.txt").unwrap(),
        "unique draft content 8f2c\n"
    );
}

#[test]
fn undo_goes_back_through_mixed_actions() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "keep me\n")
        .write("b.txt", "stage me\n");
    discard(&session, id, &["a.txt"]);
    let ps = paths(&["b.txt"]);
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &ps))
        .unwrap();

    session.undo(id).unwrap();
    session.undo(id).unwrap();

    assert_eq!(read(&repo, "a.txt").unwrap(), "keep me\n");
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]).trim(), "");
}

#[test]
fn op_log_lists_entries_newest_first_with_their_files() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "x\n").write("b.txt", "y\n");
    let ps = paths(&["b.txt"]);
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &ps))
        .unwrap();
    discard(&session, id, &["a.txt"]);
    session.undo(id).unwrap();

    let log = session.op_log(id).unwrap();

    let summary: Vec<_> = log
        .iter()
        .map(|e| (e.action.as_str(), e.undone, e.paths.clone()))
        .collect();
    assert_eq!(
        summary,
        [
            ("Discard", true, paths(&["a.txt"])),
            ("Stage b.txt", false, vec![])
        ]
    );
    assert!(log[0].time >= log[1].time);
}

#[test]
fn restore_to_an_older_entry_undoes_it_and_everything_after() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "1\n").write("b.txt", "2\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();
    session
        .write(id, "Commit", |r, g| r.commit(g, "both"))
        .unwrap();
    let first = session.op_log(id).unwrap().last().unwrap().id;

    let restored = session.restore_to(id, first).unwrap();

    assert_eq!(restored.action, "Stage a.txt");
    assert_eq!(repo.git(&["log", "--format=%s"]).trim(), "init");
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]).trim(), "");
    assert!(session.op_log(id).unwrap().iter().all(|e| e.undone));
    assert!(
        session
            .restore_to(id, first)
            .unwrap_err()
            .to_string()
            .contains("already undone")
    );

    session.redo(id).unwrap();
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "a.txt"
    );
}

#[test]
fn restore_files_of_an_old_discard_keeps_later_work() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "lost work\n");
    discard(&session, id, &["a.txt"]);
    let discard_id = session.op_log(id).unwrap()[0].id;
    repo.write("b.txt", "later\n");
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();

    session.restore_files(id, discard_id).unwrap();

    assert_eq!(read(&repo, "a.txt").unwrap(), "lost work\n");
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "b.txt",
        "the later stage stays"
    );
    let log = session.op_log(id).unwrap();
    assert_eq!(log[0].action, "Restore files from \"Discard\"");

    session.undo(id).unwrap();
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "a\n",
        "restoring is itself undoable"
    );
}

#[test]
fn restore_files_refuses_entries_that_changed_no_files() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "x\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    let stage_id = session.op_log(id).unwrap()[0].id;

    let err = session.restore_files(id, stage_id).unwrap_err();

    assert!(err.to_string().contains("changed no files"), "{err}");
    assert!(
        session
            .restore_files(id, 999)
            .unwrap_err()
            .to_string()
            .contains("is gone")
    );
}

#[test]
fn a_discard_with_its_snapshots_is_quick_on_a_small_repo() {
    let (repo, session, id) = setup();
    for i in 0..200 {
        repo.write(&format!("src/f{i}.txt"), &format!("{i}\n"));
    }
    repo.commit_all("files");
    repo.write("src/f1.txt", "dirty\n")
        .write("src/f2.txt", "dirty\n");
    session.refresh(id).unwrap();

    let t = Instant::now();
    discard(&session, id, &["src/f1.txt"]);
    let ms = t.elapsed().as_millis();

    println!("discard with two snapshots: {ms} ms");
    // Spec §6: a snapshot ≤ 250 ms with dirty text files; this is two plus the discard.
    assert!(ms < 2_000, "{ms} ms");
}
