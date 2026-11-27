use std::time::Instant;

use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;
use gitgud_core::worktree::{HunkHeader, Side};

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

fn cached(repo: &TestRepo) -> String {
    repo.git(&["diff", "--cached", "--name-only"])
        .trim()
        .to_owned()
}

fn head(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "HEAD"]).trim().to_owned()
}

fn setup() -> (TestRepo, Session, u32) {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("b.txt", "b\n")
        .commit_all("init");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    (repo, session, id)
}

#[test]
fn undo_and_redo_a_stage() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    assert_eq!(cached(&repo), "a.txt");

    let undone = session.undo(id).unwrap();
    assert_eq!(undone.action, "Stage a.txt");
    assert_eq!(cached(&repo), "");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "changed\n"
    );

    let redone = session.redo(id).unwrap();
    assert_eq!(redone.action, "Stage a.txt");
    assert_eq!(cached(&repo), "a.txt");
}

#[test]
fn undo_a_commit_keeps_the_changes_staged_and_redo_brings_it_back() {
    let (repo, session, id) = setup();
    let before = head(&repo);
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Commit \"change a\"", |r, g| r.commit(g, "change a"))
        .unwrap();
    let committed = head(&repo);
    assert_ne!(committed, before);

    let undone = session.undo(id).unwrap();
    assert_eq!(undone.action, "Commit \"change a\"");
    assert_eq!(head(&repo), before);
    assert_eq!(
        cached(&repo),
        "a.txt",
        "the commit's changes are staged again"
    );
    assert_eq!(
        undone.opened.graph_version, 3,
        "graph rebuilt on stage (WIP row appears), commit, and undo"
    );
    assert!(
        repo.git(&["reflog", "-1", "main"])
            .contains("gitgud: undo Commit \"change a\"")
    );

    session.redo(id).unwrap();
    assert_eq!(head(&repo), committed);
    assert_eq!(cached(&repo), "");
}

#[test]
fn undoes_several_steps_newest_first() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "1\n").write("b.txt", "2\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();

    assert_eq!(session.undo(id).unwrap().action, "Stage b.txt");
    assert_eq!(cached(&repo), "a.txt");
    assert_eq!(session.undo(id).unwrap().action, "Stage a.txt");
    assert_eq!(cached(&repo), "");
    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to undo")
    );

    assert_eq!(session.redo(id).unwrap().action, "Stage a.txt");
    assert_eq!(session.redo(id).unwrap().action, "Stage b.txt");
    assert!(
        session
            .redo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to redo")
    );
}

#[test]
fn a_new_action_after_undo_drops_the_redo() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "1\n").write("b.txt", "2\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session.undo(id).unwrap();
    session
        .write(id, "Stage b.txt", |r, g| r.stage(g, &paths(&["b.txt"])))
        .unwrap();

    assert!(session.redo(id).is_err());
    assert_eq!(session.undo(id).unwrap().action, "Stage b.txt");
    assert!(session.undo(id).is_err());
}

#[test]
fn undo_the_first_commit_returns_to_an_unborn_branch() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1\n");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session
        .write(id, "Stage first.txt", |r, g| {
            r.stage(g, &paths(&["first.txt"]))
        })
        .unwrap();
    session
        .write(id, "Commit \"first\"", |r, g| r.commit(g, "first"))
        .unwrap();

    session.undo(id).unwrap();

    assert!(
        !repo.git_ok(&["rev-parse", "-q", "--verify", "HEAD"]),
        "main has no commit again"
    );
    assert_eq!(cached(&repo), "first.txt");
    session.redo(id).unwrap();
    assert_eq!(repo.git(&["log", "--format=%s"]).trim(), "first");
}

#[test]
fn undo_a_hunk_stage() {
    let (repo, session, id) = setup();
    let lines: String = (1..=20).map(|i| format!("l{i}\n")).collect();
    repo.write("f.txt", &lines).commit_all("f");
    repo.write(
        "f.txt",
        &lines.replace("l2\n", "two\n").replace("l18\n", "x\n"),
    );
    let open = session.get(id).unwrap();
    let hunk = HunkHeader::from(
        &open
            .repo
            .worktree_file_diff("f.txt", Side::Unstaged)
            .unwrap()
            .hunks[0],
    );
    session
        .write(id, "Stage hunk in f.txt", |r, g| {
            r.stage_hunk(g, "f.txt", hunk)
        })
        .unwrap();
    assert_eq!(cached(&repo), "f.txt");

    session.undo(id).unwrap();

    assert_eq!(cached(&repo), "");
}

