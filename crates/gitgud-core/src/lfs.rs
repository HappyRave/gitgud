use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::remote_url::HostKind;
use crate::repo::{Error, Repo};

const LFS_HEADER: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";

/// Git LFS status, tracked patterns, and host lock support for a repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub tracked_patterns: Vec<String>,
    pub supports_locks: bool,
    pub locks_unsupported_reason: Option<String>,
}

/// An active Git LFS file lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
    pub locked_at: String,
    pub ours: bool,
}

#[derive(Deserialize)]
struct RawLockOwner {
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawLock {
    id: Option<String>,
    path: String,
    owner: Option<RawLockOwner>,
    locked_at: Option<String>,
}

/// Returns true when `bytes` is a Git LFS pointer file.
#[must_use]
pub fn is_lfs_pointer(bytes: &[u8]) -> bool {
    bytes.starts_with(LFS_HEADER) && bytes.windows(11).any(|w| w == b"oid sha256:")
}

/// Collects LFS installation, tracked filter patterns (non-smudging), and remote lock support.
pub fn lfs_info(git: &GitCli, repo: &Repo) -> Result<LfsInfo, Error> {
    let (code, out, _) = git.run_status(repo.root(), &["lfs", "version"], None)?;
    let installed = code == 0;
    let version = installed
        .then(|| out.trim().to_owned())
        .filter(|s| !s.is_empty());

    let mut tracked_patterns = BTreeSet::new();
    for file in [
        repo.root().join(".gitattributes"),
        repo.git_dir().join("info/attributes"),
    ] {
        collect_lfs_patterns(&file, &mut tracked_patterns);
    }

    let (supports_locks, locks_unsupported_reason) = match repo.primary_host() {
        HostKind::Bitbucket => (
            false,
            Some("Bitbucket Cloud does not support Git LFS file locking".into()),
        ),
        HostKind::None => (
            false,
            Some("Add a GitHub or self-hosted LFS remote to use file locks".into()),
        ),
        HostKind::GitHub | HostKind::Other => (installed, None),
    };

    Ok(LfsInfo {
        installed,
        version,
        tracked_patterns: tracked_patterns.into_iter().collect(),
        supports_locks,
        locks_unsupported_reason,
    })
}

/// Lists active LFS locks from the remote via `git lfs locks --json`.
/// Returns an empty list immediately on Bitbucket Cloud or when no remote is configured.
///
/// # Errors
/// [`Error::Command`] when `git lfs locks` fails unexpectedly.
pub fn lfs_locks_list(git: &GitCli, repo: &Repo) -> Result<Vec<LfsLock>, Error> {
    if repo.primary_host() == HostKind::Bitbucket || repo.primary_remote_url().is_none() {
        return Ok(Vec::new());
    }
    let (code, stdout, stderr) = git.run_status(repo.root(), &["lfs", "locks", "--json"], None)?;
    if code != 0 {
        let msg = crate::cli::failure_message(&stdout, &stderr);
        if msg.contains("is not a git command") || msg.contains("Invalid remote name") {
            return Ok(Vec::new());
        }
        return Err(crate::cli::command_error(&["lfs", "locks", "--json"], msg));
    }

    let me = read_user_name(git, repo.root());
    parse_locks_json(&stdout, me.as_deref())
}

/// Parses `git lfs locks --json` output into [`LfsLock`] values.
///
/// # Errors
/// [`Error::Read`] if JSON is invalid.
pub fn parse_locks_json(json_str: &str, current_user: Option<&str>) -> Result<Vec<LfsLock>, Error> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    let raw: Vec<RawLock> =
        serde_json::from_str(trimmed).map_err(|e| Error::Read(format!("LFS locks JSON: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| {
            let owner = r
                .owner
                .and_then(|o| o.name)
                .unwrap_or_else(|| "unknown".into());
            let ours =
                current_user.is_some_and(|me| !me.is_empty() && me.eq_ignore_ascii_case(&owner));
            LfsLock {
                id: r.id.unwrap_or_default(),
                path: r.path.replace('\\', "/"),
                owner,
                locked_at: r.locked_at.unwrap_or_default(),
                ours,
            }
        })
        .collect())
}

