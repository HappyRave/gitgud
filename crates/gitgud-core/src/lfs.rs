use std::collections::{BTreeSet, HashMap, HashSet};
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

/// Local Git LFS object cache statistics and dry-run prune preview.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsCacheStats {
    pub object_count: u64,
    pub total_bytes: u64,
    pub prunable_count: u64,
    pub prunable_bytes: u64,
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

/// Parses a Git LFS pointer file into `(sha256_hex, size_bytes)`.
#[must_use]
pub fn parse_lfs_pointer(bytes: &[u8]) -> Option<(String, u64)> {
    let text = std::str::from_utf8(bytes).ok()?;
    if !text.starts_with("version https://git-lfs.github.com/spec/v1\n")
        && !text.starts_with("version https://git-lfs.github.com/spec/v1\r\n")
    {
        return None;
    }
    let mut oid = None;
    let mut size = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("oid sha256:") {
            let hex = rest.trim();
            if is_sha256_hex(hex) {
                oid = Some(hex.to_ascii_lowercase());
            }
        } else if let Some(rest) = line.strip_prefix("size ") {
            size = rest.trim().parse::<u64>().ok();
        }
    }
    Some((oid?, size?))
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

/// Measures `.git/lfs/objects` size and previews prunable objects via `git lfs prune --dry-run --verbose`.
///
/// # Errors
/// [`Error::Read`] or [`Error::Command`] if inspecting the LFS cache fails.
pub fn lfs_cache_stats(git: &GitCli, repo: &Repo) -> Result<LfsCacheStats, Error> {
    let mut oid_sizes = HashMap::new();
    collect_lfs_objects(&repo.git_dir().join("lfs/objects"), &mut oid_sizes);
    let object_count = oid_sizes.len() as u64;
    let total_bytes: u64 = oid_sizes.values().copied().sum();
    if object_count == 0 {
        return Ok(LfsCacheStats::default());
    }

    let (prunable_count, prunable_bytes) = match git.run_status(
        repo.root(),
        &["lfs", "prune", "--dry-run", "--verbose"],
        None,
    ) {
        Ok((0, stdout, _)) => parse_lfs_prune_dry_run(&stdout, &oid_sizes),
        _ => (0, 0),
    };
    Ok(LfsCacheStats {
        object_count,
        total_bytes,
        prunable_count,
        prunable_bytes,
    })
}

/// Prunes unreferenced local Git LFS cache objects via `git lfs prune` and returns updated stats.
///
/// # Errors
/// [`Error::Command`] when `git lfs prune` fails.
pub fn lfs_prune(git: &GitCli, repo: &Repo) -> Result<LfsCacheStats, Error> {
    git.run(repo.root(), &["lfs", "prune"], None)?;
    lfs_cache_stats(git, repo)
}

fn collect_lfs_objects(dir: &Path, oid_sizes: &mut HashMap<String, u64>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect_lfs_objects(&path, oid_sizes);
        } else if meta.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && is_sha256_hex(name)
        {
            oid_sizes.insert(name.to_ascii_lowercase(), meta.len());
        }
    }
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Extracts prunable `(count, bytes)` from `git lfs prune --dry-run --verbose` output.
#[must_use]
pub fn parse_lfs_prune_dry_run(stdout: &str, oid_sizes: &HashMap<String, u64>) -> (u64, u64) {
    let mut matched_oids = HashSet::new();
    for token in stdout.split(|c: char| !c.is_ascii_hexdigit()) {
        if is_sha256_hex(token) {
            let key = token.to_ascii_lowercase();
            if oid_sizes.contains_key(&key) {
                matched_oids.insert(key);
            }
        }
    }
    if !matched_oids.is_empty() {
        let bytes: u64 = matched_oids
            .iter()
            .filter_map(|k| oid_sizes.get(k))
            .copied()
            .sum();
        return (matched_oids.len() as u64, bytes);
    }

    for line in stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.split("local object").next()
            && let Some(num) = rest.split_whitespace().last()
            && let Ok(n) = num.parse::<u64>()
        {
            return (n, 0);
        }
    }
    (0, 0)
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
    fn detects_and_parses_lfs_pointer_bytes() {
        let ptr = b"version https://git-lfs.github.com/spec/v1\noid sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789\nsize 4096\n";
        assert!(is_lfs_pointer(ptr));
        assert!(!is_lfs_pointer(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(
            parse_lfs_pointer(ptr),
            Some((
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_owned(),
                4096
            ))
        );
        assert_eq!(parse_lfs_pointer(b"\x89PNG\r\n\x1a\n"), None);
    }

    #[test]
    fn collects_lfs_cache_dir_and_parses_prune_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let oid_a = "a".repeat(64);
        let oid_b = "b".repeat(64);
        let subdir = dir.path().join("aa/aa");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join(&oid_a), vec![1u8; 1200]).unwrap();
        fs::write(subdir.join(&oid_b), vec![2u8; 800]).unwrap();

        let mut map = HashMap::new();
        collect_lfs_objects(dir.path(), &mut map);
        assert_eq!(map.len(), 2);
        assert_eq!(map[&oid_a], 1200);
        assert_eq!(map[&oid_b], 800);

        let dry_run = format!("prune: 1 local objects, 1 retained, done.\n* {oid_b} (800 B)\n");
        assert_eq!(parse_lfs_prune_dry_run(&dry_run, &map), (1, 800));
        assert_eq!(
            parse_lfs_prune_dry_run("prune: 3 local objects, 1 retained", &map),
            (3, 0)
        );
    }
}
