use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, command_error, failure_message};
use crate::job::ProcessTree;
use crate::repo::Error;
use crate::task::{Cancel, parse_progress};

const POLL: Duration = Duration::from_millis(25);
static LFS_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Progress parsed from `GIT_LFS_PROGRESS` (`<dir> <files_done>/<files_total> <bytes_done>/<bytes_total> <file>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsProgress {
    pub state: String,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current_file: String,
}

/// Progress emitted during [`clone_repo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    pub phase: String,
    pub percent: u8,
    pub done: u64,
    pub total: u64,
    pub lfs: Option<LfsProgress>,
}

/// Parses one line written by git-lfs to `GIT_LFS_PROGRESS`.
#[must_use]
pub fn parse_lfs_progress_line(line: &str) -> Option<LfsProgress> {
    let mut parts = line.trim().splitn(4, char::is_whitespace);
    let state = parts.next()?.trim();
    let files = parts.next()?.trim();
    let bytes = parts.next()?.trim();
    let current_file = parts.next().unwrap_or("").trim().to_owned();
    let (fd, ft) = files.split_once('/')?;
    let (bd, bt) = bytes.split_once('/')?;
    Some(LfsProgress {
        state: state.to_owned(),
        files_done: fd.parse().ok()?,
        files_total: ft.parse().ok()?,
        bytes_done: bd.parse().ok()?,
        bytes_total: bt.parse().ok()?,
        current_file,
    })
}

/// Clones `url` into `dest` with progress (including `GIT_LFS_PROGRESS`) and process-tree
/// cancellation. Removes partial `dest` if this clone created it and was cancelled or failed.
/// With `skip_lfs`, LFS files are left as pointers.
///
/// # Errors
/// [`Error::Refused`] for invalid arguments or non-empty `dest`, [`Error::Cancelled`] on cancel,
/// [`Error::Command`] when `git clone` fails.
pub fn clone_repo(
    git: &GitCli,
    url: &str,
    dest: &Path,
    long_paths: bool,
    skip_lfs: bool,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(CloneProgress),
) -> Result<PathBuf, Error> {
    let url = url.trim();
    if url.is_empty() {
        return Err(Error::Refused("repository URL can't be empty".into()));
    }
    if dest.as_os_str().is_empty() {
        return Err(Error::Refused("destination folder can't be empty".into()));
    }
    let existed = dest.exists();
    if existed {
        let non_empty = std::fs::read_dir(dest)
            .map(|mut d| d.next().is_some())
            .unwrap_or(true);
        if non_empty {
            return Err(Error::Refused(format!(
                "destination folder already exists and is not empty: {}",
                dest.display()
            )));
        }
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| Error::Read(format!("cannot create {}: {e}", parent.display())))?;

    let lfs_file = TempProgressFile::create()?;
    let dest_str = dest.to_string_lossy().into_owned();
    let mut args = vec!["clone", "--progress"];
    if long_paths {
        args.extend(["--config", "core.longpaths=true"]);
    }
    args.extend([url, &dest_str]);

    let mut cmd = git.command(parent, &args);
    cmd.env("GIT_LFS_PROGRESS", &lfs_file.path);
    if skip_lfs {
        // Pointers only: a partial download (R10) chooses what comes next.
        cmd.env("GIT_LFS_SKIP_SMUDGE", "1");
    }

    let mut tree = ProcessTree::spawn(&mut cmd).map_err(|e| command_error(&args, e.to_string()))?;
    let mut stdout = tree.child.stdout.take().expect("stdout is piped");
    let mut stderr = tree.child.stderr.take().expect("stderr is piped");

    let (tx, rx) = mpsc::channel::<String>();
    let err_reader = std::thread::spawn(move || {
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

    let mut lfs_tail = LfsTail::open(&lfs_file.path);
    let mut latest_lfs: Option<LfsProgress> = None;
    let mut messages = Vec::new();

    loop {
        if let Some(lfs) = lfs_tail.poll() {
            let pct = percent_of(lfs.bytes_done, lfs.bytes_total);
            latest_lfs = Some(lfs.clone());
            on_progress(CloneProgress {
                phase: format!("LFS {}", lfs.state),
                percent: pct,
                done: lfs.bytes_done,
                total: lfs.bytes_total,
                lfs: Some(lfs),
            });
        }
        if cancel.is_cancelled() {
            tree.kill();
            let _ = tree.child.wait();
            if !existed {
                let _ = std::fs::remove_dir_all(dest);
            }
            return Err(Error::Cancelled);
        }
        match rx.recv_timeout(POLL) {
            Ok(line) => match parse_progress(&line) {
                Some(p) => on_progress(CloneProgress {
                    phase: p.phase,
                    percent: p.percent,
                    done: p.done,
                    total: p.total,
                    lfs: latest_lfs.clone(),
                }),
                None => messages.push(line),
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let status = tree
        .child
        .wait()
        .map_err(|e| command_error(&args, e.to_string()))?;
    let _ = err_reader.join();
    let stdout = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).into_owned();
    if status.success() {
        return Ok(dest.to_path_buf());
    }
    if !existed {
        let _ = std::fs::remove_dir_all(dest);
    }
    Err(command_error(
        &args,
        failure_message(&stdout, &messages.join("\n")),
    ))
}

struct TempProgressFile {
    path: PathBuf,
}

impl TempProgressFile {
    fn create() -> Result<Self, Error> {
        let id = LFS_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gitgud-lfs-progress-{}-{id}.log",
            std::process::id()
        ));
        File::create(&path)
            .map_err(|e| Error::Read(format!("cannot create progress file: {e}")))?;
        Ok(Self { path })
    }
}

impl Drop for TempProgressFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct LfsTail {
    reader: Option<BufReader<File>>,
}

impl LfsTail {
    fn open(path: &Path) -> Self {
        Self {
            reader: File::open(path).ok().map(BufReader::new),
        }
    }

    fn poll(&mut self) -> Option<LfsProgress> {
        let r = self.reader.as_mut()?;
        let mut latest = None;
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Some(p) = parse_lfs_progress_line(&line) {
                        latest = Some(p);
                    }
                }
            }
        }
        latest
    }
}

fn percent_of(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    let pct = done.saturating_mul(100) / total;
    u8::try_from(pct.min(100)).unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_lfs_progress_line() {
        let parsed =
            parse_lfs_progress_line("download 2/5 1048576/5242880 Assets/Textures/skybox.tga")
                .expect("valid lfs line");
        assert_eq!(parsed.state, "download");
        assert_eq!(parsed.files_done, 2);
        assert_eq!(parsed.files_total, 5);
        assert_eq!(parsed.bytes_done, 1_048_576);
        assert_eq!(parsed.bytes_total, 5_242_880);
        assert_eq!(parsed.current_file, "Assets/Textures/skybox.tga");
    }

    #[test]
    fn ignores_malformed_lfs_progress_line() {
        assert!(parse_lfs_progress_line("something else").is_none());
        assert!(parse_lfs_progress_line("download bad 1/2 a").is_none());
    }
}