/// Locks `path` via `git lfs lock`.
///
/// # Errors
/// [`Error::Refused`] on Bitbucket Cloud or [`Error::Command`] on failure.
pub fn lfs_lock(git: &GitCli, repo: &Repo, path: &str) -> Result<(), Error> {
    if repo.primary_host() == HostKind::Bitbucket {
        return Err(Error::Refused(
            "Bitbucket Cloud does not support Git LFS file locking".into(),
        ));
    }
    git.run(repo.root(), &["lfs", "lock", path], None).map(drop)
}

/// Unlocks `path` via `git lfs unlock [--force]`.
///
/// # Errors
/// [`Error::Refused`] on Bitbucket Cloud or [`Error::Command`] on failure.
pub fn lfs_unlock(git: &GitCli, repo: &Repo, path: &str, force: bool) -> Result<(), Error> {
    if repo.primary_host() == HostKind::Bitbucket {
        return Err(Error::Refused(
            "Bitbucket Cloud does not support Git LFS file locking".into(),
        ));
    }
    let args: &[&str] = if force {
        &["lfs", "unlock", "--force", path]
    } else {
        &["lfs", "unlock", path]
    };
    git.run(repo.root(), args, None).map(drop)
}

/// Returns locks held by `ours == true` whose paths were touched by unpushed commits on `branch` -> `remote/remote_branch`.
///
/// # Errors
/// [`Error::Command`] if listing locks or diffing commits fails.
pub fn locks_in_push(
    git: &GitCli,
    repo: &Repo,
    remote: &str,
    remote_branch: &str,
    branch: &str,
) -> Result<Vec<LfsLock>, Error> {
    if repo.primary_host() == HostKind::Bitbucket {
        return Ok(Vec::new());
    }
    let locks = lfs_locks_list(git, repo)?;
    let ours: Vec<LfsLock> = locks.into_iter().filter(|l| l.ours).collect();
    if ours.is_empty() {
        return Ok(Vec::new());
    }

    let remote_ref = format!("refs/remotes/{remote}/{remote_branch}");
    let has_remote_ref =
        git.run_status(
            repo.root(),
            &["rev-parse", "--verify", "--quiet", &remote_ref],
            None,
        )?
        .0 == 0;
    if !has_remote_ref {
        // First push of branch: return all locks currently held by us.
        return Ok(ours);
    }

    let range = format!("{remote_ref}..{branch}");
    let changed = git.run(
        repo.root(),
        &["diff", "--name-only", "--no-renames", "-z", &range],
        None,
    )?;
    let touched: BTreeSet<&str> = changed.split('\0').filter(|p| !p.is_empty()).collect();
    Ok(ours
        .into_iter()
        .filter(|l| touched.contains(l.path.as_str()))
        .collect())
}

fn collect_lfs_patterns(attr_file: &Path, out: &mut BTreeSet<String>) {
    let Ok(content) = fs::read_to_string(attr_file) else {
        return;
    };
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(pattern) = parts.next() else {
            continue;
        };
        if parts.any(|tok| tok == "filter=lfs") {
            out.insert(pattern.to_owned());
        }
    }
}

fn read_user_name(git: &GitCli, root: &Path) -> Option<String> {
    let (code, stdout, _) = git
        .run_status(root, &["config", "--get", "user.name"], None)
        .ok()?;
    (code == 0)
        .then(|| stdout.trim().to_owned())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lfs_locks_json_and_identifies_owner() {
        let raw = r#"[
          {"id":"11","path":"Assets\\Art\\Hero.png","owner":{"name":"Alice"},"locked_at":"2026-01-01T00:00:00Z"},
          {"id":"12","path":"Assets/Audio/Sfx.wav","owner":{"name":"Bob"},"locked_at":"2026-01-02T00:00:00Z"}
        ]"#;
        let locks = parse_locks_json(raw, Some("alice")).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].path, "Assets/Art/Hero.png");
        assert!(locks[0].ours);
        assert!(!locks[1].ours);
    }

    #[test]
    fn detects_lfs_pointer_bytes() {
        assert!(is_lfs_pointer(
            b"version https://git-lfs.github.com/spec/v1\noid sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\nsize 4096\n"
        ));
        assert!(!is_lfs_pointer(b"\x89PNG\r\n\x1a\n"));
    }
}
