use std::time::Instant;

use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = f();
    println!(
        "{:>8.1} ms  {label}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    out
}

/// Where staging many files spends its time, in the order the app runs it:
/// `GITGUD_STAGE_FILES=3000 cargo test -p gitgud-core --test stage_perf -- --ignored --nocapture`.
#[test]
#[ignore = "timing run"]
fn stage_many_files() {
    let n: usize = std::env::var("GITGUD_STAGE_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let repo = TestRepo::init();
    if let Ok(v) = std::env::var("GITGUD_STAGE_AUTOCRLF") {
        repo.git(&["config", "core.autocrlf", &v]);
    }
    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.1f1\n",
    );
    for i in 0..n {
        let path = format!("Assets/Old/f{i}.cs");
        repo.write(&path, &format!("class F{i} {{}}\n"));
        repo.write(
            &format!("{path}.meta"),
            &format!("fileFormatVersion: 2\nguid: {i:032x}\n"),
        );
    }
    repo.commit_all("base");
    for i in 0..n {
        repo.write(
            &format!("Assets/Old/f{i}.cs"),
            &format!("class F{i} {{ int x; }}\n"),
        );
        let path = format!("Assets/New/g{i}.cs");
        repo.write(&path, &format!("class G{i} {{}}\n"));
        repo.write(
            &format!("{path}.meta"),
            &format!("fileFormatVersion: 2\nguid: {:032x}\n", i + n),
        );
    }
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let status = session.repo_status(id).unwrap();
    println!("{} changed entries", status.entries.len());
    let paths: Vec<String> = status.entries.iter().map(|e| e.path.clone()).collect();

    if let Ok(workers) = std::env::var("GITGUD_STAGE_PREWARM") {
        let workers: usize = workers.parse().unwrap();
        let files: Vec<&String> = paths
            .iter()
            .filter(|p| repo.path().join(p).is_file())
            .collect();
        let root = repo.path();
        timed(
            &format!(
                "PREWARM: {workers} x git -c core.autocrlf=true hash-object -w --stdin-paths ({} files)",
                files.len()
            ),
            || {
                std::thread::scope(|s| {
                    for chunk in files.chunks(files.len().div_ceil(workers)) {
                        s.spawn(move || {
                            let mut child = std::process::Command::new("git")
                                .args([
                                    "-c",
                                    "core.autocrlf=true",
                                    "hash-object",
                                    "-w",
                                    "--stdin-paths",
                                ])
                                .current_dir(root)
                                .stdin(std::process::Stdio::piped())
                                .stdout(std::process::Stdio::null())
                                .spawn()
                                .unwrap();
                            use std::io::Write;
                            let input: String = chunk.iter().map(|p| format!("{p}\n")).collect();
                            child
                                .stdin
                                .take()
                                .unwrap()
                                .write_all(input.as_bytes())
                                .unwrap();
                            assert!(child.wait().unwrap().success());
                        });
                    }
                });
            },
        );
        let input = paths.join("\0");
        timed(
            "then: git -c core.autocrlf=true add -A (objects already there)",
            || {
                let mut child = std::process::Command::new("git")
                    .args([
                        "-c",
                        "core.autocrlf=true",
                        "--literal-pathspecs",
                        "add",
                        "-A",
                        "--pathspec-from-file=-",
                        "--pathspec-file-nul",
                    ])
                    .current_dir(repo.path())
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .unwrap();
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(input.as_bytes())
                    .unwrap();
                assert!(child.wait().unwrap().success());
            },
        );
        return;
    }

    if std::env::var_os("GITGUD_STAGE_RAW_FIRST").is_some() {
        let input = paths.join("\0");
        let extra = std::env::var("GITGUD_STAGE_GITARGS").unwrap_or_default();
        timed(
            &format!("FIRST: raw git {extra} add (writes the new objects)"),
            || {
                let mut child = std::process::Command::new("git")
                    .args(extra.split_whitespace())
                    .args([
                        "--literal-pathspecs",
                        "add",
                        "-A",
                        "--pathspec-from-file=-",
                        "--pathspec-file-nul",
                    ])
                    .current_dir(repo.path())
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .unwrap();
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(input.as_bytes())
                    .unwrap();
                assert!(child.wait().unwrap().success());
            },
        );
        repo.git(&["reset", "-q"]);
        session.refresh(id).unwrap();
    }

    timed("stage (whole Session::stage)", || {
        session.stage(id, &paths).unwrap()
    });
    println!("-- what the app reads after a write --");
    timed("repo_status", || session.repo_status(id).unwrap());
    timed("merge_state", || session.merge_state(id).unwrap());
    timed("rebase_state", || session.rebase_state(id).unwrap());
    timed("sequencer_state", || session.sequencer_state(id).unwrap());
    timed("unity_info", || session.unity_info(id).unwrap());
    timed("unity_meta_issues", || {
        session.unity_meta_issues(id).unwrap()
    });
    timed("overlap", || session.overlap(id).unwrap());
    timed("op_log", || session.op_log(id).unwrap());
    timed("tags", || session.tags(id).unwrap());
    timed("branch_stacks", || session.branch_stacks(id).unwrap());
    timed("proposals_list", || session.proposals_list(id).unwrap());
    timed("lfs_info", || session.lfs_info(id).unwrap());
    timed("unstage (whole)", || session.unstage(id, &paths).unwrap());

    println!("-- raw git, same {} paths --", paths.len());
    let input = paths.join("\0");
    let run = |label: &str, args: &[&str]| {
        timed(label, || {
            let mut child = std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            assert!(child.wait().unwrap().success());
        });
    };
    run(
        "git add -A --literal-pathspecs --pathspec-from-file",
        &[
            "--literal-pathspecs",
            "add",
            "-A",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
    );
    repo.git(&["reset", "-q"]);
    run(
        "git update-index --add --remove -z --stdin",
        &["update-index", "--add", "--remove", "-z", "--stdin"],
    );
    println!(
        "{}",
        repo.git(&["status", "--porcelain"])
            .lines()
            .filter(|l| !l.starts_with("M ") && !l.starts_with("A "))
            .count()
    );
}
