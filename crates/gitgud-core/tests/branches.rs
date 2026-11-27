use gitgud_core::branches::Branch;
use gitgud_core::repo::{self, Error};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

/// `git for-each-ref` as the oracle: name, current, upstream, ahead, behind.
fn cli_branches(repo: &TestRepo) -> Vec<Branch> {
    repo.git(&[
        "for-each-ref",
        "--format=%(refname:short)%09%(HEAD)%09%(upstream:short)",
        "refs/heads",
    ])
    .lines()
    .map(|line| {
        let mut f = line.split('\t');
        let name = f.next().unwrap().to_owned();
        let current = f.next().unwrap() == "*";
        let upstream = Some(f.next().unwrap().to_owned()).filter(|u| !u.is_empty());
        let (ahead, behind) = match &upstream {
            Some(u) if repo.git_ok(&["rev-parse", "-q", "--verify", u]) => {
                let counts = repo.git(&[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{name}...{u}"),
                ]);
                let mut n = counts.split_whitespace().map(|x| x.parse().unwrap());
                (n.next().unwrap(), n.next().unwrap())
            }
            _ => (0, 0),
        };
        Branch {
            name,
            current,
            upstream,
            ahead,
            behind,
        }
    })
    .collect()
}

fn with_upstream() -> (TestRepo, tempfile::TempDir) {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("base");
    let origin = tempfile::tempdir().unwrap();
    let bare = origin.path().join("o.git");
    repo.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    repo.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    repo.git(&["fetch", "-q", "origin"]);
    repo.git(&["branch", "-q", "--set-upstream-to=origin/main", "main"]);
    (repo, origin)
}

#[test]
fn lists_branches_like_git_does() {
    let (repo, _origin) = with_upstream();
    repo.write("a.txt", "ahead 1\n").commit_all("ahead 1");
    repo.write("a.txt", "ahead 2\n").commit_all("ahead 2");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD~2"]);
    repo.git(&["branch", "feature/login"]);
    repo.git(&["branch", "feature/ui/theme", "HEAD~1"]);
    repo.git(&["branch", "hotfix"]);
    repo.git(&["config", "branch.hotfix.remote", "origin"]);
    repo.git(&["config", "branch.hotfix.merge", "refs/heads/hotfix"]);

    let branches = repo::open(repo.path()).unwrap().branches().unwrap();

    assert_eq!(branches.local, cli_branches(&repo));
    let main = branches.local.iter().find(|b| b.name == "main").unwrap();
    assert_eq!((main.current, main.ahead, main.behind), (true, 2, 0));
    let hotfix = branches.local.iter().find(|b| b.name == "hotfix").unwrap();
    assert_eq!(
        hotfix.upstream.as_deref(),
        Some("origin/hotfix"),
        "configured but never fetched"
    );
}

#[test]
fn counts_behind_too() {
    let (repo, _origin) = with_upstream();
    repo.git(&["checkout", "-q", "-b", "elsewhere"]);
    repo.write("b.txt", "b\n").commit_all("remote side");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.git(&["checkout", "-q", "main"]);
    repo.write("c.txt", "c\n").commit_all("local side");

    let branches = repo::open(repo.path()).unwrap().branches().unwrap();

    assert_eq!(branches.local, cli_branches(&repo));
    let main = branches.local.iter().find(|b| b.name == "main").unwrap();
    assert_eq!((main.ahead, main.behind), (1, 1));
}

#[test]
fn lists_remote_branches_without_origin_head() {
    let (repo, _origin) = with_upstream();
    repo.git(&["update-ref", "refs/remotes/origin/feature/x", "HEAD"]);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    repo.git(&["remote", "add", "fork", "C:/nowhere/fork.git"]);

    let branches = repo::open(repo.path()).unwrap().branches().unwrap();

    let names: Vec<_> = branches
        .remotes
        .iter()
        .map(|r| (r.name.as_str(), r.branches.clone()))
        .collect();
    assert_eq!(
        names,
        [
            ("fork", vec![]),
            (
                "origin",
                vec!["origin/feature/x".to_owned(), "origin/main".to_owned()]
            )
        ]
    );
}

#[test]
fn reports_when_the_repo_last_fetched() {
    let (repo, _origin) = with_upstream();
    let fetched = repo::open(repo.path())
        .unwrap()
        .branches()
        .unwrap()
        .fetched
        .expect("with_upstream fetched");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!((now - fetched).abs() < 300, "fetched {fetched}, now {now}");

    let fresh = TestRepo::init();
    fresh.write("a.txt", "a\n").commit_all("a");
    assert_eq!(
        repo::open(fresh.path())
            .unwrap()
            .branches()
            .unwrap()
            .fetched,
        None
    );
}

#[test]
fn detached_head_marks_no_branch_current() {
    let (repo, _origin) = with_upstream();
    repo.git(&["checkout", "-q", "--detach"]);
    let branches = repo::open(repo.path()).unwrap().branches().unwrap();
    assert!(branches.local.iter().all(|b| !b.current));
}

