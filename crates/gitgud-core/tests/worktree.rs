use gitgud_core::cli::GitCli;
use gitgud_core::diff::{FileDiff, LineKind};
use gitgud_core::git;
use gitgud_core::repo::{self, Error, Repo};
use gitgud_core::testing::TestRepo;
use gitgud_core::worktree::{HunkHeader, LineSelect, Side};
use proptest::prelude::*;

fn cli() -> GitCli {
    GitCli::new(&git::locate().unwrap())
}

fn open(repo: &TestRepo) -> Repo {
    repo::open(repo.path()).unwrap()
}

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// `-`/`+` lines of a diff, without line endings: comparable with `git diff`.
fn changed(diff: &FileDiff) -> Vec<String> {
    diff.hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter_map(|l| match l.kind {
            LineKind::Added => Some(format!("+{}", l.text)),
            LineKind::Removed => Some(format!("-{}", l.text)),
            LineKind::Context => None,
        })
        .collect()
}

fn cli_changed(repo: &TestRepo, args: &[&str]) -> Vec<String> {
    let mut all = vec![
        "-c",
        "core.quotepath=false",
        "diff",
        "--no-color",
        "--diff-algorithm=histogram",
    ];
    all.extend_from_slice(args);
    repo.git(&all)
        .lines()
        .skip_while(|l| !l.starts_with("@@"))
        .filter(|l| {
            (l.starts_with('+') || l.starts_with('-'))
                && !l.starts_with("+++")
                && !l.starts_with("---")
        })
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect()
}

fn headers(diff: &FileDiff) -> Vec<HunkHeader> {
    diff.hunks.iter().map(HunkHeader::from).collect()
}

/// The file as staged in the index, byte for byte.
fn index_bytes(repo: &TestRepo, path: &str) -> Vec<u8> {
    std::process::Command::new("git")
        .args(["show", &format!(":{path}")])
        .current_dir(repo.path())
        .output()
        .unwrap()
        .stdout
}

fn committed(file: &str, content: &str) -> TestRepo {
    let repo = TestRepo::init();
    repo.write(file, content).commit_all("init");
    repo
}

#[test]
fn unstaged_and_staged_diffs_match_git() {
    let repo = committed("f.txt", &numbered(30));
    repo.write("f.txt", &numbered(30).replace("line 3\n", "three\n"));
    repo.git(&["add", "f.txt"]);
    repo.write(
        "f.txt",
        &numbered(30)
            .replace("line 3\n", "three\n")
            .replace("line 25\n", "twenty-five\n"),
    );
    let r = open(&repo);

    let unstaged = r.worktree_file_diff("f.txt", Side::Unstaged).unwrap();
    let staged = r.worktree_file_diff("f.txt", Side::Staged).unwrap();

    assert_eq!(changed(&unstaged), cli_changed(&repo, &["--", "f.txt"]));
    assert_eq!(
        changed(&staged),
        cli_changed(&repo, &["--cached", "--", "f.txt"])
    );
    assert_eq!(changed(&staged), ["-line 3", "+three"]);
    assert_eq!(changed(&unstaged), ["-line 25", "+twenty-five"]);
}

#[test]
fn untracked_and_deleted_files_diff_against_nothing() {
    let repo = committed("gone.txt", "a\nb\n");
    repo.remove("gone.txt").write("new.txt", "x\ny\n");
    let r = open(&repo);

    assert_eq!(
        changed(&r.worktree_file_diff("new.txt", Side::Unstaged).unwrap()),
        ["+x", "+y"]
    );
    assert_eq!(
        changed(&r.worktree_file_diff("gone.txt", Side::Unstaged).unwrap()),
        ["-a", "-b"]
    );
}

#[test]
fn autocrlf_work_tree_files_diff_like_git_does() {
    let repo = TestRepo::init();
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.write("w.txt", "one\r\ntwo\r\nthree\r\n")
        .commit_all("init");
    repo.write("w.txt", "one\r\nTWO\r\nthree\r\n");

    let diff = open(&repo)
        .worktree_file_diff("w.txt", Side::Unstaged)
        .unwrap();

    assert_eq!(
        changed(&diff),
        ["-two", "+TWO"],
        "only the edited line, not every CRLF"
    );
    assert_eq!(changed(&diff), cli_changed(&repo, &["--", "w.txt"]));
}

#[test]
fn stages_one_hunk_and_leaves_the_other() {
    let repo = committed("f.txt", &numbered(40));
    let edited = numbered(40)
        .replace("line 5\n", "five\n")
        .replace("line 35\n", "thirty-five\n");
    repo.write("f.txt", &edited);
    let r = open(&repo);
    let diff = r.worktree_file_diff("f.txt", Side::Unstaged).unwrap();
    assert_eq!(diff.hunks.len(), 2);

    r.stage_hunk(&cli(), "f.txt", headers(&diff)[0]).unwrap();

    assert_eq!(cli_changed(&repo, &["--cached"]), ["-line 5", "+five"]);
    assert_eq!(cli_changed(&repo, &[]), ["-line 35", "+thirty-five"]);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        edited,
        "work tree untouched"
    );
}