#[test]
fn a_failed_action_is_not_recorded() {
    let (repo, session, id) = setup();
    let err = session
        .write(id, "Commit \"nothing\"", |r, g| r.commit(g, "nothing"))
        .unwrap_err();
    assert!(err.to_string().contains("nothing to commit"));

    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("nothing to undo")
    );
    let oplog = repo.path().join(".git/gitgud/oplog");
    let leftovers: Vec<_> = std::fs::read_dir(&oplog)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "index"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "no index copies left behind: {leftovers:?}"
    );
}

#[test]
fn refuses_to_undo_after_an_outside_commit() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    repo.git(&["commit", "-q", "-m", "from a terminal"]);
    let tip = head(&repo);

    let err = session.undo(id).unwrap_err();

    assert!(err.to_string().contains("changed outside gitgud"), "{err}");
    assert_eq!(head(&repo), tip, "nothing moved");
}

#[test]
fn refuses_to_undo_after_outside_staging_but_not_after_a_mere_git_status() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n").write("b.txt", "also\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();

    // `git status` rewrites the index's stat data but stages nothing.
    repo.write("a.txt", "changed\n");
    repo.git(&["status"]);
    session.undo(id).unwrap();
    session.redo(id).unwrap();

    repo.git(&["add", "b.txt"]);
    assert!(
        session
            .undo(id)
            .unwrap_err()
            .to_string()
            .contains("changed outside gitgud")
    );
}

#[test]
fn respects_another_gits_index_lock() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();

    let err = session.undo(id).unwrap_err();

    assert!(err.to_string().contains("index.lock"), "{err}");
    assert!(lock.exists(), "someone else's lock is left alone");
    std::fs::remove_file(&lock).unwrap();
    session.undo(id).unwrap();
}

#[test]
fn a_lock_taken_during_undo_of_a_commit_moves_nothing() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session
        .write(id, "Commit", |r, g| r.commit(g, "commit a"))
        .unwrap();
    let tip = head(&repo);
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();

    assert!(session.undo(id).is_err());
    assert_eq!(head(&repo), tip, "the branch stays where the index is");
    std::fs::remove_file(&lock).unwrap();
    assert_eq!(session.undo(id).unwrap().action, "Commit");
    assert_eq!(cached(&repo), "a.txt");
}

#[test]
fn a_failed_action_keeps_redo() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session.undo(id).unwrap();

    session
        .write(id, "Refused", |_, _| Err(Error::Refused("no".into())))
        .unwrap_err();

    assert_eq!(session.redo(id).unwrap().action, "Stage a.txt");
    assert_eq!(cached(&repo), "a.txt");
}

#[test]
fn a_failed_action_that_changed_something_can_be_undone() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage then fail", |r, g| {
            r.stage(g, &paths(&["a.txt"]))?;
            Err(Error::Refused("stopped halfway".into()))
        })
        .unwrap_err();
    assert_eq!(cached(&repo), "a.txt");

    assert_eq!(session.undo(id).unwrap().action, "Stage then fail (failed)");
    assert_eq!(cached(&repo), "");
}

#[test]
fn a_repo_open_twice_still_writes_one_at_a_time() {
    let (repo, session, first) = setup();
    let second = session.open(repo.path()).unwrap().id;
    let (started, wait) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let order = std::sync::Mutex::new(Vec::new());
    let (session, order) = (&session, &order);

    std::thread::scope(|s| {
        s.spawn(move || {
            session
                .write(first, "slow", |_, _| {
                    started.send(()).unwrap();
                    released.recv().unwrap();
                    order.lock().unwrap().push("first");
                    Ok(())
                })
                .unwrap();
        });
        wait.recv().unwrap();
        let other = s.spawn(move || {
            session
                .write(second, "quick", |_, _| {
                    order.lock().unwrap().push("second");
                    Ok(())
                })
                .unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(150));
        release.send(()).unwrap();
        other.join().unwrap();
    });
    assert_eq!(*order.lock().unwrap(), ["first", "second"]);
}

#[test]
fn a_write_that_panics_doesnt_lock_the_repo_for_good() {
    let (repo, session, id) = setup();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.write(id, "boom", |_, _| panic!("a bug"))
    }));
    assert!(panicked.is_err());
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    assert_eq!(cached(&repo), "a.txt");
}

