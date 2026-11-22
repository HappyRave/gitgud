use gitgud_core::cli::GitCli;
use gitgud_core::git;
use gitgud_core::repo::{self, Change, Entry, EntryKind, Error, Repo};
use gitgud_core::testing::TestRepo;

fn cli() -> GitCli {
    GitCli::new(&git::locate().unwrap())
}

fn open(repo: &TestRepo) -> Repo {
    repo::open(repo.path()).unwrap()
}

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

/// Status through gix, checked against the CLI like every status test.
fn entries(repo: &TestRepo) -> Vec<Entry> {
    let entries = open(repo).status().unwrap().entries;
    assert_eq!(entries, repo.cli_status());
    entries
}

fn staged(path: &str, change: Change) -> Entry {
    Entry {
        path: path.into(),
        kind: EntryKind::Tracked,
        index: Some(change),
        worktree: None,
    }
}

fn unstaged(path: &str, change: Change) -> Entry {
    Entry {
        path: path.into(),
        kind: EntryKind::Tracked,
        index: None,
        worktree: Some(change),
    }
}

fn untracked(path: &str) -> Entry {
    Entry {
        path: path.into(),
        kind: EntryKind::Untracked,
        index: None,
        worktree: None,
    }
}

fn committed() -> TestRepo {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("b.txt", "b\n")
        .commit_all("init");
    repo
}

fn hook(repo: &TestRepo, name: &str, script: &str) {
    let path = repo.path().join(".git/hooks").join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn stages_changes_new_files_and_deletions() {
    let repo = committed();
    repo.write("a.txt", "changed\n")
        .write("new.txt", "n\n")
        .remove("b.txt");

    open(&repo)
        .stage(&cli(), &paths(&["a.txt", "b.txt", "new.txt"]))
        .unwrap();

    assert_eq!(
        entries(&repo),
        [
            staged("a.txt", Change::Modified),
            staged("b.txt", Change::Deleted),
            staged("new.txt", Change::Added),
        ]
    );
}

#[test]
fn stages_only_the_given_paths() {
    let repo = committed();
    repo.write("a.txt", "changed\n").write("b.txt", "changed\n");

    open(&repo).stage(&cli(), &paths(&["a.txt"])).unwrap();

    assert_eq!(
        entries(&repo),
        [
            staged("a.txt", Change::Modified),
            unstaged("b.txt", Change::Modified),
        ]
    );
}

#[test]
fn treats_paths_literally() {
    let repo = committed();
    let names = ["[x].txt", "-dash.txt", "dir with space/ünï.txt"];
    for name in names {
        repo.write(name, "x\n");
    }
    // As a glob, "[x].txt" would also match this one.
    repo.write("x.txt", "x\n");

    open(&repo).stage(&cli(), &paths(&names)).unwrap();

    let e = entries(&repo);
    assert!(
        e.contains(&untracked("x.txt")),
        "[x].txt was read as a glob: {e:?}"
    );
    assert_eq!(
        e.iter().filter(|e| e.index == Some(Change::Added)).count(),
        3
    );
}

#[test]
fn stages_thousands_of_paths_without_hitting_the_command_line_limit() {
    let repo = committed();
    let names: Vec<String> = (0..3000)
        .map(|i| format!("generated/long-file-name-number-{i:05}.txt"))
        .collect();
    for name in &names {
        repo.write(name, "x");
    }

    open(&repo).stage(&cli(), &names).unwrap();

    assert_eq!(entries(&repo).len(), 3000);
}

#[test]
fn empty_path_list_does_nothing() {
    let repo = committed();
    repo.write("a.txt", "changed\n");
    let r = open(&repo);
    r.stage(&cli(), &[]).unwrap();
    r.unstage(&cli(), &[]).unwrap();
    assert_eq!(entries(&repo), [unstaged("a.txt", Change::Modified)]);
}

#[test]
fn unstage_returns_the_index_to_head_and_keeps_the_work_tree() {
    let repo = committed();
    repo.write("a.txt", "changed\n").write("new.txt", "n\n");
    repo.git(&["rm", "-q", "b.txt"]);
    let r = open(&repo);
    r.stage(&cli(), &paths(&["a.txt", "new.txt"])).unwrap();

    r.unstage(&cli(), &paths(&["a.txt", "b.txt", "new.txt"]))
        .unwrap();

    assert_eq!(
        entries(&repo),
        [
            unstaged("a.txt", Change::Modified),
            unstaged("b.txt", Change::Deleted),
            untracked("new.txt"),
        ]
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "changed\n"
    );
}

#[test]
fn unstage_works_before_the_first_commit() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1\n").git(&["add", "first.txt"]);

    open(&repo).unstage(&cli(), &paths(&["first.txt"])).unwrap();

    assert_eq!(entries(&repo), [untracked("first.txt")]);
}

