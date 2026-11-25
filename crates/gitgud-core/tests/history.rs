use gitgud_core::graph::Stroke;
use gitgud_core::history::{Action, Outcome, Step};
use gitgud_core::rebase::RebaseStep;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn rev(repo: &TestRepo, what: &str) -> String {
    repo.git(&["rev-parse", what]).trim().to_owned()
}

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

fn subjects(repo: &TestRepo, range: &str) -> Vec<String> {
    repo.git(&["log", "--format=%s", range])
        .lines()
        .map(str::to_owned)
        .collect()
}

fn rebasing(repo: &TestRepo) -> bool {
    repo.path().join(".git/rebase-merge").is_dir()
}

fn open(repo: &TestRepo) -> (Session, u32) {
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (session, id)
}

fn step(id: &str, action: Action) -> Step {
    Step {
        id: id.to_owned(),
        action,
        message: None,
    }
}

/// `main`: base. `feature` (HEAD): c1 "Add camera rgiht" (cam.txt), c2 "Engine"
/// (engine.txt), c3 "fixup! Engine" (engine.txt again).
fn stack() -> TestRepo {
    let repo = TestRepo::init();
    repo.write("base.txt", "base\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("cam.txt", "cam\n")
        .commit_all("Add camera rgiht");
    repo.write("engine.txt", "1\n2\n3\n").commit_all("Engine");
    repo.write("engine.txt", "1\n2 smooth\n3\n")
        .commit_all("fixup! Engine");
    repo
}

#[test]
fn the_plan_lists_commits_newest_first_with_fixups_already_applied() {
    let repo = stack();
    repo.git(&["branch", "dep", "HEAD~1"]);
    repo.git(&["branch", "up", "HEAD~2"]);
    repo.git(&["branch", "-q", "--set-upstream-to=up"]);
    let (session, id) = open(&repo);

    let plan = session.history_plan(id, "HEAD~2").unwrap();

    assert_eq!(plan.branch, "feature");
    assert_eq!(plan.base, rev(&repo, "main"));
    assert_eq!(plan.base_subject, "base");
    let shape: Vec<_> = plan
        .commits
        .iter()
        .map(|c| (c.subject.as_str(), c.action))
        .collect();
    assert_eq!(
        shape,
        [
            ("fixup! Engine", Action::Fixup),
            ("Engine", Action::Pick),
            ("Add camera rgiht", Action::Pick),
        ]
    );
    assert_eq!(plan.commits[0].id, rev(&repo, "HEAD"));
    assert_eq!(plan.dependents, ["dep", "up"]);
    assert_eq!(plan.pushed.as_deref(), Some("up"));
}

#[test]
fn the_plan_refuses_what_it_cant_edit_yet() {
    let repo = stack();
    let (session, id) = open(&repo);
    let refused = |from: &str| session.history_plan(id, from).unwrap_err().to_string();

    assert!(refused(&rev(&repo, "main")).contains("first commit"));
    repo.git(&["checkout", "-q", "-b", "other", "main"]);
    repo.write("o.txt", "o\n").commit_all("other");
    assert!(refused(&rev(&repo, "feature")).contains("isn't on other"));

    repo.git(&["merge", "-q", "--no-ff", "-m", "merge", "feature"]);
    assert!(refused("HEAD~1").contains("merge commits"));

    repo.git(&["checkout", "-q", "--detach"]);
    assert!(refused("HEAD~1").contains("check out a branch"));
}

#[test]
fn prediction_replays_each_step_and_names_conflicts_without_touching_anything() {
    let repo = TestRepo::init();
    repo.write("a.txt", "1\n2\n3\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("a.txt", "1\nA\n3\n").commit_all("one");
    repo.write("a.txt", "1\nB\n3\n").commit_all("two");
    repo.write("c.txt", "c\n").commit_all("three");
    let (session, id) = open(&repo);
    let r = session.get(id).unwrap().repo.clone();
    let git = &*session.git().unwrap();
    let (one, two, three) = (
        rev(&repo, "HEAD~2"),
        rev(&repo, "HEAD~1"),
        rev(&repo, "HEAD"),
    );
    let base = rev(&repo, "main");

    let same = r
        .predict_plan(
            git,
            &base,
            &base,
            &[
                step(&three, Action::Pick),
                step(&two, Action::Fixup),
                step(&one, Action::Reword),
            ],
        )
        .unwrap();
    assert!(same.steps.iter().all(|s| s.outcome == Outcome::Clean));
    assert_eq!((same.before, same.after, same.changed), (3, 2, Some(0)));

    // "two" before "one": two's change applies onto base, where line 2 is still "2".
    let swapped = r
        .predict_plan(
            git,
            &base,
            &base,
            &[
                step(&three, Action::Pick),
                step(&one, Action::Pick),
                step(&two, Action::Pick),
            ],
        )
        .unwrap();
    let outcomes: Vec<_> = swapped
        .steps
        .iter()
        .map(|s| (s.step, s.outcome.clone()))
        .collect();
    assert!(
        matches!(&outcomes[2], (Some(1), Outcome::Conflict { paths, .. }) if paths == &["a.txt"])
    );
    assert!(matches!(outcomes[1], (Some(2), Outcome::Conflict { .. })));
    assert_eq!(outcomes[0], (Some(3), Outcome::AfterConflict { step: 1 }));
    assert_eq!(swapped.changed, None);
    assert_eq!(swapped.first_conflict().unwrap().id, two);
    let Outcome::Conflict { tree, .. } = &swapped.steps[2].outcome else {
        unreachable!()
    };
    let text = r.predicted_file(git, tree, "a.txt").unwrap();
    assert!(text.contains("<<<<<<<") && text.contains("B"), "{text}");

    let dropped = r
        .predict_plan(
            git,
            &base,
            &base,
            &[
                step(&three, Action::Drop),
                step(&two, Action::Pick),
                step(&one, Action::Pick),
            ],
        )
        .unwrap();
    assert_eq!(dropped.steps[0].outcome, Outcome::Dropped);
    assert_eq!(dropped.steps[0].step, None);
    assert_eq!((dropped.after, dropped.changed), (2, Some(1)));

    assert_eq!(rev(&repo, "HEAD"), three);
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
    assert!(
        r.predict_plan(git, &base, &base, &[step(&three, Action::Pick)])
            .unwrap_err()
            .to_string()
            .contains("no longer matches")
    );
    assert!(
        r.predict_plan(
            git,
            &base,
            &base,
            &[
                step(&three, Action::Pick),
                step(&two, Action::Pick),
                step(&one, Action::Squash),
            ]
        )
        .unwrap_err()
        .to_string()
        .contains("nothing to squash")
    );
}

#[test]
fn a_plan_rewords_squashes_fixes_up_and_drops_in_one_undoable_rebase() {
    let repo = stack();
    repo.write("junk.txt", "junk\n").commit_all("junk");
    repo.write("squash.txt", "s\n").commit_all("squash me");
    let (session, id) = open(&repo);
    let before = rev(&repo, "HEAD");
    let plan = session.history_plan(id, "HEAD~4").unwrap();
    let ids: Vec<String> = plan.commits.iter().map(|c| c.id.clone()).collect();
    // Newest first: squash me, junk, fixup! Engine, Engine, Add camera rgiht.
    let steps = vec![
        Step {
            id: ids[0].clone(),
            action: Action::Squash,
            message: Some("Engine, smoothed\n\nWith squash.txt".into()),
        },
        step(&ids[1], Action::Drop),
        step(&ids[2], Action::Fixup),
        step(&ids[3], Action::Pick),
        Step {
            id: ids[4].clone(),
            action: Action::Reword,
            message: Some("Add camera right".into()),
        },
    ];

    session
        .run_plan(id, &plan.base, &plan.base, &steps, false)
        .unwrap();

    assert!(!rebasing(&repo));
    assert_eq!(
        subjects(&repo, "main..feature"),
        ["Engine, smoothed", "Add camera right"]
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%B"]).trim_end(),
        "Engine, smoothed\n\nWith squash.txt"
    );
    assert_eq!(read(&repo, "engine.txt").unwrap(), "1\n2 smooth\n3\n");
    assert_eq!(read(&repo, "squash.txt").unwrap(), "s\n");
    assert_eq!(read(&repo, "junk.txt"), None);
    assert!(!repo.path().join(".git/gitgud/messages").exists());

    let undone = session.undo(id).unwrap();
    assert_eq!(
        undone.action,
        "Rebase feature · squash 1, drop 1, fixup 1, reword 1"
    );
    assert_eq!(rev(&repo, "HEAD"), before);
    assert_eq!(read(&repo, "junk.txt").unwrap(), "junk\n");
    session.redo(id).unwrap();
    assert_eq!(subjects(&repo, "main..feature").len(), 2);
}

#[test]
fn reordering_onto_another_branch_and_commit_msg_hooks_behave_as_in_a_terminal() {
    let repo = stack();
    repo.git(&["checkout", "-q", "main"]);
    repo.write("m.txt", "m\n").commit_all("newer main");
    repo.git(&["checkout", "-q", "feature"]);
    let hook = repo.path().join(".git/hooks/commit-msg");
    std::fs::write(&hook, "#!/bin/sh\nprintf '\\nHooked: yes\\n' >> \"$1\"\n").unwrap();
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~2").unwrap();
    let ids: Vec<String> = plan.commits.iter().map(|c| c.id.clone()).collect();
    // Camera first (newest), engine with its fixup below it.
    let steps = vec![
        Step {
            id: ids[2].clone(),
            action: Action::Reword,
            message: Some("Add camera right".into()),
        },
        step(&ids[0], Action::Fixup),
        step(&ids[1], Action::Pick),
    ];
    let prediction = session
        .plan_preview(id, "main", &plan.base, &steps)
        .unwrap()
        .prediction;
    assert_eq!((prediction.after, prediction.changed), (2, Some(1)));

    session
        .run_plan(id, "main", &plan.base, &steps, false)
        .unwrap();

    assert_eq!(
        subjects(&repo, "HEAD"),
        ["Add camera right", "Engine", "newer main", "base"]
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%B"]).trim_end(),
        "Add camera right\n\nHooked: yes"
    );
    assert_eq!(
        session.op_log(id).unwrap()[0].action,
        "Rebase feature onto main · reword 1, fixup 1"
    );
}

#[test]
fn a_predicted_conflict_stops_the_plan_in_the_banner_state_and_resolving_continues() {
    let repo = TestRepo::init();
    repo.write("a.txt", "1\n2\n3\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("a.txt", "1\nA\n3\n").commit_all("one");
    repo.write("a.txt", "1\nB\n3\n").commit_all("two");
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~1").unwrap();
    let steps = vec![
        step(&plan.commits[1].id, Action::Pick),
        Step {
            id: plan.commits[0].id.clone(),
            action: Action::Reword,
            message: Some("two, reworded".into()),
        },
    ];

    session
        .run_plan(id, &plan.base, &plan.base, &steps, false)
        .unwrap();

    let state = session.rebase_state(id).unwrap().unwrap();
    assert_eq!((state.step, state.total, state.editing), (1, 2, false));
    assert_eq!(state.conflicts[0].path, "a.txt");
    session
        .resolve_with(id, "a.txt", "1\nB\n3\n", "keep two")
        .unwrap();
    session.rebase_step(id, RebaseStep::Continue).unwrap();
    let left = session.rebase_state(id).unwrap();
    if let Some(left) = left {
        // "one" onto "two" conflicts too.
        assert_eq!(left.step, 2);
        session
            .resolve_with(id, "a.txt", "1\nA\n3\n", "keep one")
            .unwrap();
        session.rebase_step(id, RebaseStep::Continue).unwrap();
    }
    assert!(!rebasing(&repo));
    assert_eq!(subjects(&repo, "main..HEAD"), ["one", "two, reworded"]);
}

#[test]
fn an_edit_step_stops_and_continue_amends_what_is_staged() {
    let repo = stack();
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~2").unwrap();
    let mut steps: Vec<Step> = plan.commits.iter().map(|c| step(&c.id, c.action)).collect();
    steps[2].action = Action::Edit;

    session
        .run_plan(id, &plan.base, &plan.base, &steps, false)
        .unwrap();

    let state = session.rebase_state(id).unwrap().unwrap();
    assert!(state.editing);
    assert!(state.conflicts.is_empty());
    assert_eq!(state.subject, "Add camera rgiht");
    repo.write("cam.txt", "cam, edited\n");
    session
        .write(id, "Stage cam.txt", |r, git| {
            r.stage(git, &["cam.txt".into()])
        })
        .unwrap();
    session.rebase_step(id, RebaseStep::Continue).unwrap();

    assert!(!rebasing(&repo));
    assert_eq!(
        subjects(&repo, "main..HEAD"),
        ["Engine", "Add camera rgiht"]
    );
    assert_eq!(
        repo.git(&["show", "HEAD~1:cam.txt"]),
        "cam, edited\n",
        "the edit landed in the commit it stopped at"
    );
}

#[test]
fn update_refs_moves_dependent_branches_and_undo_moves_them_back() {
    let repo = stack();
    repo.git(&["branch", "dep", "HEAD~1"]);
    let dep_before = rev(&repo, "dep");
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~2").unwrap();
    let mut steps: Vec<Step> = plan.commits.iter().map(|c| step(&c.id, c.action)).collect();
    steps[2] = Step {
        id: steps[2].id.clone(),
        action: Action::Reword,
        message: Some("Add camera right".into()),
    };

    session
        .run_plan(id, &plan.base, &plan.base, &steps, true)
        .unwrap();

    assert_eq!(subjects(&repo, "main..dep"), ["Engine", "Add camera right"]);
    assert_eq!(
        rev(&repo, "dep"),
        rev(&repo, "HEAD"),
        "dep holds the fixed-up Engine"
    );

    session.undo(id).unwrap();
    assert_eq!(rev(&repo, "dep"), dep_before);
    session.redo(id).unwrap();
    assert_eq!(rev(&repo, "dep"), rev(&repo, "HEAD"));
}

#[test]
fn without_update_refs_dependent_branches_stay() {
    let repo = stack();
    repo.git(&["branch", "dep", "HEAD~1"]);
    repo.git(&["config", "rebase.updateRefs", "true"]);
    let dep = rev(&repo, "dep");
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~2").unwrap();
    let steps: Vec<Step> = plan.commits.iter().map(|c| step(&c.id, c.action)).collect();
    session
        .run_plan(id, &plan.base, &plan.base, &steps, false)
        .unwrap();
    assert_eq!(rev(&repo, "dep"), dep);
}

#[test]
fn rewording_the_tip_keeps_the_index_and_undoes() {
    let repo = stack();
    repo.write("staged.txt", "s\n");
    repo.git(&["add", "staged.txt"]);
    let (session, id) = open(&repo);
    let before = rev(&repo, "HEAD");

    session.reword_head(id, "Smooth the engine").unwrap();

    assert_eq!(subjects(&repo, "-1"), ["Smooth the engine"]);
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]),
        "staged.txt\n"
    );
    assert_eq!(
        repo.git(&["show", "--name-only", "--format=", "HEAD"]),
        "engine.txt\n"
    );
    assert_eq!(
        session.undo(id).unwrap().action,
        "Reword \"Smooth the engine\""
    );
    assert_eq!(rev(&repo, "HEAD"), before);
}