#[test]
fn the_log_survives_reopening_the_repo() {
    let (repo, session, id) = setup();
    repo.write("a.txt", "changed\n");
    session
        .write(id, "Stage a.txt", |r, g| r.stage(g, &paths(&["a.txt"])))
        .unwrap();
    session.close(id).unwrap();

    let again = Session::default();
    let id2 = again.open(repo.path()).unwrap().id;
    assert_eq!(again.undo(id2).unwrap().action, "Stage a.txt");
}

#[test]
fn recording_an_entry_is_cheap() {
    let (repo, session, id) = setup();
    for i in 0..500 {
        repo.write(&format!("many/f{i}.txt"), "x");
    }
    repo.commit_all("many files");
    let open = session.get(id).unwrap();
    let oplog = gitgud_core::oplog::OpLog::for_repo(&open.repo);

    let t = Instant::now();
    oplog.record(&open.repo, "noop", || Ok(())).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;

    // Spec §6: ≤ 10 ms for a ref / index action's entry; generous for a debug build.
    assert!(ms < 50.0, "recording took {ms:.1} ms");
}

#[test]
fn unknown_repo_ids_fail() {
    let session = Session::default();
    assert!(matches!(session.undo(7), Err(Error::UnknownRepo(7))));
    assert!(matches!(session.redo(7), Err(Error::UnknownRepo(7))));
}

#[test]
fn prune_now_drops_expired_snapshots_and_keeps_active_ones_across_git_gc() {
    let repo = TestRepo::init();
    repo.write("a.txt", "base-a\n")
        .write("b.txt", "base-b\n")
        .commit_all("init");
    let r = gitgud_core::repo::open(repo.path()).unwrap();
    let git = gitgud_core::cli::GitCli::new(&gitgud_core::git::locate().unwrap());
    let log = gitgud_core::oplog::OpLog::for_repo(&r);

    repo.write("a.txt", "discarded-one-unique\n");
    log.record_destructive(&r, &git, "Discard a.txt", &paths(&["a.txt"]), || {
        repo.write("a.txt", "base-a\n");
        Ok(())
    })
    .unwrap();

    repo.write("b.txt", "discarded-two-unique\n");
    log.record_destructive(&r, &git, "Discard b.txt", &paths(&["b.txt"]), || {
        repo.write("b.txt", "base-b\n");
        Ok(())
    })
    .unwrap();

    let log_path = repo.path().join(".git/gitgud/oplog/log.json");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&log_path).unwrap()).unwrap();
    let arr = raw.as_array_mut().unwrap();
    assert_eq!(arr.len(), 2);
    let old_tree = arr[0]["before"]["worktree"].as_str().unwrap().to_owned();
    let kept_tree = arr[1]["before"]["worktree"].as_str().unwrap().to_owned();
    assert_ne!(old_tree, kept_tree);

    let expired = arr[0]["time"].as_i64().unwrap() - 40 * 24 * 60 * 60;
    arr[0]["time"] = serde_json::json!(expired);
    std::fs::write(&log_path, serde_json::to_vec(&raw).unwrap()).unwrap();

    let stats = log.prune_now(&r, &git, 1, 30 * 24 * 60 * 60).unwrap();
    assert_eq!(stats.entry_count, 1);

    git.run(repo.path(), &["gc", "--prune=now"], None).unwrap();
    assert!(
        git.run(repo.path(), &["cat-file", "-e", &old_tree], None)
            .is_err(),
        "pruned snapshot tree should be collected by git gc"
    );
    git.run(repo.path(), &["cat-file", "-e", &kept_tree], None)
        .unwrap();

    let kept_id = log.entries().unwrap()[0].id;
    log.restore_entry_files(&r, &git, kept_id).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("b.txt")).unwrap(),
        "discarded-two-unique\n"
    );

    let mut raw2: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&log_path).unwrap()).unwrap();
    raw2[0]["time"] = serde_json::json!(expired);
    std::fs::write(&log_path, serde_json::to_vec(&raw2).unwrap()).unwrap();

    let empty_stats = log.prune_now(&r, &git, 0, 30 * 24 * 60 * 60).unwrap();
    assert_eq!(empty_stats.entry_count, 0);
    assert!(
        git.run(
            repo.path(),
            &[
                "show-ref",
                "--verify",
                "--quiet",
                gitgud_core::snapshot::SNAPSHOT_REF
            ],
            None,
        )
        .is_err(),
        "SNAPSHOT_REF should be deleted when zero entries remain"
    );
}