#[test]
fn unstages_one_hunk() {
    let repo = committed("f.txt", &numbered(40));
    repo.write(
        "f.txt",
        &numbered(40)
            .replace("line 5\n", "five\n")
            .replace("line 35\n", "thirty-five\n"),
    );
    repo.git(&["add", "f.txt"]);
    let r = open(&repo);
    let staged = r.worktree_file_diff("f.txt", Side::Staged).unwrap();

    r.unstage_hunk(&cli(), "f.txt", headers(&staged)[1])
        .unwrap();

    assert_eq!(cli_changed(&repo, &["--cached"]), ["-line 5", "+five"]);
    assert_eq!(cli_changed(&repo, &[]), ["-line 35", "+thirty-five"]);
}

#[test]
fn keeps_crlf_bytes_when_the_index_holds_them() {
    let repo = committed("w.txt", "a\r\nb\r\nc\r\n");
    repo.write("w.txt", "a\r\nB\r\nc\r\n");
    let r = open(&repo);
    let diff = r.worktree_file_diff("w.txt", Side::Unstaged).unwrap();

    r.stage_hunk(&cli(), "w.txt", headers(&diff)[0]).unwrap();

    assert_eq!(index_bytes(&repo, "w.txt"), b"a\r\nB\r\nc\r\n");
}

#[test]
fn hunks_and_lines_of_a_non_utf8_file_stage_byte_for_byte() {
    let repo = committed("l.txt", "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n");
    let edited = b"a\nb\xe9\nc\nd\ne\nf\ng\nh\ni\nj \xe0\xe8\n";
    std::fs::write(repo.path().join("l.txt"), edited).unwrap();
    let r = open(&repo);
    let diff = r.worktree_file_diff("l.txt", Side::Unstaged).unwrap();
    let hunks = headers(&diff);
    assert_eq!(hunks.len(), 2);

    r.stage_hunk(&cli(), "l.txt", hunks[1]).unwrap();
    assert_eq!(
        index_bytes(&repo, "l.txt"),
        b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj \xe0\xe8\n"
    );

    let first = &r.worktree_file_diff("l.txt", Side::Unstaged).unwrap().hunks[0];
    let added = first
        .lines
        .iter()
        .position(|l| l.kind == LineKind::Added)
        .unwrap();
    let removed = first
        .lines
        .iter()
        .position(|l| l.kind == LineKind::Removed)
        .unwrap();
    r.stage_lines(
        &cli(),
        "l.txt",
        &LineSelect {
            hunk: HunkHeader::from(first),
            lines: vec![removed, added],
        },
    )
    .unwrap();
    assert_eq!(index_bytes(&repo, "l.txt"), edited);
}

#[test]
fn handles_a_last_line_without_newline() {
    let repo = committed("f.txt", "a\nb\nlast");
    repo.write("f.txt", "a\nb\nlast changed");
    let r = open(&repo);
    let diff = r.worktree_file_diff("f.txt", Side::Unstaged).unwrap();

    r.stage_hunk(&cli(), "f.txt", headers(&diff)[0]).unwrap();

    assert_eq!(index_bytes(&repo, "f.txt"), b"a\nb\nlast changed");
}

#[test]
fn paths_with_spaces_and_unicode() {
    let repo = committed("dir with space/ünï.txt", &numbered(5));
    repo.write(
        "dir with space/ünï.txt",
        &numbered(5).replace("line 2\n", "two\n"),
    );
    let r = open(&repo);
    let diff = r
        .worktree_file_diff("dir with space/ünï.txt", Side::Unstaged)
        .unwrap();

    r.stage_hunk(&cli(), "dir with space/ünï.txt", headers(&diff)[0])
        .unwrap();

    assert_eq!(cli_changed(&repo, &["--cached"]), ["-line 2", "+two"]);
}

#[test]
fn a_stale_hunk_is_refused_and_nothing_is_staged() {
    let repo = committed("f.txt", &numbered(10));
    repo.write("f.txt", &numbered(10).replace("line 5\n", "five\n"));
    let r = open(&repo);
    let seen = headers(&r.worktree_file_diff("f.txt", Side::Unstaged).unwrap())[0];
    repo.write("f.txt", &numbered(10).replace("line 2\n", "two\n"));

    let err = r.stage_hunk(&cli(), "f.txt", seen).unwrap_err();

    assert!(
        matches!(err, Error::Stale(ref p) if p == "f.txt"),
        "{err:?}"
    );
    assert!(cli_changed(&repo, &["--cached"]).is_empty());
}

#[test]
fn new_deleted_binary_and_conflicted_files_stage_whole() {
    let repo = committed("gone.txt", "a\n");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 1]).unwrap();
    repo.commit_all("bin");
    repo.remove("gone.txt").write("new.txt", "n\n");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 2]).unwrap();
    let r = open(&repo);
    let any = HunkHeader {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
    };

    for path in ["gone.txt", "new.txt", "bin.dat"] {
        let err = r.stage_hunk(&cli(), path, any).unwrap_err();
        assert!(err.to_string().contains("whole file"), "{path}: {err}");
    }
}

