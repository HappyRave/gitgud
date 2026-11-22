use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gix::ObjectId;
use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::diff::{FileDiff, Hunk, LineKind, is_binary};
use crate::lfs;
use crate::repo::{self, EntryKind, Error, Head, Repo, Status, read_err};
use crate::unity::{MetaIssueKind, check_meta_issues, expand_meta_pairs, find_unity_root};
use crate::worktree::{HunkHeader, Side};

const PROPOSAL_REF_PREFIX: &str = "refs/gitgud/proposals/";

/// Current review state of an agent proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Pending,
    ChangesRequested,
    Approved,
    Rejected,
    Withdrawn,
}

/// Review note attached to a proposal (typically when changes are requested).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalComment {
    pub id: String,
    pub author: String,
    pub text: String,
    pub path: Option<String>,
    pub time: i64,
}

/// Automatic safety or Unity-specific risk flag computed by `gitgud-core` (never self-reported by agents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskFlag {
    pub kind: String,
    pub path: String,
    pub detail: String,
}

/// Summary and full review metadata for one stored agent proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSummary {
    pub id: String,
    pub commit_id: String,
    pub short_commit: String,
    pub base_id: String,
    pub message: String,
    pub why: String,
    pub agent: String,
    pub status: ProposalStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub files: Vec<String>,
    pub risk_flags: Vec<RiskFlag>,
    pub comments: Vec<ProposalComment>,
    #[serde(default)]
    pub head_moved: bool,
}

/// Stable addressable hunk returned by `agent_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHunk {
    pub id: String,
    pub side: Side,
    pub header: HunkHeader,
    pub preview: String,
    pub added: usize,
    pub removed: usize,
}

/// File entry returned by `agent_status` with stable hunk IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileStatus {
    pub path: String,
    pub kind: EntryKind,
    pub binary: bool,
    pub hunks: Vec<AgentHunk>,
}

/// Agent-facing repository status snapshot (includes untracked file hunks without `git add -N`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub root: String,
    pub head: Head,
    pub files: Vec<AgentFileStatus>,
    pub risk_flags: Vec<RiskFlag>,
}

/// Input parameters for creating a hidden-ref proposal.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalInput {
    pub message: String,
    pub why: Option<String>,
    pub agent: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub hunk_ids: Vec<String>,
}

struct TempIndexGuard(PathBuf);

impl Drop for TempIndexGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let lock = PathBuf::from(format!("{}.lock", self.0.display()));
        let _ = fs::remove_file(lock);
    }
}

/// Deterministic 12-hex-char hunk identifier derived from `path` and changed lines.
#[must_use]
pub fn stable_hunk_id(path: &str, hunk: &Hunk) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(path.as_bytes());
    feed(b"\0");
    feed(&hunk.old_start.to_le_bytes());
    feed(&hunk.new_start.to_le_bytes());
    for line in &hunk.lines {
        let stripped = line.text.trim_end_matches(['\r', '\n']);
        match line.kind {
            LineKind::Added => {
                feed(b"+");
                feed(stripped.as_bytes());
            }
            LineKind::Removed => {
                feed(b"-");
                feed(stripped.as_bytes());
            }
            LineKind::Context => {}
        }
    }
    format!("{hash:012x}").chars().take(12).collect()
}

/// Inspects working copy & staged changes with stable hunk IDs and automatic Unity/LFS risk flags.
///
/// # Errors
/// [`Error::Read`] or git status error.
pub fn agent_status(repo: &Repo, git: &GitCli) -> Result<AgentStatus, Error> {
    let status = repo.status()?;
    let mut files = Vec::new();
    for entry in &status.entries {
        let mut hunks = Vec::new();
        let mut binary = false;
        for side in [Side::Staged, Side::Unstaged] {
            let has_side = match side {
                Side::Staged => entry.index.is_some(),
                Side::Unstaged => entry.worktree.is_some(),
            };
            if !has_side || entry.kind == EntryKind::Conflicted {
                continue;
            }
            if let Ok(diff) = repo.worktree_file_diff(&entry.path, side) {
                binary |= diff.binary;
                for h in &diff.hunks {
                    hunks.push(to_agent_hunk(&entry.path, side, h));
                }
            }
        }
        files.push(AgentFileStatus {
            path: entry.path.clone(),
            kind: entry.kind,
            binary,
            hunks,
        });
    }
    let risk_flags = compute_risk_flags(repo, git, &status, None);
    Ok(AgentStatus {
        root: status.root.to_string_lossy().into_owned(),
        head: status.head,
        files,
        risk_flags,
    })
}

