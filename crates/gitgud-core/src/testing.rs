use std::cell::Cell;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use crate::repo::{Change, Entry, EntryKind};

const EPOCH: i64 = 1_700_000_000;

/// A throwaway repository in a temp dir, driven by the real `git` CLI.
///
/// Local config pins identity, line endings and signing, so tests don't depend on
/// the machine's global or system git config. Every git call advances a fake
/// clock by a minute, so commit dates are distinct and orders are deterministic.
pub struct TestRepo {
    dir: TempDir,
    clock: Cell<i64>,
}

impl TestRepo {
    /// Creates an empty repository on branch `main`.
    ///
    /// # Panics
    /// When the temp dir or `git init` fails.
    pub fn init() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().expect("create temp dir"),
            clock: Cell::new(EPOCH),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.name", "gitgud tests"),
            ("user.email", "tests@gitgud.invalid"),
            ("core.autocrlf", "false"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
        ] {
            repo.git(&["config", key, value]);
        }
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Writes `contents` to `rel`, creating parent directories.
    ///
    /// # Panics
    /// On an I/O error.
    pub fn write(&self, rel: &str, contents: &str) -> &Self {
        let path = self.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, contents).expect("write file");
        self
    }

    /// Deletes the file at `rel`.
    ///
    /// # Panics
    /// On an I/O error.
    pub fn remove(&self, rel: &str) -> &Self {
        std::fs::remove_file(self.path().join(rel)).expect("remove file");
        self
    }

    /// Runs `git <args>` in the repository and returns stdout.
    ///
    /// # Panics
    /// When git can't start or exits non-zero; the message carries stderr.
    pub fn git(&self, args: &[&str]) -> String {
        let out = self.git_output(args);
        assert!(
            out.status.success(),
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("git stdout is UTF-8")
    }

    /// Runs `git <args>` and returns whether it exited zero, for commands that are
    /// expected to fail (a conflicting merge).
    ///
    /// # Panics
    /// When git can't start.
    pub fn git_ok(&self, args: &[&str]) -> bool {
        self.git_output(args).status.success()
    }

    fn git_output(&self, args: &[&str]) -> std::process::Output {
        let now = self.clock.get() + 60;
        self.clock.set(now);
        let date = format!("@{now} +0000");
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .expect("run git")
    }

    /// Commits reachable from branches, remote branches, tags and HEAD as
    /// `(id, parent ids)`, in `git log --topo-order` order: the oracle for the
    /// gix-based topology.
    ///
    /// # Panics
    /// When git fails.
    pub fn cli_topo(&self) -> Vec<(String, Vec<String>)> {
        self.git(&[
            "log",
            "--topo-order",
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
            "--format=%H %P",
        ])
        .lines()
        .map(|line| {
            let mut ids = line.split_whitespace().map(str::to_owned);
            let id = ids.next().expect("commit id");
            (id, ids.collect())
        })
        .collect()
    }

    /// Stages everything and commits it.
    ///
    /// # Panics
    /// When the commit fails.
    pub fn commit_all(&self, message: &str) -> &Self {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self
    }

    /// The status as `git status --porcelain=v2` reports it, mapped to gitgud's
    /// model and sorted by path: the oracle the gix-based status is checked against.
    ///
    /// # Panics
    /// When git fails or prints a record this parser doesn't know.
    pub fn cli_status(&self) -> Vec<Entry> {
        let out = self.git(&[
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--no-renames",
        ]);
        let mut entries: Vec<Entry> = out
            .split('\0')
            .filter(|r| !r.is_empty())
            .map(parse_porcelain_v2)
            .collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }
}

fn parse_porcelain_v2(record: &str) -> Entry {
    let (tag, rest) = record.split_at(1);
    let rest = &rest[1..];
    match tag {
        "?" => Entry {
            path: rest.to_owned(),
            kind: EntryKind::Untracked,
            index: None,
            worktree: None,
        },
        "1" => {
            let fields: Vec<&str> = rest.splitn(8, ' ').collect();
            let xy = fields[0].as_bytes();
            Entry {
                path: fields[7].to_owned(),
                kind: EntryKind::Tracked,
                index: change(xy[0]),
                worktree: change(xy[1]),
            }
        }
        "u" => {
            let fields: Vec<&str> = rest.splitn(10, ' ').collect();
            Entry {
                path: fields[9].to_owned(),
                kind: EntryKind::Conflicted,
                index: None,
                worktree: None,
            }
        }
        _ => panic!("unexpected porcelain v2 record: {record:?}"),
    }
}

fn change(code: u8) -> Option<Change> {
    match code {
        b'.' => None,
        b'A' => Some(Change::Added),
        b'M' => Some(Change::Modified),
        b'D' => Some(Change::Deleted),
        b'T' => Some(Change::TypeChange),
        other => panic!("unexpected porcelain v2 status code {:?}", other as char),
    }
}
