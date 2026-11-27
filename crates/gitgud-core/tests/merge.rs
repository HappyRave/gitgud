use gitgud_core::merge::{Chunk, Conflict, ConflictKind, ConflictSides, Side, SideInfo};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

fn merging(repo: &TestRepo) -> bool {
    repo.path().join(".git/MERGE_HEAD").exists()
}

fn head(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "HEAD"]).trim().to_owned()
}

/// `main` and `other` both edit line 3 of a.txt; `other` also adds b.txt.
fn diverged() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "one\ntwo\nthree\nfour\nfive\n")
        .commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", "one\ntwo\nTHREE theirs\nfour\nfive\n")
        .write("b.txt", "added on other\n")
        .commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", "one\ntwo\nthree yours\nfour\nfive\n")
        .commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

#[test]
fn a_commit_without_a_branch_merges_by_its_id() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("b.txt", "b\n").commit_all("first on other");
    let commit = head(&repo);
    repo.write("b.txt", "b2\n").commit_all("second on other");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("c.txt", "c\n").commit_all("yours");
    let before = head(&repo);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    let git = session.git().unwrap();
    let r = session.get(id).unwrap().repo.clone();
    assert!(r.predict_merge(&git, &commit).unwrap().is_empty());
    session.merge(id, &commit).unwrap();

    assert_eq!(
        read(&repo, "b.txt").unwrap(),
        "b\n",
        "only up to that commit"
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD^2"]).trim(), commit);
    session.undo(id).unwrap();
    assert_eq!(head(&repo), before);
}

#[test]
fn a_clean_merge_commits_and_undo_takes_it_back() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("b.txt", "b\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("c.txt", "c\n").commit_all("yours");
    let before = head(&repo);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    session.merge(id, "other").unwrap();

    assert!(!merging(&repo));
    assert_eq!(
        repo.git(&["rev-list", "--parents", "-n1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert_eq!(read(&repo, "b.txt").unwrap(), "b\n");
    assert!(session.merge_state(id).unwrap().is_none());

    assert_eq!(session.undo(id).unwrap().action, "Merge other");
    assert_eq!(head(&repo), before);
    assert_eq!(read(&repo, "b.txt"), None);
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "");
}

#[test]
fn prediction_names_the_conflicts_and_changes_nothing() {
    let (repo, session, id) = diverged();
    let open = session.get(id).unwrap();
    let git = &*session.git().unwrap();

    assert_eq!(open.repo.predict_merge(git, "other").unwrap(), ["a.txt"]);
    repo.git(&["branch", "fresh", "HEAD~1"]);
    assert!(open.repo.predict_merge(git, "fresh").unwrap().is_empty());
    assert!(!merging(&repo));
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_conflicted_merge_stops_with_its_files() {
    let (repo, session, id) = diverged();

    session.merge(id, "other").unwrap();

    assert!(merging(&repo));
    let state = session.merge_state(id).unwrap().unwrap();
    assert_eq!(state.theirs.as_deref(), Some("other"));
    assert_eq!(state.message, "Merge branch 'other'");
    assert_eq!(
        state.conflicts,
        [Conflict {
            path: "a.txt".into(),
            kind: ConflictKind::Code,
            resolved: false
        }]
    );
    assert_eq!(
        read(&repo, "b.txt").unwrap(),
        "added on other\n",
        "clean parts are merged already"
    );
}

#[test]
fn conflicts_are_grouped_unity_code_binary() {
    let repo = TestRepo::init();
    repo.git(&["config", "filter.lfs.clean", "cat"]);
    repo.git(&["config", "filter.lfs.smudge", "cat"]);
    repo.git(&["config", "filter.lfs.required", "false"]);
    repo.write(".gitattributes", "*.psd filter=lfs\n")
        .write("Ship.prefab", "a: 1\n")
        .write("Engine.cs", "int x = 1;\n")
        .write("Art.psd", "layers 1\n")
        .write("gone.txt", "base\n")
        .commit_all("base");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 1]).unwrap();
    repo.commit_all("bin");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("Ship.prefab", "a: 2\n")
        .write("Engine.cs", "int x = 2;\n")
        .write("Art.psd", "layers 2\n");
    repo.write("gone.txt", "changed on other\n");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 2]).unwrap();
    repo.commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("Ship.prefab", "a: 3\n")
        .write("Engine.cs", "int x = 3;\n")
        .write("Art.psd", "layers 3\n");
    repo.remove("gone.txt");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 3]).unwrap();
    repo.commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    session.merge(id, "other").unwrap();

    let kinds: Vec<(String, ConflictKind)> = session
        .merge_state(id)
        .unwrap()
        .unwrap()
        .conflicts
        .into_iter()
        .map(|c| (c.path, c.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Ship.prefab".into(), ConflictKind::UnityYaml),
            ("Engine.cs".into(), ConflictKind::Code),
            ("Art.psd".into(), ConflictKind::Binary),
            ("bin.dat".into(), ConflictKind::Binary),
            ("gone.txt".into(), ConflictKind::Binary),
        ]
    );

    let repo_ = session.get(id).unwrap().repo.clone();
    assert_eq!(
        repo_.conflict_sides("bin.dat").unwrap(),
        ConflictSides {
            yours: Some(SideInfo {
                size: 2,
                lfs: false,
                oid: None,
                hydrated: true,
            }),
            theirs: Some(SideInfo {
                size: 2,
                lfs: false,
                oid: None,
                hydrated: true,
            }),
        }
    );
    assert_eq!(repo_.conflict_sides("gone.txt").unwrap().yours, None);
    assert!(repo_.conflict_sides("Engine.cs").is_ok());
    assert!(repo_.conflict_sides("nope.txt").is_err());
}

