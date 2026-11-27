use gitgud_core::repo::{self, Change, Entry, EntryKind, Error, Head, Status};
use gitgud_core::testing::TestRepo;

/// Reads the status with gix and checks it against `git status --porcelain=v2`.
fn status(repo: &TestRepo) -> Status {
    let status = repo::open(repo.path()).unwrap().status().unwrap();
    assert_eq!(
        status.entries,
        repo.cli_status(),
        "gix and the git CLI disagree"
    );
    status
}

fn tracked(path: &str, index: Option<Change>, worktree: Option<Change>) -> Entry {
    Entry {
        path: path.into(),
        kind: EntryKind::Tracked,
        index,
        worktree,
    }
}

fn other(path: &str, kind: EntryKind) -> Entry {
    Entry {
        path: path.into(),
        kind,
        index: None,
        worktree: None,
    }
}

fn committed() -> TestRepo {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("dir/b.txt", "b\n")
        .commit_all("init");
    repo
}

fn branch(name: &str) -> Head {
    Head::Branch { name: name.into() }
}

#[test]
fn status_of_some_paths_and_the_dirty_check_agree_with_the_full_status() {
    let repo = committed();
    let r = repo::open(repo.path()).unwrap();
    assert!(!r.is_dirty().unwrap());

    repo.write("a.txt", "changed\n")
        .write("dir/b.txt", "changed\n")
        .write("new [1].txt", "untracked, with a glob character\n");
    assert!(r.is_dirty().unwrap());
    let some = r
        .status_of(&["a.txt".into(), "new [1].txt".into(), "gone.txt".into()])
        .unwrap();
    let full = status(&repo);
    let wanted: Vec<Entry> = full
        .entries
        .into_iter()
        .filter(|e| e.path != "dir/b.txt")
        .collect();
    assert_eq!(some.entries, wanted);

    repo.git(&["add", "a.txt"]);
    repo.git(&["checkout", "--", "dir/b.txt"]);
    std::fs::remove_file(repo.path().join("new [1].txt")).unwrap();
    assert!(r.is_dirty().unwrap(), "a staged change alone is dirty");
}

#[test]
fn clean_repo_has_no_entries() {
    let repo = committed();
    let s = status(&repo);
    assert_eq!(s.head, branch("main"));
    assert!(s.entries.is_empty());
}

#[test]
fn lists_untracked_files_one_by_one() {
    let repo = committed();
    repo.write("new.txt", "n\n")
        .write("newdir/deep/x.txt", "x\n");
    assert_eq!(
        status(&repo).entries,
        [
            other("new.txt", EntryKind::Untracked),
            other("newdir/deep/x.txt", EntryKind::Untracked),
        ]
    );
}

#[test]
fn hides_ignored_files() {
    let repo = committed();
    repo.write(".gitignore", "*.log\n").commit_all("ignore");
    repo.write("debug.log", "x\n");
    assert!(status(&repo).entries.is_empty());
}

#[test]
fn staged_addition() {
    let repo = committed();
    repo.write("new.txt", "n\n").git(&["add", "new.txt"]);
    assert_eq!(
        status(&repo).entries,
        [tracked("new.txt", Some(Change::Added), None)]
    );
}

#[test]
fn modified_in_index_and_again_in_worktree() {
    let repo = committed();
    repo.write("a.txt", "a2\n").git(&["add", "a.txt"]);
    repo.write("a.txt", "a3\n");
    assert_eq!(
        status(&repo).entries,
        [tracked(
            "a.txt",
            Some(Change::Modified),
            Some(Change::Modified)
        )]
    );
}

#[test]
fn unstaged_and_staged_deletions() {
    let repo = committed();
    repo.remove("a.txt");
    repo.git(&["rm", "-q", "dir/b.txt"]);
    assert_eq!(
        status(&repo).entries,
        [
            tracked("a.txt", None, Some(Change::Deleted)),
            tracked("dir/b.txt", Some(Change::Deleted), None),
        ]
    );
}

#[test]
fn intent_to_add_shows_as_worktree_addition() {
    let repo = committed();
    repo.write("later.txt", "l\n")
        .git(&["add", "-N", "later.txt"]);
    assert_eq!(
        status(&repo).entries,
        [tracked("later.txt", None, Some(Change::Added))]
    );
}

#[test]
fn keeps_unicode_and_spaces_in_paths() {
    let repo = committed();
    repo.write("dir with space/ünïcødé.txt", "u\n");
    assert_eq!(
        status(&repo).entries,
        [other("dir with space/ünïcødé.txt", EntryKind::Untracked)]
    );
}

#[test]
fn merge_conflict_is_one_conflicted_entry() {
    let repo = committed();
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "theirs\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", "ours\n").commit_all("ours");
    assert!(
        !repo.git_ok(&["merge", "-q", "other"]),
        "merge should conflict"
    );

    let s = status(&repo);
    assert_eq!(s.head, branch("main"));
    assert_eq!(s.entries, [other("a.txt", EntryKind::Conflicted)]);
}

#[test]
fn detached_head_shows_short_id() {
    let repo = committed();
    let id = repo.git(&["rev-parse", "--short=7", "HEAD"]);
    repo.git(&["checkout", "-q", "--detach"]);
    assert_eq!(
        status(&repo).head,
        Head::Detached {
            id: id.trim().into()
        }
    );
}

#[test]
fn unborn_branch_with_staged_file() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1\n").git(&["add", "first.txt"]);
    let s = status(&repo);
    assert_eq!(
        s.head,
        Head::Unborn {
            name: "main".into()
        }
    );
    assert_eq!(s.entries, [tracked("first.txt", Some(Change::Added), None)]);
}

#[test]
fn shows_branch_names_with_slashes() {
    let repo = committed();
    repo.git(&["checkout", "-q", "-b", "feature/login"]);
    assert_eq!(status(&repo).head, branch("feature/login"));
}

#[test]
fn opens_from_a_subfolder() {
    let repo = committed();
    let s = repo::open(repo.path().join("dir"))
        .unwrap()
        .status()
        .unwrap();
    assert_eq!(
        s.root.canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
}

#[test]
fn rejects_a_plain_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(repo::open(dir.path()), Err(Error::NotARepo(_))));
}

#[test]
fn rejects_a_bare_repo() {
    let dir = tempfile::tempdir().unwrap();
    let ok = std::process::Command::new("git")
        .args(["init", "-q", "--bare"])
        .current_dir(dir.path())
        .status()
        .unwrap()
        .success();
    assert!(ok);
    assert!(matches!(repo::open(dir.path()), Err(Error::Bare(_))));
}

#[test]
fn serializes_for_the_frontend() {
    let repo = committed();
    repo.write("a.txt", "changed\n");
    let json = serde_json::to_value(status(&repo)).unwrap();
    assert_eq!(
        json["head"],
        serde_json::json!({ "kind": "branch", "name": "main" })
    );
    assert_eq!(
        json["entries"][0],
        serde_json::json!({ "path": "a.txt", "kind": "tracked", "index": null, "worktree": "modified" })
    );
    assert!(json["root"].is_string());
}