fn write_fake_lfs_object(git_dir: &std::path::Path, oid: &str, bytes: usize) {
    let dir = git_dir.join("lfs/objects").join(&oid[..2]).join(&oid[2..4]);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(oid), vec![0xabu8; bytes]).unwrap();
}

fn dir_files_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

#[test]
fn stats_counts_op_log_only_lfs_objects_and_reclaimable_preview() {
    let repo = TestRepo::init();
    let live_oid = "1".repeat(64);
    let prunable_oid = "2".repeat(64);
    let kept_oid = "3".repeat(64);
    let ptr = |oid: &str, len: usize| {
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {len}\n")
    };

    repo.write("live.png", &ptr(&live_oid, 4096))
        .commit_all("commit live lfs pointer");
    let git_dir = repo.path().join(".git");
    write_fake_lfs_object(&git_dir, &live_oid, 4096);
    write_fake_lfs_object(&git_dir, &prunable_oid, 10_000);
    write_fake_lfs_object(&git_dir, &kept_oid, 6_000);

    let r = gitgud_core::repo::open(repo.path()).unwrap();
    let git = gitgud_core::cli::GitCli::new(&gitgud_core::git::locate().unwrap());
    let log = gitgud_core::oplog::OpLog::for_repo(&r);

    repo.write("prunable.png", &ptr(&prunable_oid, 10_000));
    log.record_destructive(
        &r,
        &git,
        "Discard prunable.png",
        &paths(&["prunable.png"]),
        || {
            std::fs::remove_file(repo.path().join("prunable.png")).unwrap();
            Ok(())
        },
    )
    .unwrap();

    repo.write("kept.png", &ptr(&kept_oid, 6_000));
    log.record_destructive(&r, &git, "Discard kept.png", &paths(&["kept.png"]), || {
        std::fs::remove_file(repo.path().join("kept.png")).unwrap();
        Ok(())
    })
    .unwrap();

    let oplog_dir = git_dir.join("gitgud/oplog");
    let log_path = oplog_dir.join("log.json");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&log_path).unwrap()).unwrap();
    let arr = raw.as_array_mut().unwrap();
    assert_eq!(arr.len(), 2);
    let expired = arr[0]["time"].as_i64().unwrap() - 40 * 24 * 60 * 60;
    arr[0]["time"] = serde_json::json!(expired);
    std::fs::write(&log_path, serde_json::to_vec(&raw).unwrap()).unwrap();

    let index_len = |key: &str| {
        raw[0][key]["index"]
            .as_str()
            .and_then(|name| std::fs::metadata(oplog_dir.join(name)).ok())
            .map_or(0, |m| m.len())
    };
    let expected_entry0_index = index_len("before") + index_len("after");
    let oplog_dir_bytes = dir_files_bytes(&oplog_dir);

    let stats = log.stats_with_limits(&r, 1, 30 * 24 * 60 * 60).unwrap();
    assert_eq!(stats.entry_count, 2);
    assert_eq!(stats.prunable_count, 1);
    assert_eq!(stats.total_bytes, oplog_dir_bytes + 10_000 + 6_000);
    assert_eq!(stats.reclaimable_bytes, expected_entry0_index + 10_000);

    let pruned = log.prune_now(&r, &git, 1, 30 * 24 * 60 * 60).unwrap();
    let oplog_dir_after = dir_files_bytes(&oplog_dir);
    assert_eq!(pruned.entry_count, 1);
    assert_eq!(pruned.prunable_count, 0);
    assert_eq!(pruned.reclaimable_bytes, 0);
    assert_eq!(pruned.total_bytes, oplog_dir_after + 6_000);
}
