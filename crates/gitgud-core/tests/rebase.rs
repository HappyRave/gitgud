use gitgud_core::merge::{Conflict, ConflictKind};
use gitgud_core::rebase::{RebaseStep, StepConflict};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

fn rev(repo: &TestRepo, what: &str) -> String {
    repo.git(&["rev-parse", what]).trim().to_owned()
}

fn rebasing(repo: &TestRepo) -> bool {
    repo.path().join(".git/rebase-merge").is_dir()
}

fn subjects(repo: &TestRepo, range: &str) -> Vec<String> {
    repo.git(&["log", "--format=%s", range])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `feature` edits line 2 of a.txt (f1) and adds b.txt (f2); `main` edits line 2
/// too (m1) when `conflict`, or adds m.txt otherwise. HEAD on `feature`.
fn diverged(conflict: bool) -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "1\n2\n3\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("a.txt", "1\nF\n3\n").commit_all("f1");
    repo.write("b.txt", "b\n").commit_all("f2");
    repo.git(&["checkout", "-q", "main"]);
    if conflict {
        repo.write("a.txt", "1\nM\n3\n").commit_all("m1");
    } else {
        repo.write("m.txt", "m\n").commit_all("m1");
    }
    repo.git(&["checkout", "-q", "feature"]);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

#[test]
fn prediction_counts_steps_and_names_the_first_conflict_without_touching_anything() {
    let (repo, session, id) = diverged(false);
    let r = session.get(id).unwrap().repo.clone();
    let plan = r.predict_rebase(session.git().unwrap(), "main").unwrap();
    assert_eq!((plan.commits, plan.behind, plan.conflict), (2, 1, None));

    let (repo2, session2, id2) = diverged(true);
    let tip = rev(&repo2, "HEAD");
    let r2 = session2.get(id2).unwrap().repo.clone();
    let plan = r2.predict_rebase(session2.git().unwrap(), "main").unwrap();
    assert_eq!(
        plan.conflict,
        Some(StepConflict {
            step: 1,
            short_id: rev(&repo2, "HEAD~1")[..7].to_owned(),
            subject: "f1".into(),
            paths: vec!["a.txt".into()],
        })
    );
    assert_eq!(rev(&repo2, "HEAD"), tip);
    assert!(!rebasing(&repo2));
    assert_eq!(repo2.git(&["status", "--porcelain"]), "");
    drop(repo);
}

#[test]
fn a_clean_rebase_replays_the_branch_and_undo_puts_it_back() {
    let (repo, session, id) = diverged(false);
    let before = rev(&repo, "feature");

    session.rebase(id, "main").unwrap();

    assert!(!rebasing(&repo));
    assert_eq!(subjects(&repo, "main..feature"), ["f2", "f1"]);
    assert_eq!(rev(&repo, "feature~2"), rev(&repo, "main"));
    assert!(session.rebase_state(id).unwrap().is_none());

    assert_eq!(session.undo(id).unwrap().action, "Rebase onto main");
    assert_eq!(rev(&repo, "feature"), before);
    assert_eq!(read(&repo, "m.txt"), None);
}

#[test]
fn a_conflicted_rebase_stops_resolves_and_continues_and_every_step_undoes() {
    let (repo, session, id) = diverged(true);
    let before = rev(&repo, "feature");

    session.rebase(id, "main").unwrap();

    let state = session.rebase_state(id).unwrap().unwrap();
    assert_eq!(
        (
            state.branch.as_str(),
            state.onto.as_str(),
            state.step,
            state.total
        ),
        ("feature", "main", 1, 2)
    );
    assert_eq!(state.subject, "f1");
    assert_eq!(
        state.conflicts,
        [Conflict {
            path: "a.txt".into(),
            kind: ConflictKind::Code,
            resolved: false
        }]
    );
    assert!(
        session
            .rebase_step(id, RebaseStep::Continue)
            .unwrap_err()
            .to_string()
            .contains("still conflicted")
    );

    session
        .resolve_with(id, "a.txt", "1\nM\nF\n3\n", "use both")
        .unwrap();
    assert!(session.rebase_state(id).unwrap().unwrap().conflicts[0].resolved);
    session.rebase_step(id, RebaseStep::Continue).unwrap();

    assert!(session.rebase_state(id).unwrap().is_none());
    assert_eq!(subjects(&repo, "main..feature"), ["f2", "f1"]);
    assert_eq!(read(&repo, "a.txt").unwrap(), "1\nM\nF\n3\n");

    assert_eq!(session.undo(id).unwrap().action, "Continue rebase");
    assert!(rebasing(&repo));
    assert_eq!(
        rev(&repo, "feature"),
        before,
        "the branch waits at its old tip"
    );
    assert!(session.rebase_state(id).unwrap().unwrap().conflicts[0].resolved);

    session.undo(id).unwrap();
    assert!(!session.rebase_state(id).unwrap().unwrap().conflicts[0].resolved);

    assert_eq!(session.undo(id).unwrap().action, "Rebase onto main");
    assert!(!rebasing(&repo));
    assert_eq!(rev(&repo, "HEAD"), before);
    assert_eq!(
        repo.git(&["symbolic-ref", "HEAD"]).trim(),
        "refs/heads/feature"
    );
    assert_eq!(read(&repo, "a.txt").unwrap(), "1\nF\n3\n");

    session.redo(id).unwrap();
    assert!(rebasing(&repo));
    assert_eq!(session.rebase_state(id).unwrap().unwrap().step, 1);
}

#[test]
fn skip_drops_the_commit_and_abort_goes_back_undoably() {
    let (repo, session, id) = diverged(true);
    session.rebase(id, "main").unwrap();
    session.rebase_step(id, RebaseStep::Skip).unwrap();
    assert!(!rebasing(&repo));
    assert_eq!(subjects(&repo, "main..feature"), ["f2"]);

    let (repo, session, id) = diverged(true);
    let before = rev(&repo, "feature");
    session.rebase(id, "main").unwrap();
    session.rebase_step(id, RebaseStep::Abort).unwrap();
    assert!(!rebasing(&repo));
    assert_eq!(rev(&repo, "HEAD"), before);

    assert_eq!(session.undo(id).unwrap().action, "Abort rebase");
    assert!(rebasing(&repo));
    assert_eq!(session.rebase_state(id).unwrap().unwrap().onto, "main");
}

#[test]
fn merge_and_rebase_refuse_while_the_other_is_in_progress() {
    let (_repo, session, id) = diverged(true);
    session.rebase(id, "main").unwrap();
    assert!(
        session
            .merge(id, "main")
            .unwrap_err()
            .to_string()
            .contains("rebase")
    );
    session.rebase_step(id, RebaseStep::Abort).unwrap();

    session.merge(id, "main").unwrap();
    assert!(
        session
            .rebase(id, "main")
            .unwrap_err()
            .to_string()
            .contains("in progress")
    );
}