#[test]
fn lfs_pointers_report_the_real_size() {
    let repo = TestRepo::init();
    let pointer = |n: u32| {
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{n:064}\nsize {n}000\n")
    };
    repo.write("t.png", &pointer(1)).commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("t.png", &pointer(2)).commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("t.png", &pointer(3)).commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();

    let sides = session
        .get(id)
        .unwrap()
        .repo
        .conflict_sides("t.png")
        .unwrap();
    let expected_oid = format!("{:064}", 2);
    assert_eq!(
        sides.theirs,
        Some(SideInfo {
            size: 2000,
            lfs: true,
            oid: Some(expected_oid.clone()),
            hydrated: false,
        })
    );

    let obj = repo
        .path()
        .join(".git/lfs/objects")
        .join(&expected_oid[..2])
        .join(&expected_oid[2..4])
        .join(&expected_oid);
    std::fs::create_dir_all(obj.parent().unwrap()).unwrap();
    std::fs::write(obj, b"png").unwrap();

    let updated = session
        .get(id)
        .unwrap()
        .repo
        .conflict_sides("t.png")
        .unwrap();
    assert!(updated.theirs.unwrap().hydrated);
}

#[test]
fn conflict_text_chunks_the_file_like_git() {
    let (_repo, session, id) = diverged();
    session.merge(id, "other").unwrap();
    let open = session.get(id).unwrap();

    let text = open
        .repo
        .conflict_text(&session.git().unwrap(), "a.txt")
        .unwrap();

    assert!(text.final_newline);
    assert_eq!(
        text.chunks,
        [
            Chunk::Same {
                lines: vec!["one".into(), "two".into()]
            },
            Chunk::Conflict {
                yours: vec!["three yours".into()],
                base: vec!["three".into()],
                theirs: vec!["THREE theirs".into()],
            },
            Chunk::Same {
                lines: vec!["four".into(), "five".into()]
            },
        ]
    );
}

