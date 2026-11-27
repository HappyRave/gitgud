use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// Deterministic 12-hex-char hunk identifier derived from `path`, changed lines (line-shift
/// invariant), and zero-based duplicate-content `occurrence` index within the same file.
#[must_use]
pub fn stable_hunk_id_at(path: &str, hunk: &Hunk, occurrence: usize) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(path.as_bytes());
    feed(b"\0");
    for line in &hunk.lines {
        let stripped = line.text.trim_end_matches(['\r', '\n']);
        match line.kind {
            LineKind::Added => {
                feed(b"+");
                feed(stripped.as_bytes());
                feed(b"\n");
            }
            LineKind::Removed => {
                feed(b"-");
                feed(stripped.as_bytes());
                feed(b"\n");
            }
            LineKind::Context => {}
        }
    }
    if occurrence > 0 {
        feed(b"\0");
        feed(&(occurrence as u64).to_le_bytes());
    }
    format!("{hash:012x}").chars().take(12).collect()
}

/// Deterministic 12-hex-char hunk identifier derived from `path` and changed lines.
#[must_use]
pub fn stable_hunk_id(path: &str, hunk: &Hunk) -> String {
    stable_hunk_id_at(path, hunk, 0)
}

fn assign_hunk_ids<'a>(path: &str, hunks: impl IntoIterator<Item = &'a Hunk>) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut ids = Vec::new();
    for hunk in hunks {
        let base = stable_hunk_id_at(path, hunk, 0);
        let occ = seen.entry(base.clone()).or_insert(0);
        let id = if *occ == 0 {
            base
        } else {
            stable_hunk_id_at(path, hunk, *occ)
        };
        *occ += 1;
        ids.push(id);
    }
    ids
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
                let ids = assign_hunk_ids(&entry.path, &diff.hunks);
                for (h, hid) in diff.hunks.iter().zip(ids) {
                    hunks.push(to_agent_hunk(side, h, hid));
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

struct PreparedTree {
    tree_oid: String,
    base_id: String,
    head_oid: Option<ObjectId>,
    head_tree_id: Option<ObjectId>,
    selected_paths: Vec<String>,
}

fn build_proposal_tree(
    repo: &Repo,
    git: &GitCli,
    status: &Status,
    input: &CreateProposalInput,
    tmp_idx: &Path,
) -> Result<PreparedTree, Error> {
    let _ = fs::remove_file(tmp_idx);
    let root = repo.root();
    let gix_repo = repo.gix();
    let head_oid = gix_repo.head_id().ok().map(gix::Id::detach);
    let base_id = head_oid
        .map(|oid| oid.to_string())
        .unwrap_or_else(|| "unborn".into());
    let head_tree_id = head_oid
        .and_then(|oid| gix_repo.find_commit(oid).ok())
        .and_then(|c| c.tree_id().ok())
        .map(gix::Id::detach);

    let idx_env = [("GIT_INDEX_FILE", tmp_idx)];
    if let Some(oid) = head_oid {
        let hex = oid.to_string();
        git.run_env(root, &["read-tree", &hex], None, &idx_env)?;
    }

    let selected_paths: Vec<String>;
    if !input.hunk_ids.is_empty() {
        let wanted: HashSet<&str> = input.hunk_ids.iter().map(String::as_str).collect();
        let mut patch_bundle = Vec::new();
        let mut matched_paths = BTreeSet::new();
        let mut matched_ids = HashSet::new();
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
                    let ids = assign_hunk_ids(&entry.path, hunks.iter().map(|(h, _)| h));
                    for ((_, raw_patch), hid) in hunks.into_iter().zip(ids) {
                        if wanted.contains(hid.as_str()) {
                            patch_bundle.extend_from_slice(&raw_patch);
                            matched_paths.insert(entry.path.clone());
                            matched_ids.insert(hid);
                        }
                    }
                }
            }
        }
        let mut seen_missing = HashSet::new();
        let missing: Vec<&str> = input
            .hunk_ids
            .iter()
            .map(String::as_str)
            .filter(|id| !matched_ids.contains(*id) && seen_missing.insert(*id))
            .collect();
        if !missing.is_empty() {
            return Err(Error::Refused(format!(
                "Unknown or stale hunk ID(s): {}. Refresh status before proposing.",
                missing.join(", ")
            )));
        }
        if patch_bundle.is_empty() {
            return Err(Error::Refused(
                "None of the requested hunk IDs matched current changes".into(),
            ));
        }
        git.run_env(
            root,
            &["apply", "--cached", "--whitespace=nowarn", "-"],
            Some(&patch_bundle),
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
            expand_meta_pairs(status, &raw_paths)
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
    Ok(PreparedTree {
        tree_oid,
        base_id,
        head_oid,
        head_tree_id,
        selected_paths,
    })
}