#[test]
fn conflicted_files_have_no_working_copy_diff() {
    let repo = committed("a.txt", "base\n");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "theirs\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", "ours\n").commit_all("ours");
    assert!(!repo.git_ok(&["merge", "-q", "other"]));

    let err = open(&repo)
        .worktree_file_diff("a.txt", Side::Unstaged)
        .unwrap_err();
    assert!(err.to_string().contains("conflicted"), "{err}");
}

/// Edits of a file with distinct lines, so hunks are well separated or merged deterministically.
fn edit() -> impl Strategy<Value = (String, String)> {
    (
        5usize..80,
        prop::collection::vec((0u8..3, any::<u16>()), 1..10),
    )
        .prop_map(|(n, edits)| {
            let old: Vec<String> = (0..n).map(|i| format!("line {i}")).collect();
            let mut new = old.clone();
            for (k, (op, at)) in edits.into_iter().enumerate() {
                let at = usize::from(at) % (new.len() + 1);
                match op {
                    0 if at < new.len() && new.len() > 1 => drop(new.remove(at)),
                    1 if at < new.len() => new[at] = format!("changed {k}"),
                    _ => new.insert(at, format!("inserted {k}")),
                }
            }
            let join = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
            (join(&old), join(&new))
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    #[test]
    fn staging_every_hunk_then_unstaging_every_hunk_round_trips((old, new) in edit()) {
        let repo = committed("f.txt", &old);
        repo.write("f.txt", &new);
        let r = open(&repo);
        let git = cli();

        while let Some(h) = r.worktree_file_diff("f.txt", Side::Unstaged).unwrap().hunks.first() {
            r.stage_hunk(&git, "f.txt", h.into()).unwrap();
        }
        prop_assert_eq!(index_bytes(&repo, "f.txt"), new.as_bytes());

        while let Some(h) = r.worktree_file_diff("f.txt", Side::Staged).unwrap().hunks.last() {
            r.unstage_hunk(&git, "f.txt", h.into()).unwrap();
        }
        prop_assert_eq!(index_bytes(&repo, "f.txt"), old.as_bytes());
    }
}

#[test]
fn stage_unstage_and_discard_individual_lines_round_trip() {
    let repo = committed("f.txt", "a\nb\nc\nd\n");
    repo.write("f.txt", "a\nB1\nB2\nc\nd\nE1\n");
    let r = open(&repo);
    let git = cli();
    let diff = r.worktree_file_diff("f.txt", Side::Unstaged).unwrap();
    let hunk = &diff.hunks[0];
    // Hunk lines: 0:ctx "a", 1:del "b", 2:add "B1", 3:add "B2", 4:ctx "c", 5:ctx "d", 6:add "E1"
    // Stage only -b and +B1 (indices 1 and 2).
    r.stage_lines(
        &git,
        "f.txt",
        &LineSelect {
            hunk: hunk.into(),
            lines: vec![1, 2],
        },
    )
    .unwrap();
    assert_eq!(index_bytes(&repo, "f.txt"), b"a\nB1\nc\nd\n");

    // Unstage only -b (index 1 of staged hunk), leaving +B1 staged as an insertion after a & b!
    let staged_diff = r.worktree_file_diff("f.txt", Side::Staged).unwrap();
    let staged_hunk = &staged_diff.hunks[0];
    r.unstage_lines(
        &git,
        "f.txt",
        &LineSelect {
            hunk: staged_hunk.into(),
            lines: vec![1],
        },
    )
    .unwrap();
    assert_eq!(index_bytes(&repo, "f.txt"), b"a\nb\nB1\nc\nd\n");

    // Discard only +E1 from unstaged working copy via Session so it is undoable.
    let session = gitgud_core::session::Session::default();
    let opened = session.open(repo.path()).unwrap();
    let unstaged2 = session
        .get(opened.id)
        .unwrap()
        .repo
        .worktree_file_diff("f.txt", Side::Unstaged)
        .unwrap();
    let h2 = &unstaged2.hunks[0];
    let e1_idx = h2
        .lines
        .iter()
        .position(|l| l.text == "E1")
        .expect("E1 in unstaged diff");
    session
        .discard_lines(
            opened.id,
            "f.txt",
            &LineSelect {
                hunk: h2.into(),
                lines: vec![e1_idx],
            },
        )
        .unwrap();
    let after_discard = std::fs::read_to_string(repo.path().join("f.txt")).unwrap();
    assert!(!after_discard.contains("E1"), "{after_discard:?}");

    session.undo(opened.id).unwrap();
    let after_undo = std::fs::read_to_string(repo.path().join("f.txt")).unwrap();
    assert!(after_undo.contains("E1"), "{after_undo:?}");
}