#[test]
fn files_that_contain_marker_lines_still_parse() {
    let repo = TestRepo::init();
    let tricky = "<<<<<<< not a marker\n=======\n>>>>>>> also not\nline\n";
    repo.write("t.txt", tricky).commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("t.txt", &tricky.replace("line", "theirs"))
        .commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("t.txt", &tricky.replace("line", "yours"))
        .commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();

    let text = session
        .get(id)
        .unwrap()
        .repo
        .conflict_text(&session.git().unwrap(), "t.txt")
        .unwrap();

    assert_eq!(
        text.chunks[0],
        Chunk::Same {
            lines: vec![
                "<<<<<<< not a marker".into(),
                "=======".into(),
                ">>>>>>> also not".into()
            ]
        }
    );
    assert!(
        matches!(&text.chunks[1], Chunk::Conflict { yours, theirs, .. } if yours == &["yours"] && theirs == &["theirs"])
    );
}

#[test]
fn resolve_with_text_stages_it_and_ctrl_z_brings_the_conflict_back() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    session
        .resolve_with(
            id,
            "a.txt",
            "one\ntwo\nthree yours\nTHREE theirs\nfour\nfive\n",
            "use both",
        )
        .unwrap();

    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "one\ntwo\nthree yours\nTHREE theirs\nfour\nfive\n"
    );
    let state = session.merge_state(id).unwrap().unwrap();
    assert!(state.conflicts[0].resolved);
    assert_eq!(
        session.op_log(id).unwrap()[0].action,
        "Resolve a.txt (use both)"
    );

    session.undo(id).unwrap();
    assert!(!session.merge_state(id).unwrap().unwrap().conflicts[0].resolved);
    assert!(
        read(&repo, "a.txt").unwrap().contains("<<<<<<<"),
        "the conflict is back in the file"
    );
}