fn commit_prepared_proposal(
    repo: &Repo,
    git: &GitCli,
    status: &Status,
    id: &str,
    now: i64,
    input: &CreateProposalInput,
    prepared: PreparedTree,
) -> Result<ProposalSummary, Error> {
    let message = input.message.trim();
    let root = repo.root();
    let mut commit_args = vec!["commit-tree", prepared.tree_oid.as_str()];
    if prepared.head_oid.is_some() {
        commit_args.extend(["-p", prepared.base_id.as_str()]);
    }
    let commit_oid = git
        .run(root, &commit_args, Some(message.as_bytes()))?
        .trim()
        .to_owned();

    let ref_name = format!("{PROPOSAL_REF_PREFIX}{id}");
    git.run(root, &["update-ref", &ref_name, &commit_oid], None)?;

    let filter: BTreeSet<String> = prepared.selected_paths.iter().cloned().collect();
    let risk_flags = compute_risk_flags(repo, git, status, Some(&filter));

    let summary = ProposalSummary {
        id: id.to_owned(),
        short_commit: commit_oid.chars().take(7).collect(),
        commit_id: commit_oid,
        base_id: prepared.base_id,
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
        files: prepared.selected_paths,
        risk_flags,
        comments: Vec::new(),
        head_moved: false,
    };
    save_summary(repo, &summary)?;
    Ok(summary)
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
    let prop_dir = proposals_dir(repo);
    fs::create_dir_all(&prop_dir).map_err(read_err)?;

    let tmp_idx = prop_dir.join(format!("tmp-{id}.index"));
    let _guard = TempIndexGuard(tmp_idx.clone());
    let prepared = build_proposal_tree(repo, git, &status, input, &tmp_idx)?;
    if let (Some(head_tid), Ok(tid)) = (
        prepared.head_tree_id,
        ObjectId::from_hex(prepared.tree_oid.as_bytes()),
    ) && head_tid == tid
    {
        return Err(Error::Refused(
            "Proposal has no changes compared to HEAD".into(),
        ));
    }

    commit_prepared_proposal(repo, git, &status, &id, now, input, prepared)
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

/// Result returned by [`wait_for_proposal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitProposalResult {
    pub proposal: ProposalSummary,
    pub timed_out: bool,
}

/// Polls until proposal `id` leaves [`ProposalStatus::Pending`] or `timeout_secs` elapses.
///
/// # Errors
/// [`Error::Refused`] if the proposal doesn't exist, or [`Error::Read`] if metadata fails to read.
pub fn wait_for_proposal(
    repo: &Repo,
    id: &str,
    timeout_secs: u64,
    poll_interval: Duration,
) -> Result<WaitProposalResult, Error> {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    let interval = poll_interval.max(Duration::from_millis(20));
    loop {
        let proposal = get_proposal(repo, id)?;
        if proposal.status != ProposalStatus::Pending {
            return Ok(WaitProposalResult {
                proposal,
                timed_out: false,
            });
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return Ok(WaitProposalResult {
                proposal,
                timed_out: true,
            });
        }
        std::thread::sleep(interval.min(deadline - now));
    }
}