/// Creates a commit under `refs/gitgud/proposals/<id>` using an isolated temporary index so the
/// user's real `.git/index`, working copy, and branches remain completely untouched.
///
/// # Errors
/// [`Error::Refused`] for empty message or empty changeset, or [`Error::Command`] on git failure.
pub fn create_proposal(
    repo: &Repo,
    git: &GitCli,
    input: &CreateProposalInput,
) -> Result<ProposalSummary, Error> {
    let message = input.message.trim();
    if message.is_empty() {
        return Err(Error::Refused(
            "Proposal commit message cannot be empty".into(),
        ));
    }

    let status = repo.status()?;
    let now = now_secs();
    let id = unique_proposal_id(repo, now);
    let root = repo.root();
    let prop_dir = proposals_dir(repo);
    fs::create_dir_all(&prop_dir).map_err(read_err)?;

    let tmp_idx = prop_dir.join(format!("tmp-{id}.index"));
    let _guard = TempIndexGuard(tmp_idx.clone());
    let _ = fs::remove_file(&tmp_idx);

    let gix_repo = repo.gix();
    let head_oid = gix_repo.head_id().ok().map(gix::Id::detach);
    let base_id = head_oid
        .map(|oid| oid.to_string())
        .unwrap_or_else(|| "unborn".into());

    let idx_env = [("GIT_INDEX_FILE", tmp_idx.as_path())];
    if let Some(oid) = head_oid {
        let hex = oid.to_string();
        git.run_env(root, &["read-tree", &hex], None, &idx_env)?;
    }

    let selected_paths: Vec<String>;
    if !input.hunk_ids.is_empty() {
        let wanted: HashSet<&str> = input.hunk_ids.iter().map(String::as_str).collect();
        let mut patch_bundle = String::new();
        let mut matched_paths = BTreeSet::new();
        for entry in &status.entries {
            if !input.paths.is_empty() && !input.paths.iter().any(|p| p == &entry.path) {
                continue;
            }
            for side in [Side::Staged, Side::Unstaged] {
                let has_side = match side {
                    Side::Staged => entry.index.is_some(),
                    Side::Unstaged => entry.worktree.is_some(),
                };
                if !has_side || entry.kind == EntryKind::Conflicted {
                    continue;
                }
                if let Ok(hunks) = repo.side_hunks_with_patches(&entry.path, side) {
                    for (hunk, raw_patch) in hunks {
                        let hid = stable_hunk_id(&entry.path, &hunk);
                        if wanted.contains(hid.as_str()) {
                            patch_bundle.push_str(&raw_patch);
                            matched_paths.insert(entry.path.clone());
                        }
                    }
                }
            }
        }
        if patch_bundle.is_empty() {
            return Err(Error::Refused(
                "None of the requested hunk IDs matched current changes".into(),
            ));
        }
        git.run_env(
            root,
            &["apply", "--cached", "--whitespace=nowarn", "-"],
            Some(patch_bundle.as_bytes()),
            &idx_env,
        )?;
        selected_paths = matched_paths.into_iter().collect();
    } else {
        let raw_paths: Vec<String> = if input.paths.is_empty() {
            status.entries.iter().map(|e| e.path.clone()).collect()
        } else {
            input.paths.clone()
        };
        if raw_paths.is_empty() {
            return Err(Error::Refused(
                "Working copy has no changes to propose".into(),
            ));
        }
        let is_unity = find_unity_root(root).is_some();
        let (expanded, _) = if is_unity {
            expand_meta_pairs(&status, &raw_paths)
        } else {
            (raw_paths, Vec::new())
        };
        selected_paths = expanded;
        let pathspec = nul_pathspec(&selected_paths);
        git.run_env(
            root,
            &[
                "--literal-pathspecs",
                "add",
                "-A",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(&pathspec),
            &idx_env,
        )?;
    }

    let tree_oid = git
        .run_env(root, &["write-tree"], None, &idx_env)?
        .trim()
        .to_owned();
    if let Some(oid) = head_oid
        && let Ok(head_commit) = gix_repo.find_commit(oid)
        && let Ok(tid) = ObjectId::from_hex(tree_oid.as_bytes())
        && head_commit.tree_id().is_ok_and(|t| t == tid)
    {
        return Err(Error::Refused(
            "Proposal has no changes compared to HEAD".into(),
        ));
    }

    let mut commit_args = vec!["commit-tree", tree_oid.as_str()];
    if head_oid.is_some() {
        commit_args.extend(["-p", base_id.as_str()]);
    }
    let commit_oid = git
        .run(root, &commit_args, Some(message.as_bytes()))?
        .trim()
        .to_owned();

    let ref_name = format!("{PROPOSAL_REF_PREFIX}{id}");
    git.run(root, &["update-ref", &ref_name, &commit_oid], None)?;

    let filter: BTreeSet<String> = selected_paths.iter().cloned().collect();
    let risk_flags = compute_risk_flags(repo, git, &status, Some(&filter));

    let summary = ProposalSummary {
        id: id.clone(),
        short_commit: commit_oid.chars().take(7).collect(),
        commit_id: commit_oid,
        base_id,
        message: message.to_owned(),
        why: input.why.as_deref().map(str::trim).unwrap_or("").to_owned(),
        agent: input
            .agent
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("agent")
            .to_owned(),
        status: ProposalStatus::Pending,
        created_at: now,
        updated_at: now,
        files: selected_paths,
        risk_flags,
        comments: Vec::new(),
        head_moved: false,
    };
    save_summary(repo, &summary)?;
    Ok(summary)
}

/// Lists all recorded proposals in `.git/gitgud/proposals/`, newest first.
///
/// # Errors
/// [`Error::Read`] if stored proposal metadata cannot be read.
pub fn list_proposals(repo: &Repo) -> Result<Vec<ProposalSummary>, Error> {
    let dir = proposals_dir(repo);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let cur_head = repo
        .gix()
        .head_id()
        .ok()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unborn".into());
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(read_err)? {
        let entry = entry.map_err(read_err)?;
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let body = fs::read_to_string(&p).map_err(read_err)?;
        let Ok(mut item) = serde_json::from_str::<ProposalSummary>(&body) else {
            continue;
        };
        item.head_moved = matches!(
            item.status,
            ProposalStatus::Pending | ProposalStatus::ChangesRequested
        ) && cur_head != item.base_id;
        out.push(item);
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(out)
}

/// Fetches a single proposal by ID.
///
/// # Errors
/// [`Error::Refused`] if not found.
pub fn get_proposal(repo: &Repo, proposal_id: &str) -> Result<ProposalSummary, Error> {
    let path = summary_path(repo, proposal_id);
    if !path.is_file() {
        return Err(Error::Refused(format!(
            "Proposal '{proposal_id}' not found"
        )));
    }
    let body = fs::read_to_string(&path).map_err(read_err)?;
    let mut item: ProposalSummary = serde_json::from_str(&body).map_err(read_err)?;
    let cur_head = repo
        .gix()
        .head_id()
        .ok()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unborn".into());
    item.head_moved = matches!(
        item.status,
        ProposalStatus::Pending | ProposalStatus::ChangesRequested
    ) && cur_head != item.base_id;
    Ok(item)
}

/// Reads the diff for `path` inside a stored proposal commit.
///
/// # Errors
/// [`Error::Read`] / [`Error::Refused`].
pub fn proposal_file_diff(repo: &Repo, proposal_id: &str, path: &str) -> Result<FileDiff, Error> {
    let prop = get_proposal(repo, proposal_id)?;
    repo.commit_file_diff(&prop.commit_id, path)
}

/// Appends review feedback to a proposal and sets status to `ChangesRequested`.
///
/// # Errors
/// [`Error::Refused`] if empty comment or finalized proposal.
pub fn request_changes(
    repo: &Repo,
    proposal_id: &str,
    comment: &str,
    path: Option<&str>,
) -> Result<ProposalSummary, Error> {
    let text = comment.trim();
    if text.is_empty() {
        return Err(Error::Refused("Feedback comment cannot be empty".into()));
    }
    let mut prop = get_proposal(repo, proposal_id)?;
    if !matches!(
        prop.status,
        ProposalStatus::Pending | ProposalStatus::ChangesRequested
    ) {
        return Err(Error::Refused(format!(
            "Proposal '{proposal_id}' is already finalized"
        )));
    }
    let now = now_secs();
    prop.comments.push(ProposalComment {
        id: format!("c-{now}-{}", prop.comments.len() + 1),
        author: "Max".into(),
        text: text.to_owned(),
        path: path
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        time: now,
    });
    prop.status = ProposalStatus::ChangesRequested;
    prop.updated_at = now;
    save_summary(repo, &prop)?;
    Ok(prop)
}

/// Rejects an active proposal and deletes its hidden ref.
///
/// # Errors
/// [`Error::Refused`] / [`Error::Command`].
pub fn reject_proposal(
    repo: &Repo,
    git: &GitCli,
    proposal_id: &str,
    reason: Option<&str>,
) -> Result<ProposalSummary, Error> {
    finalize_close(repo, git, proposal_id, ProposalStatus::Rejected, reason)
}

/// Agent withdraws its active proposal and deletes its hidden ref.
///
/// # Errors
/// [`Error::Refused`] / [`Error::Command`].
pub fn withdraw_proposal(
    repo: &Repo,
    git: &GitCli,
    proposal_id: &str,
) -> Result<ProposalSummary, Error> {
    finalize_close(repo, git, proposal_id, ProposalStatus::Withdrawn, None)
}

/// Approves a proposal: replays its selected files/patch onto current HEAD using `--3way` if
/// HEAD moved, syncs approved paths, commits via real `git commit` (so hooks/signing run),
/// and cleans up the hidden ref.
///
/// # Errors
/// [`Error::Refused`] or [`Error::Command`] on conflict/empty message.
pub fn approve_proposal(
    repo: &Repo,
    git: &GitCli,
    proposal_id: &str,
    message_override: Option<&str>,
    selected_paths: &[String],
) -> Result<ProposalSummary, Error> {
    let mut prop = get_proposal(repo, proposal_id)?;
    if !matches!(
        prop.status,
        ProposalStatus::Pending | ProposalStatus::ChangesRequested
    ) {
        return Err(Error::Refused(format!(
            "Proposal '{proposal_id}' is not pending review"
        )));
    }
    let final_msg = message_override
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&prop.message)
        .to_owned();

    let paths: Vec<String> = if selected_paths.is_empty() {
        prop.files.clone()
    } else {
        selected_paths.to_vec()
    };
    if paths.is_empty() {
        return Err(Error::Refused("Select at least one file to approve".into()));
    }

    let root = repo.root();
    let patch_bytes = diff_bytes_for_proposal(repo, git, &prop, &paths)?;
    if patch_bytes.is_empty() {
        return Err(Error::Refused(
            "Selected proposal files already match current HEAD".into(),
        ));
    }

    // Apply onto the real index via 3-way merge so moved-HEAD changes replay cleanly.
    let (code, stdout, stderr) = git.run_status(
        root,
        &["apply", "--cached", "--3way", "-"],
        Some(&patch_bytes),
    )?;
    if code != 0 {
        return Err(Error::Refused(format!(
            "Cannot replay proposal onto current HEAD without conflicts:\n{}",
            crate::cli::failure_message(&stdout, &stderr)
        )));
    }

    // Sync worktree files for the approved paths so committed tree and disk match cleanly.
    let gix_repo = repo.gix();
    let commit_oid = ObjectId::from_hex(prop.commit_id.as_bytes()).map_err(read_err)?;
    let commit_obj = gix_repo.find_commit(commit_oid).map_err(read_err)?;
    let prop_tree = commit_obj.tree().map_err(read_err)?;

    for path in &paths {
        let in_prop = prop_tree
            .lookup_entry_by_path(path)
            .map_err(read_err)?
            .is_some();
        let disk = root.join(path);
        if in_prop {
            git.run(root, &["checkout-index", "-f", "--", path], None)?;
        } else if disk.exists() {
            let _ = fs::remove_file(&disk);
        }
    }

    git.run(root, &["commit", "-m", &final_msg], None)?;

    let ref_name = format!("{PROPOSAL_REF_PREFIX}{proposal_id}");
    let _ = git.run_status(root, &["update-ref", "-d", &ref_name], None);

    prop.status = ProposalStatus::Approved;
    prop.message = final_msg;
    prop.updated_at = now_secs();
    prop.head_moved = false;
    save_summary(repo, &prop)?;
    Ok(prop)
}

/// Active proposals whose hidden commit refs resolve (for Graph dashed ghost row rendering).
pub(crate) fn active_proposal_refs(
    repo_root: &Path,
) -> Result<Vec<(String, ObjectId, ObjectId)>, Error> {
    let opened = repo::open(repo_root)?;
    let list = list_proposals(&opened)?;
    let gix_repo = opened.gix();
    let mut out = Vec::new();
    for item in list {
        if !matches!(
            item.status,
            ProposalStatus::Pending | ProposalStatus::ChangesRequested
        ) {
            continue;
        }
        let Ok(commit_oid) = ObjectId::from_hex(item.commit_id.as_bytes()) else {
            continue;
        };
        if gix_repo.find_commit(commit_oid).is_err() {
            continue;
        }
        let base_oid = ObjectId::from_hex(item.base_id.as_bytes())
            .unwrap_or_else(|_| ObjectId::null(gix_repo.object_hash()));
        out.push((item.id, commit_oid, base_oid));
    }
    Ok(out)
}

fn finalize_close(
    repo: &Repo,
    git: &GitCli,
    proposal_id: &str,
    status: ProposalStatus,
    reason: Option<&str>,
) -> Result<ProposalSummary, Error> {
    let mut prop = get_proposal(repo, proposal_id)?;
    let now = now_secs();
    if let Some(note) = reason.map(str::trim).filter(|s| !s.is_empty()) {
        prop.comments.push(ProposalComment {
            id: format!("c-{now}-{}", prop.comments.len() + 1),
            author: "Max".into(),
            text: note.to_owned(),
            path: None,
            time: now,
        });
    }
    let ref_name = format!("{PROPOSAL_REF_PREFIX}{proposal_id}");
    let _ = git.run_status(repo.root(), &["update-ref", "-d", &ref_name], None);
    prop.status = status;
    prop.updated_at = now;
    prop.head_moved = false;
    save_summary(repo, &prop)?;
    Ok(prop)
}

fn diff_bytes_for_proposal(
    repo: &Repo,
    git: &GitCli,
    prop: &ProposalSummary,
    paths: &[String],
) -> Result<Vec<u8>, Error> {
    let mut args = vec!["diff", "--binary"];
    if prop.base_id == "unborn" {
        // Diff empty tree against commit.
        args = vec![
            "diff-tree",
            "-p",
            "--binary",
            "--root",
            &prop.commit_id,
            "--",
        ];
    } else {
        args.push(&prop.base_id);
        args.push(&prop.commit_id);
        args.push("--");
    }
    for p in paths {
        args.push(p.as_str());
    }
    let out = git.run(repo.root(), &args, None)?;
    Ok(out.into_bytes())
}

fn compute_risk_flags(
    repo: &Repo,
    git: &GitCli,
    status: &Status,
    only_paths: Option<&BTreeSet<String>>,
) -> Vec<RiskFlag> {
    let mut flags = Vec::new();
    let in_scope = |p: &str| only_paths.is_none_or(|set| set.contains(p));

    for issue in check_meta_issues(repo, status) {
        if !in_scope(&issue.path) {
            continue;
        }
        let kind = match issue.kind {
            MetaIssueKind::MissingMeta => "missingMeta",
            MetaIssueKind::OrphanMeta => "orphanMeta",
            MetaIssueKind::GuidCollision => "guidCollision",
            MetaIssueKind::CaseMismatch => "caseMismatch",
        };
        flags.push(RiskFlag {
            kind: kind.into(),
            path: issue.path,
            detail: issue.detail,
        });
    }

    let lfs_info = lfs::lfs_info(git, repo).ok();
    let held_locks: HashSet<String> = lfs_info
        .as_ref()
        .filter(|info| info.supports_locks)
        .and_then(|_| lfs::lfs_locks_list(git, repo).ok())
        .map(|list| list.into_iter().map(|l| l.path).collect())
        .unwrap_or_default();

    for entry in &status.entries {
        if !in_scope(&entry.path) {
            continue;
        }
        let full = repo.root().join(&entry.path);
        let disk_bytes = fs::read(&full).ok();
        if entry.worktree == Some(crate::repo::Change::Deleted)
            || entry.index == Some(crate::repo::Change::Deleted)
        {
            flags.push(RiskFlag {
                kind: "deletedFile".into(),
                path: entry.path.clone(),
                detail: format!("Deletes existing file {}", entry.path),
            });
        } else if let Some(bytes) = &disk_bytes
            && is_binary(bytes)
        {
            flags.push(RiskFlag {
                kind: "binaryFile".into(),
                path: entry.path.clone(),
                detail: format!("Binary file changed ({} bytes)", bytes.len()),
            });
        }
        if let Some(info) = &lfs_info
            && lfs_pattern_matches(&entry.path, &info.tracked_patterns)
        {
            if let Some(bytes) = &disk_bytes
                && lfs::is_lfs_pointer(bytes)
            {
                flags.push(RiskFlag {
                    kind: "lfsPointer".into(),
                    path: entry.path.clone(),
                    detail: format!("Worktree file {} is an unhydrated LFS pointer", entry.path),
                });
            }
            if info.supports_locks && !held_locks.contains(&entry.path) {
                flags.push(RiskFlag {
                    kind: "unlockedLfs".into(),
                    path: entry.path.clone(),
                    detail: format!("LFS-tracked asset {} is not locked", entry.path),
                });
            }
        }
    }

    flags
}

fn lfs_pattern_matches(path: &str, patterns: &[String]) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    patterns.iter().any(|p| {
        let pat = p.trim().to_ascii_lowercase();
        if let Some(suffix) = pat.strip_prefix('*') {
            lower.ends_with(suffix)
        } else {
            lower == pat
        }
    })
}

