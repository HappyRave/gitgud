use std::process::Command;

use gitgud_core::diff::{FileChange, Hunk, LineKind, text_hunks};
use gitgud_core::repo::{self, Change};
use gitgud_core::testing::TestRepo;
use proptest::prelude::*;

/// A diff as text, the way `git diff` prints the part after the file header.
fn render(hunks: &[Hunk]) -> String {
    let mut out = String::new();
    for h in hunks {
        let range = |start: u32, len: u32| {
            if len == 1 {
                format!("{start}")
            } else {
                format!("{start},{len}")
            }
        };
        out += &format!(
            "@@ -{} +{} @@\n",
            range(h.old_start, h.old_lines),
            range(h.new_start, h.new_lines)
        );
        for l in &h.lines {
            let sign = match l.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            };
            out += &format!("{sign}{}\n", l.text);
        }
    }
    out
}

/// `git diff` output from the first `@@`, without "\ No newline" markers and the
/// text after each header's closing `@@` (git's function-context hint).
fn cli_hunks(stdout: &str) -> String {
    stdout
        .lines()
        .skip_while(|l| !l.starts_with("@@"))
        .filter(|l| !l.starts_with('\\'))
        .map(|l| match l.strip_prefix("@@") {
            Some(rest) => {
                let end = rest.find("@@").expect("closing @@") + 4;
                format!("{}\n", &l[..end])
            }
            None => format!("{}\n", l.trim_end_matches('\r')),
        })
        .collect()
}

/// git's defaults (indent heuristic on) except the algorithm, which gix fixes to histogram.
fn diff_args() -> [&'static str; 7] {
    [
        "-c",
        "core.quotepath=false",
        "diff",
        "--no-color",
        "--no-renames",
        "--diff-algorithm=histogram",
        "-U3",
    ]
}

/// The first parent, or git's empty tree for a root commit.
fn parent_of(repo: &TestRepo, rev: &str) -> String {
    let parent = format!("{rev}^");
    if repo.git_ok(&["rev-parse", "-q", "--verify", &parent]) {
        parent
    } else {
        repo.git(&["hash-object", "-t", "tree", "--stdin"])
            .trim()
            .to_owned()
    }
}

fn cli_commit_hunks(repo: &TestRepo, rev: &str, path: &str) -> String {
    let parent = parent_of(repo, rev);
    let mut args = diff_args().to_vec();
    args.extend([parent.as_str(), rev, "--", path]);
    cli_hunks(&repo.git(&args))
}

fn cli_files(repo: &TestRepo, rev: &str) -> Vec<FileChange> {
    let parent = parent_of(repo, rev);
    let out = repo.git(&[
        "-c",
        "core.quotepath=false",
        "diff",
        "--no-renames",
        "--name-status",
        &parent,
        rev,
    ]);
    out.lines()
        .map(|l| {
            let (status, path) = l.split_once('\t').unwrap();
            let change = match status {
                "A" => Change::Added,
                "D" => Change::Deleted,
                "M" => Change::Modified,
                "T" => Change::TypeChange,
                other => panic!("unexpected status {other}"),
            };
            FileChange {
                path: path.into(),
                change,
            }
        })
        .collect()
}

fn head(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "HEAD"]).trim().to_owned()
}

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// Checks files and every file's hunks for commit `rev` against git.
fn check_commit(repo: &TestRepo, rev: &str) {
    let r = repo::open(repo.path()).unwrap();
    let id = repo.git(&["rev-parse", rev]).trim().to_owned();
    let detail = r.commit_detail(&id).unwrap();
    assert_eq!(detail.files, cli_files(repo, rev), "files of {rev}");
    for file in &detail.files {
        let diff = r.commit_file_diff(&id, &file.path).unwrap();
        assert!(!diff.binary, "{} is text", file.path);
        assert_eq!(
            render(&diff.hunks),
            cli_commit_hunks(repo, rev, &file.path),
            "hunks of {}",
            file.path
        );
    }
}