/// Fetches a single proposal by ID.
///
/// # Errors
/// [`Error::Refused`] if not found.
pub fn get_proposal(repo: &Repo, proposal_id: &str) -> Result<ProposalSummary, Error> {
    // Ids come from agents and become file names: only what `unique_proposal_id` makes.
    if proposal_id.is_empty()
        || !proposal_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Refused(format!(
            "'{proposal_id}' is not a proposal id"
        )));
    }
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
        author: reviewer(repo),
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
    // The commit below takes the whole index, so anything already staged would go in too.
    match git.run_status(root, &["diff", "--cached", "--quiet"], None)? {
        (0, ..) => {}
        (1, ..) => {
            return Err(Error::Refused(
                "Unstage your staged changes first: approving commits everything staged".into(),
            ));
        }
        (_, stdout, stderr) => {
            return Err(crate::cli::command_error(
                &["diff", "--cached", "--quiet"],
                crate::cli::failure_message(&stdout, &stderr),
            ));
        }
    }
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
        // The index matched HEAD before, so this drops exactly what the failed apply left.
        git.run(root, &["reset", "-q"], None)?;
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
    git.run(root, &["update-ref", "-d", &ref_name], None)?;

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
    if !matches!(
        prop.status,
        ProposalStatus::Pending | ProposalStatus::ChangesRequested
    ) {
        return Err(Error::Refused(format!(
            "Proposal '{proposal_id}' is already finalized"
        )));
    }
    let now = now_secs();
    if let Some(note) = reason.map(str::trim).filter(|s| !s.is_empty()) {
        prop.comments.push(ProposalComment {
            id: format!("c-{now}-{}", prop.comments.len() + 1),
            author: reviewer(repo),
            text: note.to_owned(),
            path: None,
            time: now,
        });
    }
    let ref_name = format!("{PROPOSAL_REF_PREFIX}{proposal_id}");
    git.run(repo.root(), &["update-ref", "-d", &ref_name], None)?;
    prop.status = status;
    prop.updated_at = now;
    prop.head_moved = false;
    save_summary(repo, &prop)?;
    Ok(prop)
}