#[test]
fn resolve_with_writes_crlf_for_autocrlf_users() {
    let repo = TestRepo::init();
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.write("w.txt", "a\r\nb\r\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("w.txt", "a\r\ntheirs\r\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("w.txt", "a\r\nyours\r\n").commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();

    session
        .resolve_with(id, "w.txt", "a\nyours\ntheirs\n", "use both")
        .unwrap();

    assert_eq!(read(&repo, "w.txt").unwrap(), "a\r\nyours\r\ntheirs\r\n");
    assert_eq!(
        repo.git(&["status", "--porcelain"]).trim(),
        "M  w.txt",
        "staged, and the work tree matches it"
    );
}

fn staged_bytes(repo: &TestRepo, path: &str) -> Vec<u8> {
    std::process::Command::new("git")
        .args(["show", &format!(":{path}")])
        .current_dir(repo.path())
        .output()
        .unwrap()
        .stdout
}

/// `main` and `other` both edit the last line of `path`, committed as given.
fn diverged_bytes(base: &[u8], theirs: &[u8], yours: &[u8]) -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    let write = |bytes: &[u8], msg: &str| {
        std::fs::write(repo.path().join("f.txt"), bytes).unwrap();
        repo.commit_all(msg);
    };
    write(base, "base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    write(theirs, "theirs");
    repo.git(&["checkout", "-q", "main"]);
    write(yours, "yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();
    (repo, session, id)
}

#[test]
fn conflicts_of_several_files_read_at_once_stay_apart() {
    let repo = TestRepo::init();
    let names: Vec<String> = (0..6).map(|i| format!("f{i}.txt")).collect();
    for n in &names {
        repo.write(n, &format!("{n} base\n"));
    }
    repo.commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    for n in &names {
        repo.write(n, &format!("{n} theirs\n"));
    }
    repo.commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    for n in &names {
        repo.write(n, &format!("{n} yours\n"));
    }
    repo.commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();
    let r = session.get(id).unwrap().repo.clone();
    let git = session.git().unwrap();

    std::thread::scope(|s| {
        for n in &names {
            let (r, git) = (&r, &git);
            s.spawn(move || {
                for _ in 0..5 {
                    let text = r.conflict_text(git, n).unwrap();
                    let Chunk::Conflict { yours, theirs, .. } = &text.chunks[0] else {
                        panic!("{text:?}");
                    };
                    assert_eq!(yours, &[format!("{n} yours")]);
                    assert_eq!(theirs, &[format!("{n} theirs")]);
                }
            });
        }
    });
    assert!(
        !repo
            .path()
            .join(".git/gitgud/merge-file")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
}

#[test]
fn resolving_a_crlf_file_keeps_crlf_in_the_repo() {
    let (repo, session, id) = diverged_bytes(
        b"keep\r\nb\r\n",
        b"keep\r\ntheirs\r\n",
        b"keep\r\nyours\r\n",
    );
    let git = session.git().unwrap();
    let text = session
        .get(id)
        .unwrap()
        .repo
        .conflict_text(&git, "f.txt")
        .unwrap();
    assert_eq!(
        text.chunks[0],
        Chunk::Same {
            lines: vec!["keep".into()]
        }
    );

    session
        .resolve_with(id, "f.txt", "keep\nyours\ntheirs\n", "use both")
        .unwrap();

    assert_eq!(staged_bytes(&repo, "f.txt"), b"keep\r\nyours\r\ntheirs\r\n");
}

#[test]
fn resolving_a_latin1_file_keeps_its_bytes() {
    let (repo, session, id) = diverged_bytes(
        b"caf\xe9\nb\n",
        b"caf\xe9\ntheirs \xe0\n",
        b"caf\xe9\nyours\n",
    );
    let git = session.git().unwrap();
    let text = session
        .get(id)
        .unwrap()
        .repo
        .conflict_text(&git, "f.txt")
        .unwrap();
    let Chunk::Same { lines } = &text.chunks[0] else {
        panic!("{text:?}");
    };
    assert_eq!(lines, &vec!["caf\u{e9}".to_owned()]);
    let Chunk::Conflict { theirs, .. } = &text.chunks[1] else {
        panic!("{text:?}");
    };

    let err = session
        .resolve_with(id, "f.txt", "caf\u{e9} \u{20ac}\n", "edited")
        .unwrap_err();
    assert!(err.to_string().contains("isn't UTF-8"), "{err}");

    let merged = format!("caf\u{e9}\n{}\n", theirs[0]);
    session
        .resolve_with(id, "f.txt", &merged, "take theirs")
        .unwrap();
    assert_eq!(staged_bytes(&repo, "f.txt"), b"caf\xe9\ntheirs \xe0\n");
}

#[test]
fn resolve_side_keeps_one_whole_and_deletions_stay_deleted() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    session.resolve_side(id, "a.txt", Side::Theirs).unwrap();
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "one\ntwo\nTHREE theirs\nfour\nfive\n"
    );
    assert!(session.merge_state(id).unwrap().unwrap().conflicts[0].resolved);

    let repo2 = TestRepo::init();
    repo2.write("f.txt", "base\n").commit_all("base");
    repo2.git(&["checkout", "-q", "-b", "other"]);
    repo2.write("f.txt", "edited\n").commit_all("edit");
    repo2.git(&["checkout", "-q", "main"]);
    repo2.remove("f.txt").commit_all("delete");
    let id2 = session.open(repo2.path()).unwrap().id;
    session.merge(id2, "other").unwrap();

    session.resolve_side(id2, "f.txt", Side::Yours).unwrap();

    assert_eq!(read(&repo2, "f.txt"), None, "yours deleted it");
    assert!(session.merge_state(id2).unwrap().unwrap().conflicts[0].resolved);
}

#[test]
fn commit_merge_waits_for_every_file_then_makes_a_merge_commit() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    let err = session
        .merge_commit(id, "Merge branch 'other'")
        .unwrap_err();
    assert!(err.to_string().contains("still conflicted"), "{err}");

    session.resolve_side(id, "a.txt", Side::Yours).unwrap();
    let message = session.merge_state(id).unwrap().unwrap().message;
    session.merge_commit(id, &message).unwrap();

    assert!(!merging(&repo));
    assert!(session.merge_state(id).unwrap().is_none());
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "Merge branch 'other'"
    );
    assert_eq!(
        repo.git(&["rev-list", "--parents", "-n1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert!(!repo.path().join(".git/gitgud/merge.json").exists());
}

#[test]
fn an_unreadable_merge_record_stops_the_commit_instead_of_skipping_its_checks() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();
    session.resolve_side(id, "a.txt", Side::Yours).unwrap();
    std::fs::write(repo.path().join(".git/gitgud/merge.json"), "{ not json").unwrap();
    let before = head(&repo);

    assert!(session.merge_commit(id, "Merge").is_err());
    assert_eq!(head(&repo), before);
    assert!(merging(&repo));
}

#[test]
fn abort_goes_back_and_ctrl_z_puts_the_merge_back_in_progress() {
    let (repo, session, id) = diverged();
    let before = head(&repo);
    session.merge(id, "other").unwrap();
    session.resolve_side(id, "a.txt", Side::Theirs).unwrap();

    session.merge_abort(id).unwrap();
    assert!(!merging(&repo));
    assert_eq!(head(&repo), before);
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "one\ntwo\nthree yours\nfour\nfive\n"
    );
    assert_eq!(read(&repo, "b.txt"), None);

    session.undo(id).unwrap();
    assert!(merging(&repo));
    let state = session.merge_state(id).unwrap().unwrap();
    assert!(
        state.conflicts[0].resolved,
        "the pick made before aborting is back too"
    );
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "one\ntwo\nTHREE theirs\nfour\nfive\n"
    );
}

