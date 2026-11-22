use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, SystemTime};

use serde::Serialize;

use crate::cli::{GitCli, command_error, failure_message};
use crate::job::ProcessTree;
use crate::repo::{Error, Repo, read_err};

/// How often a running task checks whether it was cancelled.
const POLL: Duration = Duration::from_millis(25);

/// Shared flag that stops a running task; cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// One progress line from git, e.g. `Receiving objects:  45% (450/1000)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub phase: String,
    pub percent: u8,
    pub done: u64,
    pub total: u64,
}

/// Parses git's `--progress` lines, with or without the `remote: ` prefix.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let line = line.trim().strip_prefix("remote:").unwrap_or(line).trim();
    let (phase, rest) = line.split_once(':')?;
    let rest = rest.trim_start();
    let (percent, rest) = rest.split_once('%')?;
    let percent: u8 = percent.trim().parse().ok().filter(|p| *p <= 100)?;
    let counts = rest.trim_start().strip_prefix('(')?;
    let (counts, _) = counts.split_once(')')?;
    let (done, total) = counts.split_once('/')?;
    Some(Progress {
        phase: phase.trim().to_owned(),
        percent,
        done: done.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
    })
}

impl GitCli {
    /// Runs `git <args>` with `--progress` output parsed into `on_progress`, and
    /// kills git and every process it started when `cancel` is set.
    ///
    /// # Errors
    /// [`Error::Cancelled`] after a cancel, [`Error::Command`] when git fails.
    pub fn run_with_progress(
        &self,
        cwd: &Path,
        args: &[&str],
        on_progress: &mut dyn FnMut(Progress),
        cancel: &Cancel,
    ) -> Result<String, Error> {
        let mut tree = ProcessTree::spawn(&mut self.command(cwd, args))
            .map_err(|e| command_error(args, e.to_string()))?;
        let mut stdout = tree.child.stdout.take().expect("stdout is piped");
        let mut stderr = tree.child.stderr.take().expect("stderr is piped");

        let (tx, rx) = mpsc::channel::<String>();
        let err_reader = std::thread::spawn(move || {
            // Progress rewrites one line with \r; split on both line ends.
            let mut buf = [0u8; 4096];
            let mut line = Vec::new();
            loop {
                match stderr.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        for &b in &buf[..n] {
                            if b == b'\r' || b == b'\n' {
                                if !line.is_empty() {
                                    let _ = tx.send(String::from_utf8_lossy(&line).into_owned());
                                    line.clear();
                                }
                            } else {
                                line.push(b);
                            }
                        }
                    }
                }
            }
            if !line.is_empty() {
                let _ = tx.send(String::from_utf8_lossy(&line).into_owned());
            }
        });
        let out_reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = stdout.read_to_end(&mut out);
            out
        });

        let mut messages = Vec::new();
        loop {
            if cancel.is_cancelled() {
                tree.kill();
                let _ = tree.child.wait();
                return Err(Error::Cancelled);
            }
            match rx.recv_timeout(POLL) {
                Ok(line) => match parse_progress(&line) {
                    Some(p) => on_progress(p),
                    None => messages.push(line),
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        let status = tree
            .child
            .wait()
            .map_err(|e| command_error(args, e.to_string()))?;
        let _ = err_reader.join();
        let stdout = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).into_owned();
        if status.success() {
            return Ok(stdout);
        }
        Err(command_error(
            args,
            failure_message(&stdout, &messages.join("\n")),
        ))
    }
}

impl Repo {
    /// `git fetch --all`, with progress and cancel. A cancelled fetch's partial
    /// pack files are removed.
    ///
    /// # Errors
    /// [`Error::Cancelled`], or [`Error::Command`] with git's message (no
    /// network, credentials refused...).
    pub fn fetch(
        &self,
        git: &GitCli,
        on_progress: &mut dyn FnMut(Progress),
        cancel: &Cancel,
    ) -> Result<(), Error> {
        self.fetch_remote(git, None, on_progress, cancel)
    }

    /// Fetches `remote` (or `--all` when `None`), with progress and cancel.
    ///
    /// # Errors
    /// [`Error::Cancelled`], or [`Error::Command`] with git's message.
    pub fn fetch_remote(
        &self,
        git: &GitCli,
        remote: Option<&str>,
        on_progress: &mut dyn FnMut(Progress),
        cancel: &Cancel,
    ) -> Result<(), Error> {
        let started = SystemTime::now();
        let mut args = vec!["fetch"];
        if let Some(r) = remote {
            args.push(r);
        } else {
            args.push("--all");
        }
        args.push("--progress");
        let result = git.run_with_progress(self.root(), &args, on_progress, cancel);
        if matches!(result, Err(Error::Cancelled)) {
            self.remove_partial_packs(started)?;
        }
        result.map(drop)
    }

    /// Deletes `tmp_pack_*` files a killed fetch left, if they are from this run.
    fn remove_partial_packs(&self, since: SystemTime) -> Result<(), Error> {
        let packs = self.gix().objects.store_ref().path().join("pack");
        let Ok(dir) = std::fs::read_dir(&packs) else {
            return Ok(());
        };
        for entry in dir {
            let entry = entry.map_err(read_err)?;
            let name = entry.file_name();
            let fresh = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t >= since);
            if name.to_string_lossy().starts_with("tmp_pack_") && fresh {
                std::fs::remove_file(entry.path()).map_err(read_err)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_and_remote_progress() {
        assert_eq!(
            parse_progress("Receiving objects:  16% (579112/3619449), 295.00 MiB | 30.80 MiB/s"),
            Some(Progress {
                phase: "Receiving objects".into(),
                percent: 16,
                done: 579_112,
                total: 3_619_449,
            })
        );
        assert_eq!(
            parse_progress("remote: Counting objects: 100% (20/20), done."),
            Some(Progress {
                phase: "Counting objects".into(),
                percent: 100,
                done: 20,
                total: 20,
            })
        );
    }

    #[test]
    fn ignores_lines_that_are_not_progress() {
        for line in [
            "",
            "Fetching origin",
            "From https://example.invalid/repo",
            " * [new branch]      main       -> origin/main",
            "fatal: unable to access: 50% sure (nope)",
            "Receiving objects: 150% (1/2)",
            "remote: Enumerating objects: 5, done.",
        ] {
            assert_eq!(parse_progress(line), None, "{line:?}");
        }
    }
}