/// The reviewer's name for comments: git's `user.name`.
fn reviewer(repo: &Repo) -> String {
    repo.gix()
        .config_snapshot()
        .string("user.name")
        .map_or_else(|| "You".to_owned(), |name| name.to_string())
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
    git.run_bytes(repo.root(), &args, None)
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
    // Asking the server is a network round trip: only when a changed file is in
    // LFS. When it can't answer, no file is called unlocked.
    let held_locks: Option<HashSet<String>> = lfs_info
        .as_ref()
        .filter(|info| {
            info.supports_locks
                && status.entries.iter().any(|e| {
                    in_scope(&e.path) && lfs_pattern_matches(&e.path, &info.tracked_patterns)
                })
        })
        .and_then(|_| lfs::lfs_locks_list(git, repo).ok())
        .map(|list| list.into_iter().map(|l| l.path).collect());

    for entry in &status.entries {
        if !in_scope(&entry.path) {
            continue;
        }
        let full = repo.root().join(&entry.path);
        // The head tells binary from text and a pointer from content: textures can
        // be hundreds of MB.
        let disk_bytes = read_head(&full);
        if entry.worktree == Some(crate::repo::Change::Deleted)
            || entry.index == Some(crate::repo::Change::Deleted)
        {
            flags.push(RiskFlag {
                kind: "deletedFile".into(),
                path: entry.path.clone(),
                detail: format!("Deletes existing file {}", entry.path),
            });
        } else if let Some((bytes, len)) = &disk_bytes
            && is_binary(bytes)
        {
            flags.push(RiskFlag {
                kind: "binaryFile".into(),
                path: entry.path.clone(),
                detail: format!("Binary file changed ({len} bytes)"),
            });
        }
        if let Some(info) = &lfs_info
            && lfs_pattern_matches(&entry.path, &info.tracked_patterns)
        {
            if let Some((bytes, _)) = &disk_bytes
                && lfs::is_lfs_pointer(bytes)
            {
                flags.push(RiskFlag {
                    kind: "lfsPointer".into(),
                    path: entry.path.clone(),
                    detail: format!("Worktree file {} is an unhydrated LFS pointer", entry.path),
                });
            }
            if held_locks
                .as_ref()
                .is_some_and(|held| !held.contains(&entry.path))
            {
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

/// The first 8 KiB of `path` and its full size, `None` when it can't be read.
fn read_head(path: &Path) -> Option<(Vec<u8>, u64)> {
    use std::io::Read;
    let file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut head = Vec::with_capacity(8192);
    file.take(8192).read_to_end(&mut head).ok()?;
    Some((head, len))
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

fn to_agent_hunk(side: Side, hunk: &Hunk, id: String) -> AgentHunk {
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
        id,
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
    repo.gix().common_dir().join("gitgud").join("proposals")
}

fn summary_path(repo: &Repo, id: &str) -> PathBuf {
    proposals_dir(repo).join(format!("{id}.json"))
}

fn save_summary(repo: &Repo, summary: &ProposalSummary) -> Result<(), Error> {
    let json = serde_json::to_vec_pretty(summary).map_err(read_err)?;
    crate::atomic_file::write_atomic(&summary_path(repo, &summary.id), &json).map_err(read_err)
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

/// Relative file path for writing coding agent instructions for `target`.
#[must_use]
pub fn agent_setup_rel_path(target: &str) -> &'static str {
    match target {
        "claude" => ".claude/skills/gitgud/SKILL.md",
        "codex" => ".codex/skills/gitgud/SKILL.md",
        "cursor" => ".cursor/rules/gitgud.mdc",
        _ => "GITGUD_AGENT.md",
    }
}

/// Canonical instructions teaching external coding agents the safe gitgud proposal workflow.
#[must_use]
pub fn agent_setup_text() -> &'static str {
    "\
# gitgud Agent Workflow Instructions

Never commit, checkout, merge, rebase, or push directly with `git` when working in a `gitgud` repository.
Instead, stage and propose changes cleanly for human review inside gitgud:

1. Inspect status & stable hunk IDs: `gitgud-cli status --json` (or MCP `status`).
2. Propose logical commits without modifying the user's index:
   `gitgud-cli propose -m \"<commit message>\" --why \"<why & test evidence>\" --agent <name> --json`
3. Wait for human review feedback or approval:
   `gitgud-cli proposal <id> --wait --json` (or MCP `proposal_wait` / `proposal`).
4. For Unity binary/LFS assets, acquire locks via `gitgud-cli locks lock <path>`.
"
}

const HOOK_COMMAND: &str = "gitgud-cli agent hook --event stop --agent claude --json";

/// JSON snippet for `.claude/settings.local.json` configuring the opt-in Stop hook helper.
#[must_use]
pub fn claude_stop_hook_snippet() -> serde_json::Value {
    serde_json::json!({
        "hooks": {
            "Stop": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": HOOK_COMMAND
                        }
                    ]
                }
            ]
        }
    })
}

fn contains_gitgud_hook(val: &serde_json::Value) -> bool {
    match val {
        serde_json::Value::String(s) => s.contains("gitgud-cli agent hook"),
        serde_json::Value::Array(arr) => arr.iter().any(contains_gitgud_hook),
        serde_json::Value::Object(map) => map.values().any(contains_gitgud_hook),
        _ => false,
    }
}

