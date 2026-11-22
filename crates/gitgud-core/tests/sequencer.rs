use gitgud_core::sequencer::{SequencerKind, SequencerStep};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn read(repo: &TestRepo, path: &str) -> String {
    std::fs::read_to_string(repo.path().join(path)).unwrap()
}

#[test]
fn cherry_pick_and_revert_clean_and_undoable() {
    let repo = TestRepo::init();
    repo.write("a.txt", "base\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("b.txt", "from other\n").commit_all("add b");
    let target = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    repo.git(&["checkout", "-q", "main"]);

    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let git = session.git().unwrap();
    assert!(
        session
            .get(id)
            .unwrap()
            .repo
            .predict_cherry_pick(git, &target)
            .unwrap()
            .is_empty()
    );

    session.cherry_pick(id, &target).unwrap();
    assert_eq!(read(&repo, "b.txt"), "from other\n");
    assert!(session.sequencer_state(id).unwrap().is_none());

    let picked = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    session.revert_commit(id, &picked).unwrap();
    assert!(!repo.path().join("b.txt").exists());
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "Revert \"add b\""
    );

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "b.txt"), "from other\n");
    session.undo(id).unwrap();
    assert!(!repo.path().join("b.txt").exists());
    session.redo(id).unwrap();
    assert_eq!(read(&repo, "b.txt"), "from other\n");
}

#[test]
fn conflicted_cherry_pick_resolves_continues_and_undoes() {
    let repo = TestRepo::init();
    repo.write("a.txt", "1\n2\n3\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "1\nother\n3\n")
        .commit_all("other edit");
    let target = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", "1\nmain\n3\n").commit_all("main edit");

    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let git = session.git().unwrap();
    assert_eq!(
        session
            .get(id)
            .unwrap()
            .repo
            .predict_cherry_pick(git, &target)
            .unwrap(),
        ["a.txt"]
    );

    session.cherry_pick(id, &target).unwrap();
    let state = session.sequencer_state(id).unwrap().unwrap();
    assert_eq!(state.kind, SequencerKind::CherryPick);
    assert_eq!(state.subject, "other edit");
    assert_eq!(state.conflicts.len(), 1);
    assert!(!state.conflicts[0].resolved);

    session
        .resolve_with(id, "a.txt", "1\nmain\nother\n3\n", "use both")
        .unwrap();
    session.sequencer_step(id, SequencerStep::Continue).unwrap();
    assert!(session.sequencer_state(id).unwrap().is_none());
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "other edit");

    session.undo(id).unwrap();
    assert!(session.sequencer_state(id).unwrap().is_some());
    session.sequencer_step(id, SequencerStep::Abort).unwrap();
    assert!(session.sequencer_state(id).unwrap().is_none());
    assert_eq!(read(&repo, "a.txt"), "1\nmain\n3\n");
}
