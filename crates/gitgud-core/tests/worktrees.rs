use std::fs;
use std::path::PathBuf;

use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

#[test]
fn lists_linked_worktree_proposes_from_it_and_approves_onto_main() {
    let main = TestRepo::init();
    main.write("app.txt", "v1\n").commit_all("initial");

    let wt_parent = tempfile::tempdir().unwrap();
    let wt_dir: PathBuf = wt_parent.path().join("agent-wt");
    main.git(&[
        "worktree",
        "add",
        "-b",
        "agent/feature",
        wt_dir.to_str().unwrap(),
    ]);
    fs::write(wt_dir.join("app.txt"), "v2 from worktree\n").unwrap();

    let session = Session::default();
    let opened = session.open(main.path()).unwrap();

    let worktrees = session.worktrees_list(opened.id).unwrap();
    let linked = worktrees.iter().find(|w| !w.is_main).unwrap();
    assert_eq!(linked.branch.as_deref(), Some("agent/feature"));
    assert_eq!(linked.dirty_count, 1);

    let prop = session
        .propose_from_worktree(
            opened.id,
            &wt_dir,
            "feat: import worktree edits",
            Some("tested in linked worktree"),
            Some("worktree-agent"),
        )
        .unwrap();
    assert_eq!(prop.files, vec!["app.txt"]);

    let listed = session.proposals_list(opened.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, prop.id);

    session
        .proposal_approve(opened.id, &prop.id, None, &[])
        .unwrap();
    assert_eq!(
        fs::read_to_string(main.path().join("app.txt")).unwrap(),
        "v2 from worktree\n"
    );

    main.git(&["worktree", "remove", "--force", wt_dir.to_str().unwrap()]);
}

#[test]
fn tracks_stale_origin_commits_refreshes_index_and_records_timings() {
    let remote = TestRepo::init();
    remote.write("f.txt", "base\n").commit_all("base");
    remote.git(&["branch", "-M", "main"]);
    let bare = tempfile::tempdir().unwrap();
    let bare_path = bare.path().to_str().unwrap();
    remote.git(&[
        "clone",
        "--bare",
        remote.path().to_str().unwrap(),
        bare_path,
    ]);

    let work = TestRepo::init();
    work.git(&["remote", "add", "origin", bare_path]);
    work.git(&["fetch", "origin"]);
    work.git(&["checkout", "-B", "main", "origin/main"]);
    work.git(&["branch", "--set-upstream-to=origin/main", "main"]);
    work.write("f.txt", "pushed\n").commit_all("pushed work");
    work.git(&["push", "origin", "main"]);
    let old_oid = work.git(&["rev-parse", "HEAD"]).trim().to_owned();

    // Rewrite locally (reset back + new commit) so old_oid remains only on origin/main.
    work.git(&["reset", "--hard", "HEAD~1"]);
    work.write("f.txt", "rewritten\n")
        .commit_all("rewritten work");

    let session = Session::default();
    let opened = session.open(work.path()).unwrap();
    session.repo_status(opened.id).unwrap();
    session.graph_rows(opened.id, 0, 10).unwrap();
    session.refresh_index(opened.id).unwrap();

    let stale = session
        .stale_origin_commits(opened.id, "origin/main")
        .unwrap();
    assert_eq!(stale, vec![old_oid]);

    let timings = session.recent_timings();
    assert!(timings.iter().any(|t| t.op == "open"));
    assert!(timings.iter().any(|t| t.op == "status"));
    assert!(timings.iter().any(|t| t.op == "graph_rows"));
    assert!(timings.iter().any(|t| t.op == "refresh_index"));
}