#[test]
fn quick_fixup_and_drop_apply_at_once_and_undo() {
    let repo = stack();
    let (session, id) = open(&repo);
    let tip = rev(&repo, "HEAD");

    session
        .quick_edit(id, &rev(&repo, "HEAD"), Action::Fixup)
        .unwrap();
    assert_eq!(
        subjects(&repo, "main..HEAD"),
        ["Engine", "Add camera rgiht"]
    );
    assert_eq!(read(&repo, "engine.txt").unwrap(), "1\n2 smooth\n3\n");

    session
        .quick_edit(id, &rev(&repo, "HEAD~1"), Action::Drop)
        .unwrap();
    assert_eq!(subjects(&repo, "main..HEAD"), ["Engine"]);
    assert_eq!(read(&repo, "cam.txt"), None);
    assert_eq!(
        session.undo(id).unwrap().action,
        "Drop \"Add camera rgiht\""
    );
    assert_eq!(
        session.undo(id).unwrap().action,
        "Fixup \"fixup! Engine\" into its parent"
    );
    assert_eq!(rev(&repo, "HEAD"), tip);

    assert!(
        session
            .quick_edit(id, &rev(&repo, "HEAD~2"), Action::Fixup)
            .unwrap_err()
            .to_string()
            .contains("first commit")
    );
    assert!(
        session
            .quick_edit(id, &tip, Action::Squash)
            .unwrap_err()
            .to_string()
            .contains("use the plan")
    );
}