#[test]
fn commit_detail_carries_message_people_and_parents() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("first");
    repo.write("a.txt", "b\n")
        .commit_all("Subject line\n\nBody paragraph.\n\nSecond paragraph.");

    let r = repo::open(repo.path()).unwrap();
    let detail = r.commit_detail(&head(&repo)).unwrap();

    assert_eq!(detail.subject, "Subject line");
    assert_eq!(detail.body, "Body paragraph.\n\nSecond paragraph.");
    assert_eq!(detail.author.name, "gitgud tests");
    assert_eq!(detail.author.email, "tests@gitgud.invalid");
    assert_eq!(
        detail.author.time.to_string(),
        repo.git(&["log", "-1", "--format=%at"]).trim()
    );
    assert_eq!(detail.parents, [repo.git(&["rev-parse", "HEAD~1"]).trim()]);
    assert_eq!(detail.short_id, &head(&repo)[..7]);
}

#[test]
fn root_commit_lists_every_file_as_added() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("dir/b.txt", "b\n")
        .commit_all("root");
    check_commit(&repo, "HEAD");
    let detail = repo::open(repo.path())
        .unwrap()
        .commit_detail(&head(&repo))
        .unwrap();
    assert!(detail.parents.is_empty());
    assert!(detail.files.iter().all(|f| f.change == Change::Added));
}

#[test]
fn additions_deletions_and_edits_in_subfolders() {
    let repo = TestRepo::init();
    repo.write("keep.txt", &numbered(20))
        .write("gone.txt", "bye\n")
        .write("deep/dir/x.txt", "x\n")
        .commit_all("before");
    repo.write("keep.txt", &numbered(20).replace("line 10\n", "line ten\n"))
        .remove("gone.txt")
        .write("deep/dir/x.txt", "x2\n")
        .write("new dir/ünï.txt", "new\n")
        .commit_all("after");
    check_commit(&repo, "HEAD");
}

#[test]
fn far_apart_changes_are_two_hunks_and_near_ones_merge() {
    let repo = TestRepo::init();
    repo.write("far.txt", &numbered(40))
        .write("near.txt", &numbered(40))
        .commit_all("before");
    let far = numbered(40)
        .replace("line 5\n", "five\n")
        .replace("line 30\n", "thirty\n");
    let near = numbered(40)
        .replace("line 5\n", "five\n")
        .replace("line 11\n", "eleven\n");
    repo.write("far.txt", &far)
        .write("near.txt", &near)
        .commit_all("after");

    check_commit(&repo, "HEAD");
    let r = repo::open(repo.path()).unwrap();
    assert_eq!(
        r.commit_file_diff(&head(&repo), "far.txt")
            .unwrap()
            .hunks
            .len(),
        2
    );
    assert_eq!(
        r.commit_file_diff(&head(&repo), "near.txt")
            .unwrap()
            .hunks
            .len(),
        1
    );
}

#[test]
fn edits_at_the_start_and_end_of_a_file() {
    let repo = TestRepo::init();
    repo.write("f.txt", &numbered(10)).commit_all("before");
    repo.write("f.txt", &format!("new first\n{}last\n", &numbered(10)[7..]))
        .commit_all("after");
    check_commit(&repo, "HEAD");
}

#[test]
fn empty_files_and_whole_file_rewrites() {
    let repo = TestRepo::init();
    repo.write("empty.txt", "")
        .write("full.txt", &numbered(5))
        .commit_all("before");
    repo.write("empty.txt", "now\nsome\n")
        .write("full.txt", "")
        .commit_all("after");
    check_commit(&repo, "HEAD");
}

#[test]
fn crlf_lines_lose_their_endings_in_the_text() {
    let repo = TestRepo::init();
    repo.write("w.txt", "a\r\nb\r\nc\r\n").commit_all("before");
    repo.write("w.txt", "a\r\nB\r\nc\r\n").commit_all("after");
    check_commit(&repo, "HEAD");
    let diff = repo::open(repo.path())
        .unwrap()
        .commit_file_diff(&head(&repo), "w.txt")
        .unwrap();
    assert!(diff.hunks[0].lines.iter().all(|l| !l.text.ends_with('\r')));
}