#[test]
fn checkout_paths_are_the_files_that_differ() {
    let repo = TestRepo::init();
    repo.write("same.txt", "s\n")
        .write("changed.txt", "1\n")
        .write("gone.txt", "g\n")
        .commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("changed.txt", "2\n")
        .write("dir/new.txt", "n\n")
        .remove("gone.txt")
        .commit_all("other");
    repo.git(&["checkout", "-q", "main"]);

    let paths = repo::open(repo.path())
        .unwrap()
        .checkout_paths("other")
        .unwrap();

    let cli: Vec<String> = repo
        .git(&["diff", "--name-only", "HEAD", "other"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(paths, cli);
    assert!(matches!(
        repo::open(repo.path()).unwrap().checkout_paths("nope"),
        Err(Error::Refused(_))
    ));
}

fn two_branches() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "main version\n")
        .write("shared.txt", "s\n")
        .commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "other version\n")
        .write("only-other.txt", "o\n")
        .commit_all("other");
    repo.git(&["checkout", "-q", "main"]);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

#[test]
fn checkout_switches_and_ctrl_z_switches_back() {
    let (repo, session, id) = two_branches();

    let opened = session.checkout(id, "other").unwrap();

    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "other");
    assert_eq!(read(&repo, "a.txt").unwrap(), "other version\n");
    assert_eq!(opened.graph_version, 1, "HEAD moved, graph rebuilt");

    let undone = session.undo(id).unwrap();
    assert_eq!(undone.action, "Check out other");
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
    assert_eq!(read(&repo, "a.txt").unwrap(), "main version\n");
    assert_eq!(read(&repo, "only-other.txt"), None);
    assert_eq!(
        repo.git(&["status", "--porcelain"]).trim(),
        "",
        "clean, like before"
    );

    session.redo(id).unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "other");
    assert_eq!(read(&repo, "only-other.txt").unwrap(), "o\n");
}

#[test]
fn local_changes_travel_and_come_back_on_undo() {
    let (repo, session, id) = two_branches();
    repo.write("shared.txt", "my edit\n");

    session.checkout(id, "other").unwrap();
    assert_eq!(read(&repo, "shared.txt").unwrap(), "my edit\n");

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "shared.txt").unwrap(), "my edit\n");
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
}

#[test]
fn checkout_refuses_to_overwrite_local_changes_and_records_nothing() {
    let (repo, session, id) = two_branches();
    repo.write("a.txt", "uncommitted\n");

    let err = session.checkout(id, "other").unwrap_err();

    assert!(err.to_string().contains("would be overwritten"), "{err}");
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
    assert_eq!(read(&repo, "a.txt").unwrap(), "uncommitted\n");
    assert!(session.op_log(id).unwrap().is_empty());
}

#[test]
fn a_name_that_git_would_read_as_an_option_is_refused() {
    let (repo, session, id) = two_branches();
    let main = repo.git(&["rev-parse", "main"]);
    assert!(matches!(
        session.create_branch(id, "-D", Some("main"), false),
        Err(gitgud_core::repo::Error::Refused(_))
    ));
    assert!(matches!(
        session.create_tag(id, "-d", Some("main"), None),
        Err(gitgud_core::repo::Error::Refused(_))
    ));
    assert_eq!(
        repo.git(&["rev-parse", "main"]),
        main,
        "main is still there"
    );
}

#[test]
fn create_rename_delete_and_undo_branches_including_from_detached_head() {
    let (repo, session, id) = two_branches();

    // Create + checkout at HEAD~0, then undo and redo.
    session
        .create_branch(id, "feature/new", None, true)
        .unwrap();
    assert_eq!(
        repo.git(&["branch", "--show-current"]).trim(),
        "feature/new"
    );
    session.undo(id).unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
    assert!(!repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/feature/new"]));
    session.redo(id).unwrap();
    assert_eq!(
        repo.git(&["branch", "--show-current"]).trim(),
        "feature/new"
    );

    // Rename current branch and undo/redo it.
    session
        .rename_branch(id, "feature/new", "feature/renamed")
        .unwrap();
    assert_eq!(
        repo.git(&["branch", "--show-current"]).trim(),
        "feature/renamed"
    );
    assert!(!repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/feature/new"]));
    session.undo(id).unwrap();
    assert_eq!(
        repo.git(&["branch", "--show-current"]).trim(),
        "feature/new"
    );
    assert!(!repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/feature/renamed"]));

    // Switch back to main, verify safe delete refuses unmerged, force deletes, and undo restores it.
    session.checkout(id, "main").unwrap();
    assert!(session.delete_branch(id, "other", false).is_err());
    session.delete_branch(id, "other", true).unwrap();
    assert!(!repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/other"]));
    session.undo(id).unwrap();
    assert!(repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/other"]));

    // Detached HEAD: create branch here and undo back to detached.
    let detached_id = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    repo.git(&["checkout", "-q", "--detach"]);
    session
        .create_branch(id, "rescued", Some(&detached_id), true)
        .unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "rescued");
    session.undo(id).unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]).trim(), detached_id);
}

#[test]
fn checkout_remote_creates_tracking_branch_and_undoes() {
    let (repo, _origin) = with_upstream();
    repo.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    session
        .checkout_remote(id, "origin/topic", "topic")
        .unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "topic");
    let up = repo
        .git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .trim()
        .to_owned();
    assert_eq!(up, "origin/topic");

    session.undo(id).unwrap();
    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
    assert!(!repo.git_ok(&["rev-parse", "--verify", "-q", "refs/heads/topic"]));
}