fn to_agent_hunk(path: &str, side: Side, hunk: &Hunk) -> AgentHunk {
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut preview = String::new();
    for line in &hunk.lines {
        match line.kind {
            LineKind::Added => {
                added += 1;
                if preview.is_empty() {
                    preview = format!("+{}", line.text.trim_end());
                }
            }
            LineKind::Removed => {
                removed += 1;
                if preview.is_empty() {
                    preview = format!("-{}", line.text.trim_end());
                }
            }
            LineKind::Context => {}
        }
    }
    AgentHunk {
        id: stable_hunk_id(path, hunk),
        side,
        header: HunkHeader::from(hunk),
        preview: preview.chars().take(80).collect(),
        added,
        removed,
    }
}

fn nul_pathspec(paths: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in paths {
        out.extend_from_slice(p.as_bytes());
        out.push(0);
    }
    out
}

fn proposals_dir(repo: &Repo) -> PathBuf {
    repo.git_dir().join("gitgud").join("proposals")
}

fn summary_path(repo: &Repo, id: &str) -> PathBuf {
    proposals_dir(repo).join(format!("{id}.json"))
}

fn save_summary(repo: &Repo, summary: &ProposalSummary) -> Result<(), Error> {
    let dir = proposals_dir(repo);
    fs::create_dir_all(&dir).map_err(read_err)?;
    let path = summary_path(repo, &summary.id);
    let tmp = dir.join(format!("{}.tmp", summary.id));
    let json = serde_json::to_string_pretty(summary).map_err(read_err)?;
    fs::write(&tmp, json).map_err(read_err)?;
    fs::rename(&tmp, &path).map_err(read_err)?;
    Ok(())
}

fn unique_proposal_id(repo: &Repo, now: i64) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let base = format!("p-{now}-{:04x}", millis & 0xffff);
    if !summary_path(repo, &base).exists() {
        return base;
    }
    for n in 1..1000u32 {
        let candidate = format!("{base}-{n}");
        if !summary_path(repo, &candidate).exists() {
            return candidate;
        }
    }
    base
}

fn now_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(0)
}