#[test]
fn merge_commits_diff_against_the_first_parent() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("f.txt", "feature\n").commit_all("feature");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("m.txt", "main\n").commit_all("main");
    repo.git(&["merge", "-q", "--no-ff", "-m", "merge", "feature"]);

    check_commit(&repo, "HEAD");
    let detail = repo::open(repo.path())
        .unwrap()
        .commit_detail(&head(&repo))
        .unwrap();
    assert_eq!(detail.parents.len(), 2);
    assert_eq!(
        detail.files,
        [FileChange {
            path: "f.txt".into(),
            change: Change::Added
        }]
    );
}

#[test]
fn line_numbers_follow_each_side() {
    let hunks = text_hunks("a\nb\nc\n", "a\nB\nc\nd\n");
    let numbers: Vec<_> = hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.old, l.new))
        .collect();
    assert_eq!(
        numbers,
        [
            (LineKind::Context, Some(1), Some(1)),
            (LineKind::Removed, Some(2), None),
            (LineKind::Added, None, Some(2)),
            (LineKind::Context, Some(3), Some(3)),
            (LineKind::Added, None, Some(4)),
        ]
    );
}

#[test]
fn binary_files_have_no_hunks() {
    let repo = TestRepo::init();
    std::fs::write(repo.path().join("img.bin"), [0u8, 1, 2, 3]).unwrap();
    repo.commit_all("before");
    std::fs::write(repo.path().join("img.bin"), [0u8, 9, 9]).unwrap();
    repo.commit_all("after");
    let diff = repo::open(repo.path())
        .unwrap()
        .commit_file_diff(&head(&repo), "img.bin")
        .unwrap();
    assert!(diff.binary);
    assert!(diff.hunks.is_empty());
    let meta = diff.binary_meta.unwrap();
    assert_eq!(
        meta.before.as_ref().map(|s| (s.size, s.lfs, s.hydrated)),
        Some((4, false, true))
    );
    assert_eq!(
        meta.after.as_ref().map(|s| (s.size, s.lfs, s.hydrated)),
        Some((3, false, true))
    );
}

#[test]
fn lfs_pointers_diff_as_binary_with_hydration_meta() {
    let repo = TestRepo::init();
    let oid1 = "1".repeat(64);
    let oid2 = "2".repeat(64);
    let ptr1 =
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid1}\nsize 1024\n");
    let ptr2 =
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid2}\nsize 2048\n");

    repo.write("Art/Hero.png", &ptr1).commit_all("add lfs");
    repo.write("Art/Hero.png", &ptr2).commit_all("update lfs");

    let r = repo::open(repo.path()).unwrap();
    let diff = r.commit_file_diff(&head(&repo), "Art/Hero.png").unwrap();
    assert!(diff.binary);
    assert!(diff.hunks.is_empty());
    let meta = diff.binary_meta.unwrap();
    let before = meta.before.unwrap();
    let after = meta.after.unwrap();
    assert_eq!(before.size, 1024);
    assert!(before.lfs);
    assert_eq!(before.oid.as_deref(), Some(oid1.as_str()));
    assert!(!before.hydrated);
    assert_eq!(after.size, 2048);
    assert!(after.lfs);
    assert!(!after.hydrated);

    let obj = repo.path().join(".git/lfs/objects/22/22").join(&oid2);
    std::fs::create_dir_all(obj.parent().unwrap()).unwrap();
    std::fs::write(obj, b"png").unwrap();

    let hydrated_diff = r.commit_file_diff(&head(&repo), "Art/Hero.png").unwrap();
    let hydrated_meta = hydrated_diff.binary_meta.unwrap();
    assert!(!hydrated_meta.before.unwrap().hydrated);
    assert!(hydrated_meta.after.unwrap().hydrated);
}

#[test]
fn bad_ids_and_untouched_paths_are_errors() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("first");
    let r = repo::open(repo.path()).unwrap();
    assert!(r.commit_detail("not-hex").is_err());
    assert!(r.commit_detail(&"0".repeat(40)).is_err());
    assert!(r.commit_file_diff(&head(&repo), "missing.txt").is_err());
}