#[test]
fn staging_a_conflicted_file_marks_it_resolved() {
    let repo = committed();
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "theirs\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", "ours\n").commit_all("ours");
    assert!(!repo.git_ok(&["merge", "-q", "other"]));
    repo.write("a.txt", "resolved\n");

    open(&repo).stage(&cli(), &paths(&["a.txt"])).unwrap();

    assert_eq!(entries(&repo), [staged("a.txt", Change::Modified)]);
}

#[test]
fn commits_the_index_with_the_exact_message() {
    let repo = committed();
    repo.write("a.txt", "changed\n").write("b.txt", "also\n");
    let r = open(&repo);
    r.stage(&cli(), &paths(&["a.txt"])).unwrap();

    let message = "Subject line\n\nBody with \"quotes\", $dollars and `ticks`.\nSecond line.";
    r.commit(&cli(), message).unwrap();

    assert_eq!(repo.git(&["log", "-1", "--format=%B"]).trim_end(), message);
    assert_eq!(
        repo.git(&["show", "--name-only", "--format=", "HEAD"])
            .trim(),
        "a.txt"
    );
    assert_eq!(entries(&repo), [unstaged("b.txt", Change::Modified)]);
}

#[test]
fn first_commit_on_an_unborn_branch() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1\n");
    let r = open(&repo);
    r.stage(&cli(), &paths(&["first.txt"])).unwrap();
    r.commit(&cli(), "first").unwrap();

    assert_eq!(repo.git(&["log", "--format=%s"]).trim(), "first");
    assert!(entries(&repo).is_empty());
}

#[test]
fn commit_with_nothing_staged_fails_with_gits_message() {
    let repo = committed();
    let err = open(&repo).commit(&cli(), "nothing").unwrap_err();
    match err {
        Error::Command { command, message } => {
            assert_eq!(command, "git commit -q --file=-");
            assert!(message.contains("nothing to commit"), "{message}");
        }
        other => panic!("expected a command error, got {other:?}"),
    }
}

#[test]
fn commit_with_an_empty_message_fails() {
    let repo = committed();
    repo.write("a.txt", "changed\n");
    let r = open(&repo);
    r.stage(&cli(), &paths(&["a.txt"])).unwrap();

    let err = r.commit(&cli(), "  \n").unwrap_err();
    assert!(err.to_string().contains("empty commit message"), "{err}");
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
}

#[test]
fn a_rejecting_pre_commit_hook_fails_the_commit_with_its_output() {
    let repo = committed();
    hook(
        &repo,
        "pre-commit",
        "echo 'lint: trailing whitespace in a.txt' >&2\nexit 1",
    );
    repo.write("a.txt", "changed \n");
    let r = open(&repo);
    r.stage(&cli(), &paths(&["a.txt"])).unwrap();

    let err = r.commit(&cli(), "should not land").unwrap_err();

    assert_eq!(err.to_string(), "lint: trailing whitespace in a.txt");
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
    assert_eq!(entries(&repo), [staged("a.txt", Change::Modified)]);
}

#[test]
fn commit_msg_hooks_run_and_can_rewrite_the_message() {
    let repo = committed();
    hook(
        &repo,
        "commit-msg",
        r#"printf '\n\nTicket: GG-1\n' >> "$1""#,
    );
    repo.write("a.txt", "changed\n");
    let r = open(&repo);
    r.stage(&cli(), &paths(&["a.txt"])).unwrap();

    r.commit(&cli(), "with ticket").unwrap();

    assert_eq!(
        repo.git(&["log", "-1", "--format=%B"]).trim_end(),
        "with ticket\n\nTicket: GG-1"
    );
}

#[test]
fn stage_reports_a_missing_path() {
    let repo = committed();
    let err = open(&repo)
        .stage(&cli(), &paths(&["nope.txt"]))
        .unwrap_err();
    assert!(err.to_string().contains("nope.txt"), "{err}");
}
