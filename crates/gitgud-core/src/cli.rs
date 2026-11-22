use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::git::GitInfo;
use crate::repo::Error;

/// Runs the `git` CLI for writes, so hooks, credential helpers and user config
/// behave as in a terminal.
#[derive(Debug, Clone)]
pub struct GitCli {
    path: PathBuf,
}

impl GitCli {
    pub fn new(info: &GitInfo) -> Self {
        Self {
            path: info.path.clone(),
        }
    }

    /// Runs `git <args>` in `cwd`, feeding `stdin`, and returns stdout.
    ///
    /// # Errors
    /// [`Error::Command`] when git can't start or exits non-zero; the message is
    /// git's stderr, or stdout when stderr is empty (`git commit` reports
    /// "nothing to commit" there).
    pub fn run(&self, cwd: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<String, Error> {
        self.run_env(cwd, args, stdin, &[])
    }

    /// As [`GitCli::run`], with extra environment variables (`GIT_INDEX_FILE`...).
    ///
    /// # Errors
    /// As [`GitCli::run`].
    pub fn run_env(
        &self,
        cwd: &Path,
        args: &[&str],
        stdin: Option<&[u8]>,
        env: &[(&str, &Path)],
    ) -> Result<String, Error> {
        let fail = |message: String| command_error(args, message);
        let mut cmd = self.command(cwd, args);
        cmd.envs(env.iter().map(|(k, v)| (k, v.as_os_str())));
        cmd.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = cmd.spawn().map_err(|e| fail(e.to_string()))?;

        let output = std::thread::scope(|s| {
            if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
                // A separate thread, so a git that writes before it has read all
                // of stdin can't deadlock against us.
                s.spawn(move || pipe.write_all(bytes));
            }
            child.wait_with_output()
        })
        .map_err(|e| fail(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() {
            return Ok(stdout);
        }
        Err(fail(failure_message(
            &stdout,
            &String::from_utf8_lossy(&output.stderr),
        )))
    }

    /// Runs `git <args>` and returns its exit code with stdout and stderr, for
    /// commands whose non-zero exit is an answer, not a failure (`merge-file`
    /// counts conflicts, `merge-tree` exits 1 when there are some).
    ///
    /// # Errors
    /// [`Error::Command`] only when git can't start or is killed by a signal.
    pub fn run_status(
        &self,
        cwd: &Path,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<(i32, String, String), Error> {
        let fail = |message: String| command_error(args, message);
        let mut cmd = self.command(cwd, args);
        cmd.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = cmd.spawn().map_err(|e| fail(e.to_string()))?;
        let output = std::thread::scope(|s| {
            if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
                s.spawn(move || pipe.write_all(bytes));
            }
            child.wait_with_output()
        })
        .map_err(|e| fail(e.to_string()))?;
        let code = output
            .status
            .code()
            .ok_or_else(|| fail("git was killed".into()))?;
        Ok((
            code,
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }

    /// `git <args>` in `cwd` with piped output, no console window and no prompts.
    pub(crate) fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.path);
        cmd.args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_console_window(&mut cmd);
        cmd
    }
}

pub(crate) fn command_error(args: &[&str], message: String) -> Error {
    Error::Command {
        command: format!("git {}", args.join(" ")),
        message,
    }
}

/// git's stderr, or its stdout when stderr is empty.
pub(crate) fn failure_message(stdout: &str, stderr: &str) -> String {
    if stderr.trim().is_empty() {
        stdout.trim().to_owned()
    } else {
        stderr.trim().to_owned()
    }
}

#[cfg(windows)]
pub(crate) fn no_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn no_console_window(_: &mut Command) {}
