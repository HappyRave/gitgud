use std::time::Instant;

use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::worktree::{HunkHeader, Side};

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

fn cached(repo: &TestRepo) -> String {
    repo.git(&["diff", "--cached", "--name-only"])
        .trim()
        .to_owned()
}

fn head(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "HEAD"]).trim().to_owned()
}

fn setup() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("b.txt", "b\n")
        .commit_all("init");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

#[test]
fn undo_and_redo_a_stage() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    assert_eq!(cached(&repo), "a.txt");

    let undone = session.undo(id).unwrap();
    assert_eq!(undone.action, "Stage a.txt");
    assert_eq!(cached(&repo), "");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "changed\n"
    );

    let redone = session.redo(id).unwrap();
    assert_eq!(redone.action, "Stage a.txt");
    assert_eq!(cached(&repo), "a.txt");
}

#[test]
fn undo_a_commit_keeps_the_changes_staged_and_redo_brings_it_back() {
    let (repo, session, id) = setup();
    let before = head(&repo);
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Commit \"change a\"", |r, g| r.commit(g, "change a"))
        .unwrap();
    let committed = head(&repo);
    assert_ne!(committed, before);

    let undone = session.undo(id).unwrap();
    assert_eq!(undone.action, "Commit \"change a\"");
    assert_eq!(head(&repo), before);
    assert_eq!(
        cached(&repo),
        "a.txt",
        "the commit's changes are staged again"
    );
    assert_eq!(
        undone.opened.graph_version, 3,
        "graph rebuilt on stage (WIP row appears), commit, and undo"
    );
    assert!(
        repo.git(&["reflog", "-1", "main"])
            .contains("gitgud: undo Commit \"change a\"")
    );

    session.redo(id).unwrap();
    assert_eq!(head(&repo), committed);
    assert_eq!(cached(&repo), "");
}

#[test]
fn undoes_several_steps_newest_first() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "1\n").write("b.txt", "2\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();

    assert_eq!(session.undo(id).unwrap().action, "Stage b.txt");
    assert_eq!(cached(&repo), "a.txt");
    assert_eq!(session.undo(id).unwrap().action, "Stage a.txt");
    assert_eq!(cached(&repo), "");
    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to undo")
    );

    assert_eq!(session.redo(id).unwrap().action, "Stage a.txt");
    assert_eq!(session.redo(id).unwrap().action, "Stage b.txt");
    assert!(
        session
            .redo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to redo")
    );
}

#[test]
fn a_new_action_after_undo_drops_the_redo() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "1\n").write("b.txt", "2\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session.undo(id).unwrap();
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();

    assert!(session.redo(id).is_err());
    assert_eq!(session.undo(id).unwrap().action, "Stage b.txt");
    assert!(session.undo(id).is_err());
}

#[test]
fn undo_the_first_commit_returns_to_an_unborn_branch() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1\n");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session
        .write(id, "Stage first.txt", |r, g| {
            r.stage(g, &paths(&["first.txt"]))
        })
        .unwrap();
    session
        .write(id, "Commit \"first\"", |r, g| r.commit(g, "first"))
        .unwrap();

    session.undo(id).unwrap();

    assert!(
        !repo.git_ok(&["rev-parse", "-q", "--verify", "HEAD"]),
        "main has no commit again"
    );
    assert_eq!(cached(&repo), "first.txt");
    session.redo(id).unwrap();
    assert_eq!(repo.git(&["log", "--format=%s"]).trim(), "first");
}

#[test]
fn undo_a_hunk_stage() {
    let (repo, session, id) = setup();
    let lines: String = (1..=20).map(|i| format!("l{i}\n")).collect();
    repo.write("f.txt", &lines).commit_all("f");
    repo.write(
        "f.txt",
        &lines.replace("l2\n", "two\n").replace("l18\n", "x\n"),
    );
    let open = session.get(id).unwrap();
    let hunk = HunkHeader::from(
        &open
            .repo
            .worktree_file_diff("f.txt", Side::Unstaged)
            .unwrap()
            .hunks[0],
    );
    session
        .write(id, "Stage hunk in f.txt", |r, g| {
            r.stage_hunk(g, "f.txt", hunk)
        })
        .unwrap();
    assert_eq!(cached(&repo), "f.txt");

    session.undo(id).unwrap();

    assert_eq!(cached(&repo), "");
}

#[test]
fn a_failed_action_is_not_recorded() {
    let (repo, session, id) = setup();
    let err = session
        .write(id, "Commit \"nothing\"", |r, g| r.commit(g, "nothing"))
        .unwrap_err();
    assert!(err.to_string().contains("nothing to commit"));

    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to undo")
    );
    let oplog = repo.path().join(".git/gitgud/oplog");
    let leftovers: Vec<_> = std::fs::read_dir(&oplog)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "index"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "no index copies left behind: {leftovers:?}"
    );
}

#[test]
fn refuses_to_undo_after_an_outside_commit() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    repo.git(&["commit", "-q", "-m", "from a terminal"]);
    let tip = head(&repo);

    let err = session.undo(id).unwrap_err();

    assert!(err.to_string().contains("changed outside gitgud"), "{err}");
    assert_eq!(head(&repo), tip, "nothing moved");
}

#[test]
fn refuses_to_undo_after_outside_staging_but_not_after_a_mere_git_status() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n").write("b.txt", "also\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();

    // `git status` rewrites the index's stat data but stages nothing.
    repo.write("a.txt", "changed\n");
    repo.git(&["status"]);
    session.undo(id).unwrap();
    session.redo(id).unwrap();

    repo.git(&["add", "b.txt"]);
    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("changed outside gitgud")
    );
}

#[test]
fn respects_another_gits_index_lock() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();

    let err = session.undo(id).unwrap_err();

    assert!(err.to_string().contains("index.lock"), "{err}");
    assert!(lock.exists(), "someone else's lock is left alone");
    std::fs::remove_file(&lock).unwrap();
    session.undo(id).unwrap();
}

#[test]
fn the_log_survives_reopening_the_repo() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session.close(id).unwrap();

    let again = Session::default();
    let id2 = again.open(repo.path()).unwrap().id;
    assert_eq!(again.undo(id2).unwrap().action, "Stage a.txt");
}

#[test]
fn recording_an_entry_is_cheap() {
    let (repo, session, id) = setup();
    for i in 0..500 {
        repo.write(&format!("many/f{i}.txt"), "x");
    }
    repo.commit_all("many files");
    let open = session.get(id).unwrap();
    let oplog = gitgud_core::oplog::OpLog::for_repo(&open.repo);

    let t = Instant::now();
    oplog.record(&open.repo, "noop", || Ok(())).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;

    // Spec §6: ≤ 10 ms for a ref / index action's entry; generous for a debug build.
    assert!(ms < 50.0, "recording took {ms:.1} ms");
}

#[test]
fn unknown_repo_ids_fail() {
    let session = Session::default();
    assert!(matches!(session.undo(7), Err(Error::UnknownRepo(7))));
    assert!(matches!(session.redo(7), Err(Error::UnknownRepo(7))));
}