#[test]
fn undoing_the_merge_start_leaves_no_merge_behind() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    session.undo(id).unwrap();

    assert!(!merging(&repo));
    assert!(session.merge_state(id).unwrap().is_none());
    assert_eq!(
        read(&repo, "a.txt").unwrap(),
        "one\ntwo\nthree yours\nfour\nfive\n"
    );
    assert_eq!(read(&repo, "b.txt"), None);
    assert_eq!(repo.git(&["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_second_merge_is_refused_while_one_is_in_progress() {
    let (_repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    let err = session.merge(id, "other").unwrap_err();

    assert!(err.to_string().contains("already in progress"), "{err}");
}

#[test]
fn a_merge_finished_in_a_terminal_leaves_no_stale_state_for_the_next_one() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();
    repo.write("a.txt", "resolved\n");
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-q", "--no-edit"]);
    repo.git(&["checkout", "-q", "-b", "later", "HEAD~1"]);
    repo.write("c.txt", "later\n").commit_all("later");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("c.txt", "main\n").commit_all("main c");
    assert!(!repo.git_ok(&["merge", "-q", "later"]));

    let state = session.merge_state(id).unwrap().unwrap();

    assert_eq!(state.theirs, None);
    assert_eq!(
        state.conflicts,
        [Conflict {
            path: "c.txt".into(),
            kind: ConflictKind::Code,
            resolved: false
        }]
    );
}

#[test]
fn resolve_with_refuses_a_path_that_is_not_conflicted() {
    let (repo, session, id) = diverged();
    session.merge(id, "other").unwrap();

    for path in ["b.txt", "../outside.txt"] {
        let err = session.resolve_with(id, path, "x\n", "edited").unwrap_err();
        assert!(err.to_string().contains("not conflicted"), "{path}: {err}");
    }
    assert_eq!(read(&repo, "b.txt").unwrap(), "added on other\n");
    assert!(!repo.path().join("../outside.txt").exists());
}

#[test]
fn a_hook_that_refuses_a_clean_merge_shows_its_message_and_changes_nothing() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("b.txt", "b\n").commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("c.txt", "c\n").commit_all("yours");
    let hook = repo.path().join(".git/hooks/pre-merge-commit");
    std::fs::write(
        &hook,
        "#!/bin/sh\necho 'no merges on fridays' >&2\nexit 1\n",
    )
    .unwrap();
    let before = head(&repo);
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    let err = session.merge(id, "other").unwrap_err();

    assert!(err.to_string().contains("no merges on fridays"), "{err}");
    assert!(!merging(&repo));
    assert_eq!(head(&repo), before);
    assert_eq!(read(&repo, "b.txt"), None);
}