#[test]
fn the_preview_draws_dashed_after_rows_over_the_faded_commits() {
    let repo = stack();
    let (session, id) = open(&repo);
    let plan = session.history_plan(id, "HEAD~2").unwrap();
    let mut steps: Vec<Step> = plan.commits.iter().map(|c| step(&c.id, c.action)).collect();
    steps[2] = Step {
        id: steps[2].id.clone(),
        action: Action::Reword,
        message: Some("Add camera right".into()),
    };

    let preview = session
        .plan_preview(id, &plan.base, &plan.base, &steps)
        .unwrap();

    assert_eq!(preview.rows, 4 + 2);
    let rows = session.preview_rows(id, 0, 10).unwrap();
    let shape: Vec<_> = rows
        .iter()
        .map(|r| (r.subject.as_str(), r.after.as_deref(), r.faded))
        .collect();
    assert_eq!(
        shape,
        [
            ("Engine", Some("+ fixup folded in"), false),
            ("Add camera right", Some("reworded"), false),
            ("fixup! Engine", None, true),
            ("Engine", None, true),
            ("Add camera rgiht", None, true),
            ("base", None, false),
        ]
    );
    assert!(
        rows[0]
            .layout
            .segments
            .iter()
            .all(|s| s.stroke == Stroke::Dashed)
    );
    assert!(
        rows[5]
            .layout
            .segments
            .iter()
            .any(|s| s.stroke == Stroke::Dashed),
        "the after chain lands on the base"
    );
    assert_eq!(
        session.get(id).unwrap().graph.len(),
        4,
        "the real graph is untouched"
    );
}
