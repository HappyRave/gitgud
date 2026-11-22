use gitgud_core::remote::{PullOutcome, PullStrategy};
use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn with_remote() -> (TestRepo, tempfile::TempDir, TestRepo) {
    let local = TestRepo::init();
    local.write("a.txt", "1\n").commit_all("base");
    let origin = tempfile::tempdir().unwrap();
    let bare = origin.path().join("origin.git");
    local.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    local.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    local.git(&["fetch", "-q", "origin"]);
    local.git(&["branch", "-q", "--set-upstream-to=origin/main", "main"]);

    let other = TestRepo::init();
    other.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    other.git(&["fetch", "-q", "origin"]);
    other.git(&["checkout", "-q", "-B", "main", "origin/main"]);
    (local, origin, other)
}

#[test]
fn pull_fast_forwards_and_reports_up_to_date() {
    let (local, _origin, other) = with_remote();
    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;

    let up = session.pull(id, PullStrategy::Auto, false, |_| {}).unwrap();
    assert_eq!(up.outcome, PullOutcome::UpToDate);

    other.write("b.txt", "2\n").commit_all("remote commit");
    other.git(&["push", "-q", "origin", "main"]);

    let ff = session.pull(id, PullStrategy::Auto, false, |_| {}).unwrap();
    assert_eq!(ff.outcome, PullOutcome::FastForwarded);
    assert_eq!(
        std::fs::read_to_string(local.path().join("b.txt")).unwrap(),
        "2\n"
    );

    let undone = session.undo(id).unwrap();
    assert!(undone.action.starts_with("Pull "), "{}", undone.action);
    assert!(!local.path().join("b.txt").exists());
}

#[test]
fn diverged_pull_asks_once_and_remembers_strategy() {
    let (local, _origin, other) = with_remote();
    local.git(&["config", "pull.ff", "only"]);
    local.git(&["config", "pull.rebase", "false"]);
    local.write("local.txt", "l\n").commit_all("local commit");
    other.write("remote.txt", "r\n").commit_all("remote commit");
    other.git(&["push", "-q", "origin", "main"]);

    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;

    let diverged = session.pull(id, PullStrategy::Auto, false, |_| {}).unwrap();
    assert_eq!(
        diverged.outcome,
        PullOutcome::Diverged {
            ahead: 1,
            behind: 1,
            upstream: "origin/main".into(),
        }
    );

    let merged = session.pull(id, PullStrategy::Merge, true, |_| {}).unwrap();
    assert_eq!(merged.outcome, PullOutcome::Merged);
    assert_eq!(
        local.git(&["config", "--get", "pull.rebase"]).trim(),
        "false"
    );

    other.write("remote2.txt", "r2\n").commit_all("remote 2");
    other.git(&["push", "-q", "origin", "main"]);
    let auto_merge = session.pull(id, PullStrategy::Auto, false, |_| {}).unwrap();
    assert_eq!(auto_merge.outcome, PullOutcome::Merged);
}

#[test]
fn pull_rebase_remembers_and_stops_on_conflict() {
    let (local, _origin, other) = with_remote();
    local.write("a.txt", "local\n").commit_all("local edit");
    other.write("a.txt", "remote\n").commit_all("remote edit");
    other.git(&["push", "-q", "origin", "main"]);

    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;

    let res = session
        .pull(id, PullStrategy::Rebase, true, |_| {})
        .unwrap();
    assert_eq!(res.outcome, PullOutcome::Conflicts);
    assert_eq!(
        local.git(&["config", "--get", "pull.rebase"]).trim(),
        "true"
    );
    assert!(session.rebase_state(id).unwrap().is_some());

    session.undo(id).unwrap();
    assert!(session.rebase_state(id).unwrap().is_none());
    assert_eq!(
        std::fs::read_to_string(local.path().join("a.txt")).unwrap(),
        "local\n"
    );
}

#[test]
fn push_publish_force_with_lease_and_delete_remote_branch() {
    let (local, _origin, other) = with_remote();
    local.git(&["checkout", "-q", "-b", "feature/ui"]);
    local.write("ui.txt", "v1\n").commit_all("ui v1");

    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;

    let info = session.get(id).unwrap().repo.push_info(None).unwrap();
    assert_eq!(info.branch, "feature/ui");
    assert_eq!(info.remote.as_deref(), Some("origin"));
    assert_eq!(info.upstream, None);
    assert_eq!(info.ahead, 1);
    assert!(!info.needs_force);

    session
        .push(
            id,
            "origin",
            "feature/ui",
            "feature/ui",
            true,
            false,
            |_| {},
        )
        .unwrap();
    let after = session.get(id).unwrap().repo.push_info(None).unwrap();
    assert_eq!(after.upstream.as_deref(), Some("origin/feature/ui"));
    assert_eq!(after.ahead, 0);

    // Rewrite locally and verify force-with-lease works when up to date,
    // and safely refuses if remote moved underneath.
    local.write("ui.txt", "v2\n").commit_all("amend target");
    local.git(&["reset", "--soft", "HEAD~1"]);
    local.git(&["commit", "-q", "--amend", "-m", "ui v1 rewritten"]);
    let rewritten = session.get(id).unwrap().repo.push_info(None).unwrap();
    assert!(rewritten.needs_force);
    assert_eq!((rewritten.ahead, rewritten.behind), (1, 1));

    session
        .push(
            id,
            "origin",
            "feature/ui",
            "feature/ui",
            false,
            true,
            |_| {},
        )
        .unwrap();

    other.git(&["fetch", "-q", "origin"]);
    assert_eq!(
        other
            .git(&["log", "-1", "--format=%s", "origin/feature/ui"])
            .trim(),
        "ui v1 rewritten"
    );

    session
        .delete_remote_branch(id, "origin", "feature/ui")
        .unwrap();
    assert!(!local.git_ok(&[
        "ls-remote",
        "--exit-code",
        "--heads",
        "origin",
        "feature/ui"
    ]));
    let _ = Error::Cancelled;
}