/// Idempotently writes or updates `.claude/settings.local.json` with gitgud's opt-in Stop hook.
///
/// # Errors
/// [`Error::Read`] if reading, parsing, or writing `.claude/settings.local.json` fails.
pub fn write_claude_stop_hook(repo_root: &Path) -> Result<String, Error> {
    let rel = ".claude/settings.local.json";
    let dest = repo_root.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(read_err)?;
    }

    let mut root = match fs::read_to_string(&dest) {
        Ok(raw) if !raw.trim().is_empty() => serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| Error::Read(format!("invalid JSON in {rel}: {e}")))?,
        Ok(_) => serde_json::json!({}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(read_err(e)),
    };

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| Error::Read(format!("{rel} must contain a JSON object")))?;
    let hooks_val = root_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks_val
        .as_object_mut()
        .ok_or_else(|| Error::Read(format!("{rel} hooks field must be an object")))?;
    let stop_val = hooks_obj
        .entry("Stop")
        .or_insert_with(|| serde_json::json!([]));
    let stop_arr = stop_val
        .as_array_mut()
        .ok_or_else(|| Error::Read(format!("{rel} hooks.Stop must be an array")))?;

    if !stop_arr.iter().any(contains_gitgud_hook) {
        stop_arr.push(serde_json::json!({
            "hooks": [{ "type": "command", "command": HOOK_COMMAND }]
        }));
    }

    let rendered = serde_json::to_string_pretty(&root).map_err(read_err)?;
    fs::write(&dest, format!("{rendered}\n")).map_err(read_err)?;
    Ok(rel.to_owned())
}

/// Creates a WIP proposal for uncommitted changes unless the worktree is clean or an active
/// `Pending` proposal from `agent` already captures the exact same tree state.
///
/// # Errors
/// [`Error::Read`] or [`Error::Command`] on git failure.
pub fn run_agent_hook(
    repo: &Repo,
    git: &GitCli,
    agent: &str,
    message: Option<&str>,
) -> Result<Option<ProposalSummary>, Error> {
    let agent = agent.trim();
    let agent = if agent.is_empty() { "agent" } else { agent };
    let status = repo.status()?;
    if status.entries.is_empty() {
        return Ok(None);
    }

    let now = now_secs();
    let id = unique_proposal_id(repo, now);
    let prop_dir = proposals_dir(repo);
    fs::create_dir_all(&prop_dir).map_err(read_err)?;
    let tmp_idx = prop_dir.join(format!("tmp-{id}.index"));
    let _guard = TempIndexGuard(tmp_idx.clone());

    let input = CreateProposalInput {
        message: String::new(),
        why: Some("Auto-proposed via gitgud agent Stop hook".into()),
        agent: Some(agent.to_owned()),
        paths: Vec::new(),
        hunk_ids: Vec::new(),
    };
    let prepared = build_proposal_tree(repo, git, &status, &input, &tmp_idx)?;
    let Ok(tid) = ObjectId::from_hex(prepared.tree_oid.as_bytes()) else {
        return Ok(None);
    };
    if prepared.head_tree_id == Some(tid) {
        return Ok(None);
    }

    let gix_repo = repo.gix();
    let already_pending = list_proposals(repo)?.into_iter().any(|p| {
        p.status == ProposalStatus::Pending
            && p.agent == agent
            && ObjectId::from_hex(p.commit_id.as_bytes())
                .ok()
                .and_then(|cid| gix_repo.find_commit(cid).ok())
                .and_then(|c| c.tree_id().ok())
                .is_some_and(|existing_tid| existing_tid == tid)
    });
    if already_pending {
        return Ok(None);
    }

    let default_msg = format!("wip({agent}): auto-proposed session changes");
    let msg = message
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_msg)
        .to_owned();
    let final_input = CreateProposalInput {
        message: msg,
        ..input
    };
    let summary = commit_prepared_proposal(repo, git, &status, &id, now, &final_input, prepared)?;
    Ok(Some(summary))
}

/// Writes coding agent instructions for `target` inside `repo_root` and returns the relative path written.
///
/// # Errors
/// [`Error::Read`] if creating parent folders or writing the file fails.
pub fn write_agent_setup(repo_root: &Path, target: &str) -> Result<String, Error> {
    let rel = agent_setup_rel_path(target);
    let dest = repo_root.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(read_err)?;
    }
    fs::write(&dest, agent_setup_text()).map_err(read_err)?;
    Ok(rel.to_owned())
}