fn git_no_index(old: &str, new: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::write(&a, old).unwrap();
    std::fs::write(&b, new).unwrap();
    let out = Command::new("git")
        .args(diff_args())
        .arg("--no-index")
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    cli_hunks(&String::from_utf8(out.stdout).unwrap())
}

/// Replays `hunks` over `old`: every context and removed line must match `old`,
/// line numbers must count up on each side, and the result must be `new`.
fn check_hunks_rebuild(old: &str, new: &str, hunks: &[Hunk]) -> Result<(), TestCaseError> {
    let old_lines: Vec<&str> = old.lines().collect();
    let mut rebuilt: Vec<String> = Vec::new();
    let mut o = 0usize;
    let mut prev_end: Option<usize> = None;
    for h in hunks {
        let first_old = if h.old_lines == 0 {
            h.old_start
        } else {
            h.old_start - 1
        } as usize;
        if let Some(end) = prev_end {
            // Hunks whose context would touch are one hunk.
            prop_assert!(first_old > end, "hunks this close should have merged");
        }
        prop_assert!(first_old >= o);
        rebuilt.extend(old_lines[o..first_old].iter().map(|s| (*s).to_owned()));
        o = first_old;
        let mut n = rebuilt.len();
        let leading = h
            .lines
            .iter()
            .take_while(|l| l.kind == LineKind::Context)
            .count();
        let trailing = h
            .lines
            .iter()
            .rev()
            .take_while(|l| l.kind == LineKind::Context)
            .count();
        prop_assert!(leading <= 3 && trailing <= 3);
        for l in &h.lines {
            match l.kind {
                LineKind::Context | LineKind::Removed => {
                    prop_assert_eq!(l.old, Some(o as u32 + 1));
                    prop_assert_eq!(l.text.as_str(), old_lines[o]);
                    o += 1;
                }
                LineKind::Added => prop_assert_eq!(l.old, None),
            }
            match l.kind {
                LineKind::Context | LineKind::Added => {
                    prop_assert_eq!(l.new, Some(n as u32 + 1));
                    rebuilt.push(l.text.clone());
                    n += 1;
                }
                LineKind::Removed => prop_assert_eq!(l.new, None),
            }
        }
        prev_end = Some(o);
    }
    rebuilt.extend(old_lines[o..].iter().map(|s| (*s).to_owned()));
    prop_assert_eq!(rebuilt, new.lines().map(str::to_owned).collect::<Vec<_>>());
    Ok(())
}

/// Small-alphabet text: lines repeat, so several diffs are equally right.
fn ambiguous_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!["a", "b", "c", "{", "}", "", "  x", "fn f()"]),
        0..40,
    )
    .prop_map(|lines| lines.iter().map(|l| format!("{l}\n")).collect())
}

/// An edit of a file whose lines are all distinct: only one diff is right.
fn unique_edit() -> impl Strategy<Value = (String, String)> {
    (
        1usize..60,
        prop::collection::vec((0u8..4, any::<u16>()), 0..12),
    )
        .prop_map(|(n, edits)| {
            let old: Vec<String> = (0..n).map(|i| format!("line {i}")).collect();
            let mut new = old.clone();
            for (k, (op, at)) in edits.into_iter().enumerate() {
                let at = usize::from(at) % (new.len() + 1);
                match op {
                    0 if at < new.len() => drop(new.remove(at)),
                    1 if at < new.len() => new[at] = format!("changed {k}"),
                    _ => new.insert(at, format!("inserted {k}")),
                }
            }
            let join = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
            (join(&old), join(&new))
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // gix's histogram can pair repeated lines differently from git's (both are
    // correct diffs of the same size), so ambiguous texts are checked for
    // correctness, not for equality with git.
    #[test]
    fn hunks_always_rebuild_both_sides(old in ambiguous_text(), new in ambiguous_text()) {
        check_hunks_rebuild(&old, &new, &text_hunks(&old, &new))?;
    }

    #[test]
    fn hunks_match_git_when_the_diff_is_unambiguous((old, new) in unique_edit()) {
        let hunks = text_hunks(&old, &new);
        check_hunks_rebuild(&old, &new, &hunks)?;
        prop_assert_eq!(render(&hunks), git_no_index(&old, &new));
    }
}
