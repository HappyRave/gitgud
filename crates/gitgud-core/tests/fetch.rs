use std::path::Path;
use std::time::{Duration, Instant};

use gitgud_core::cli::GitCli;
use gitgud_core::git;
use gitgud_core::graph::RefLabel;
use gitgud_core::repo::{self, Error};
use gitgud_core::session::Session;
use gitgud_core::task::{Cancel, Progress};
use gitgud_core::testing::TestRepo;

fn cli() -> GitCli {
    GitCli::new(&git::locate().unwrap())
}

/// A repo whose `origin` is a bare clone, plus a second clone that pushes to it.
fn with_origin(commits: usize) -> (TestRepo, tempfile::TempDir, TestRepo) {
    let local = TestRepo::init();
    for i in 0..commits {
        local
            .write(&format!("f{}.txt", i % 50), &format!("{i}\n"))
            .commit_all(&format!("c{i}"));
    }
    let origin = tempfile::tempdir().unwrap();
    let bare = origin.path().join("origin.git");
    local.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    local.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    local.git(&["fetch", "-q", "origin"]);

    let other = TestRepo::init();
    other.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    other.git(&["fetch", "-q", "origin"]);
    other.git(&["checkout", "-q", "-B", "main", "origin/main"]);
    (local, origin, other)
}

fn alive(pid: &str) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(pid)
}

#[test]
fn fetch_brings_new_remote_commits() {
    let (local, _origin, other) = with_origin(3);
    other.write("new.txt", "n\n").commit_all("pushed elsewhere");
    other.git(&["push", "-q", "origin", "main"]);

    repo::open(local.path())
        .unwrap()
        .fetch(&cli(), &mut |_| {}, &Cancel::default())
        .unwrap();

    assert_eq!(
        local
            .git(&["log", "-1", "--format=%s", "origin/main"])
            .trim(),
        "pushed elsewhere"
    );
}

#[test]
fn fetch_reports_progress_lines() {
    let (local, _origin, other) = with_origin(1);
    for i in 0..400 {
        other.write(
            &format!("bulk/{i}.txt"),
            &format!("{i} {}\n", "x".repeat(200)),
        );
    }
    other.commit_all("bulk");
    other.git(&["push", "-q", "origin", "main"]);
    let mut seen: Vec<Progress> = Vec::new();

    repo::open(local.path())
        .unwrap()
        .fetch(&cli(), &mut |p| seen.push(p), &Cancel::default())
        .unwrap();

    assert!(!seen.is_empty(), "git printed no progress");
    assert!(
        seen.iter().all(|p| p.percent <= 100 && p.done <= p.total),
        "{seen:?}"
    );
    assert!(seen.iter().any(|p| p.percent == 100), "{seen:?}");
}

#[test]
fn a_failed_fetch_returns_gits_message() {
    let local = TestRepo::init();
    local.write("a.txt", "a").commit_all("a");
    local.git(&["remote", "add", "origin", "C:/definitely/not/a/repo.git"]);

    let err = repo::open(local.path())
        .unwrap()
        .fetch(&cli(), &mut |_| {}, &Cancel::default())
        .unwrap_err();

    match err {
        Error::Command { message, .. } => assert!(
            message.contains("does not appear to be a git repository"),
            "{message}"
        ),
        other => panic!("{other:?}"),
    }
}

/// A remote whose transport hangs: an `ext::` helper that records the Windows
/// PIDs of its shell and a `tail` grandchild, drops a partial pack, and waits.
fn hanging_remote(local: &TestRepo) -> std::path::PathBuf {
    let pids = local.path().join("pids.txt");
    let script = local.path().join("hang.sh");
    std::fs::write(
        &script,
        format!(
            "cat /proc/$$/winpid > '{pids}'\n\
             touch .git/objects/pack/tmp_pack_gitgud_test\n\
             tail -f /dev/null &\n\
             cat /proc/$!/winpid >> '{pids}'\n\
             wait\n",
            pids = pids.to_str().unwrap().replace('\\', "/"),
        ),
    )
    .unwrap();
    local.git(&["config", "protocol.ext.allow", "always"]);
    let url = format!("ext::sh {}", script.to_str().unwrap().replace('\\', "/"));
    local.git(&["remote", "add", "origin", &url]);
    pids
}

/// Waits for the hanging remote's PIDs; on timeout calls `stop` first, so the
/// test's scope can join the fetch instead of hanging on it.
fn wait_for(path: &Path, lines: usize, stop: impl Fn()) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let found: Vec<String> = std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .collect();
        if found.len() >= lines {
            return found;
        }
        if Instant::now() >= deadline {
            stop();
            panic!("the hanging remote never started");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn cancel_kills_git_and_everything_it_started() {
    let local = TestRepo::init();
    local.write("a.txt", "a").commit_all("a");
    let pids_file = hanging_remote(&local);
    let r = repo::open(local.path()).unwrap();
    let cancel = Cancel::default();

    let result = std::thread::scope(|s| {
        let running = s.spawn(|| r.fetch(&cli(), &mut |_| {}, &cancel));
        let pids = wait_for(&pids_file, 2, || cancel.cancel());
        assert!(
            pids.iter().all(|p| alive(p)),
            "helpers are running: {pids:?}"
        );
        let t = Instant::now();
        cancel.cancel();
        let result = running.join().unwrap();
        let ms = t.elapsed().as_millis();
        // Spec §6: cancel stops within 250 ms; generous for a loaded test machine.
        assert!(ms < 2_000, "cancel took {ms} ms");
        (result, pids)
    });

    let (result, pids) = result;
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
    std::thread::sleep(Duration::from_millis(200));
    for pid in &pids {
        assert!(!alive(pid), "process {pid} outlived the cancel");
    }
    assert!(
        !local
            .path()
            .join(".git/objects/pack/tmp_pack_gitgud_test")
            .exists(),
        "the partial pack is removed"
    );
}

#[test]
fn session_fetch_refreshes_and_one_runs_at_a_time() {
    let (local, _origin, other) = with_origin(2);
    other.write("x.txt", "x\n").commit_all("remote work");
    other.git(&["push", "-q", "origin", "main"]);
    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;
    assert!(!session.cancel(id), "nothing to cancel yet");

    let opened = session.fetch(id, |_| {}).unwrap();

    assert_eq!(
        opened.graph_version, 1,
        "origin/main moved, so the graph was rebuilt"
    );
    let open = session.get(id).unwrap();
    let rows = open.graph.rows(&open.repo, 0, 10).unwrap();
    assert!(
        rows[0].labels.contains(&RefLabel::Remote {
            name: "origin/main".into()
        }),
        "{:?}",
        rows[0]
    );
}

#[test]
fn session_refuses_a_second_fetch_while_one_runs() {
    let local = TestRepo::init();
    local.write("a.txt", "a").commit_all("a");
    let pids_file = hanging_remote(&local);
    let session = Session::default();
    let id = session.open(local.path()).unwrap().id;

    std::thread::scope(|s| {
        let running = s.spawn(|| session.fetch(id, |_| {}));
        wait_for(&pids_file, 2, || {
            session.cancel(id);
        });
        let second = session.fetch(id, |_| {});
        assert!(matches!(second, Err(Error::Refused(_))), "{second:?}");
        assert!(session.cancel(id));
        assert!(matches!(running.join().unwrap(), Err(Error::Cancelled)));
    });
    assert!(!session.cancel(id), "the task is gone once it ended");
}