/// A git worktree attached to the repository (including the main working tree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
    pub dirty_count: usize,
    pub is_main: bool,
}

/// Parses `git worktree list --porcelain` blocks into `(path, branch, head)` tuples.
#[must_use]
pub fn parse_worktree_porcelain(stdout: &str) -> Vec<(String, Option<String>, String)> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut head = String::new();

    for raw in stdout.lines().chain(std::iter::once("")) {
        let line = raw.trim();
        if line.is_empty() {
            if let Some(p) = path.take() {
                out.push((p, branch.take(), std::mem::take(&mut head)));
            }
            branch = None;
            head.clear();
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = Some(rest.replace('\\', "/"));
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            head = rest.to_owned();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            let short = rest.strip_prefix("refs/heads/").unwrap_or(rest);
            branch = Some(short.to_owned());
        }
    }
    out
}

/// Lists all git worktrees attached to `repo`, computing dirty file counts.
///
/// # Errors
/// [`Error::Command`] if `git worktree list` fails.
pub fn list_worktrees(git: &GitCli, repo: &Repo) -> Result<Vec<WorktreeInfo>, Error> {
    let out = git.run(repo.root(), &["worktree", "list", "--porcelain"], None)?;
    let main_canon = fs::canonicalize(repo.root()).unwrap_or_else(|_| repo.root().to_path_buf());
    let mut infos = Vec::new();
    for (path, branch, head) in parse_worktree_porcelain(&out) {
        let wt_path = Path::new(&path);
        let wt_canon = fs::canonicalize(wt_path).unwrap_or_else(|_| wt_path.to_path_buf());
        let is_main = wt_canon == main_canon;
        let dirty_count = repo::open(wt_path)
            .and_then(|r| r.status())
            .map(|s| s.entries.len())
            .unwrap_or(0);
        infos.push(WorktreeInfo {
            path,
            branch,
            head,
            dirty_count,
            is_main,
        });
    }
    Ok(infos)
}

/// Captures uncommitted changes inside linked `worktree_path` into a shared proposal on `main_repo`.
///
/// # Errors
/// [`Error::Refused`] if the worktree is clean or message empty; [`Error::Command`] / [`Error::Read`] on git failure.
pub fn propose_from_worktree(
    main_repo: &Repo,
    git: &GitCli,
    worktree_path: &Path,
    message: &str,
    why: Option<&str>,
    agent: Option<&str>,
) -> Result<ProposalSummary, Error> {
    let wt_repo = repo::open(worktree_path)?;
    if wt_repo.status()?.entries.is_empty() {
        return Err(Error::Refused(
            "Selected worktree has no uncommitted changes to propose".into(),
        ));
    }
    let input = CreateProposalInput {
        message: message.to_owned(),
        why: why.map(str::to_owned),
        agent: Some(
            agent
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("worktree")
                .to_owned(),
        ),
        paths: Vec::new(),
        hunk_ids: Vec::new(),
    };
    let summary = create_proposal(&wt_repo, git, &input)?;
    // Verify it is readable via main_repo immediately.
    let _ = get_proposal(main_repo, &summary.id)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_worktree_porcelain_output() {
        let raw = "\
worktree C:\\repos\\boat
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree C:\\repos\\boat-agent
HEAD 2222222222222222222222222222222222222222
branch refs/heads/agent/physics

worktree C:\\repos\\boat-detached
HEAD 3333333333333333333333333333333333333333
detached
";
        let parsed = parse_worktree_porcelain(raw);
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[0],
            (
                "C:/repos/boat".into(),
                Some("main".into()),
                "1111111111111111111111111111111111111111".into()
            )
        );
        assert_eq!(parsed[1].1.as_deref(), Some("agent/physics"));
        assert_eq!(parsed[2].1, None);
    }
}
