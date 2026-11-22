use gix::ObjectId;
use gix::remote::Direction;
use serde::{Deserialize, Serialize};

use crate::branches::count_only_in;
use crate::cli::GitCli;
use crate::remote_url::pr_url_for;
use crate::repo::{Error, Repo, read_err};
use crate::task::{Cancel, Progress};

const MAX_PUSH_COMMITS: usize = 50;

/// Strategy for pulling changes from an upstream branch (spec §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullStrategy {
    /// Follow `pull.rebase` / `pull.ff`, defaulting to fast-forward only.
    Auto,
    FfOnly,
    Merge,
    Rebase,
}

/// Outcome of a pull operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PullOutcome {
    UpToDate,
    FastForwarded,
    Merged,
    Rebased,
    Conflicts,
    Diverged {
        ahead: u32,
        behind: u32,
        upstream: String,
    },
}

/// Summary of what pulling the current branch would use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullTarget {
    pub branch: String,
    pub remote: Option<String>,
    pub upstream_full: String,
    pub upstream_short: String,
}

/// One commit waiting to be pushed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushCommit {
    pub short_id: String,
    pub subject: String,
}

/// Summary shown by the Push dialog before sending commits to a remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushInfo {
    pub branch: String,
    pub remote: Option<String>,
    pub remote_branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub needs_force: bool,
    pub commits: Vec<PushCommit>,
    pub remotes: Vec<String>,
    pub pr_url: Option<String>,
}

impl Repo {
    /// Returns the current branch's tracking target for a pull, or refuses if
    /// HEAD is detached / unborn or has no upstream configured.
    pub(crate) fn pull_target(&self) -> Result<PullTarget, Error> {
        let repo = self.gix();
        let head = repo.head().map_err(read_err)?;
        let (branch_ref, branch) = match head.kind {
            gix::head::Kind::Symbolic(r) => {
                let short = r.name.shorten().to_string();
                (r.name, short)
            }
            gix::head::Kind::Unborn(name) => {
                let short = name.shorten().to_string();
                (name, short)
            }
            gix::head::Kind::Detached { .. } => {
                return Err(Error::Refused(
                    "HEAD is detached: check out a branch before pulling".into(),
                ));
            }
        };
        let remote = repo
            .branch_remote_name(branch_ref.as_ref(), Direction::Fetch)
            .map(|n| n.as_bstr().to_string());
        let upstream = repo
            .branch_remote_tracking_ref_name(branch_ref.as_ref(), Direction::Fetch)
            .transpose()
            .map_err(read_err)?
            .ok_or_else(|| {
                Error::Refused(format!(
                    "branch {branch} has no upstream branch to pull from"
                ))
            })?;
        Ok(PullTarget {
            branch,
            remote,
            upstream_short: upstream.shorten().to_string(),
            upstream_full: upstream.as_bstr().to_string(),
        })
    }

    /// Resolves git's configured `pull.rebase` / `pull.ff` strategy, defaulting
    /// to [`PullStrategy::FfOnly`] when unconfigured (spec §3.2).
    pub(crate) fn configured_pull_strategy(&self, git: &GitCli) -> Result<PullStrategy, Error> {
        let rebase = config_get(git, self.root(), "pull.rebase")?;
        let ff = config_get(git, self.root(), "pull.ff")?;
        Ok(match rebase.as_deref() {
            Some("true" | "merges" | "interactive" | "yes" | "on" | "1") => PullStrategy::Rebase,
            Some("false" | "no" | "off" | "0") => {
                if ff.as_deref() == Some("only") {
                    PullStrategy::FfOnly
                } else {
                    PullStrategy::Merge
                }
            }
            _ => match ff.as_deref() {
                Some("false" | "no" | "off" | "0") => PullStrategy::Merge,
                _ => PullStrategy::FfOnly,
            },
        })
    }

    /// Saves `pull.rebase` in the repository's local config when the user asks
    /// gitgud to remember their Merge / Rebase choice.
    pub(crate) fn set_pull_rebase(&self, git: &GitCli, rebase: bool) -> Result<(), Error> {
        git.run(
            self.root(),
            &[
                "config",
                "--local",
                "pull.rebase",
                if rebase { "true" } else { "false" },
            ],
            None,
        )?;
        if !rebase {
            git.run(self.root(), &["config", "--local", "pull.ff", "true"], None)?;
        }
        Ok(())
    }

    /// Computes ahead / behind of `HEAD` relative to `upstream_full` (`None`
    /// when the upstream tracking ref doesn't exist locally yet).
    pub(crate) fn upstream_counts(&self, upstream_full: &str) -> Result<(u32, u32), Error> {
        let repo = self.gix();
        let mut up_ref = repo
            .try_find_reference(upstream_full)
            .map_err(read_err)?
            .ok_or_else(|| {
                Error::Refused(format!(
                    "upstream ref {upstream_full} was not found on the remote"
                ))
            })?;
        let up_id = up_ref.peel_to_id().map_err(read_err)?.detach();
        let head = repo.head().map_err(read_err)?;
        match head.kind {
            gix::head::Kind::Unborn(_) => Ok((0, 1)),
            _ => {
                let tip = repo.head_id().map_err(read_err)?.detach();
                Ok((
                    count_only_in(&repo, tip, up_id)?,
                    count_only_in(&repo, up_id, tip)?,
                ))
            }
        }
    }

    /// Fast-forwards the current branch to `upstream`.
    pub fn fast_forward(&self, git: &GitCli, upstream: &str) -> Result<(), Error> {
        git.run(
            self.root(),
            &["merge", "--ff-only", "--no-stat", upstream],
            None,
        )
        .map(drop)
    }

    /// Summary of what pushing the current branch (to `remote_override` or its
    /// configured remote) will send.
    ///
    /// # Errors
    /// [`Error::Refused`] when HEAD is detached or unborn; [`Error::Read`] on
    /// git read errors.
    pub fn push_info(&self, remote_override: Option<&str>) -> Result<PushInfo, Error> {
        let repo = self.gix();
        let head = repo.head().map_err(read_err)?;
        let (branch_ref, branch, tip) = match head.kind {
            gix::head::Kind::Symbolic(r) => {
                let short = r.name.shorten().to_string();
                let id = repo
                    .head_id()
                    .map_err(|_| Error::Refused("make a commit before pushing".into()))?
                    .detach();
                (r.name, short, id)
            }
            gix::head::Kind::Unborn(_) => {
                return Err(Error::Refused("make a commit before pushing".into()));
            }
            gix::head::Kind::Detached { .. } => {
                return Err(Error::Refused(
                    "HEAD is detached: create or switch to a branch before pushing".into(),
                ));
            }
        };

        let mut remotes: Vec<String> = repo
            .remote_names()
            .into_iter()
            .map(|n| n.to_string())
            .collect();
        remotes.sort();
        if let Some(pos) = remotes.iter().position(|r| r == "origin") {
            let origin = remotes.remove(pos);
            remotes.insert(0, origin);
        }

        let configured_remote = repo
            .branch_remote_name(branch_ref.as_ref(), Direction::Push)
            .or_else(|| repo.branch_remote_name(branch_ref.as_ref(), Direction::Fetch))
            .map(|n| n.as_bstr().to_string());

        let remote = remote_override
            .map(ToOwned::to_owned)
            .or(configured_remote)
            .or_else(|| remotes.first().cloned());

        let configured_merge = repo
            .config_snapshot()
            .string(&format!("branch.{branch}.merge"))
            .map(|s| {
                let s = s.to_string();
                s.strip_prefix("refs/heads/").unwrap_or(&s).to_owned()
            });
        let remote_branch = if remote_override.is_none() {
            configured_merge.unwrap_or_else(|| branch.clone())
        } else {
            branch.clone()
        };

        let upstream_full = if remote_override.is_none() {
            repo.branch_remote_tracking_ref_name(branch_ref.as_ref(), Direction::Fetch)
                .transpose()
                .map_err(read_err)?
                .map(|r| r.as_bstr().to_string())
        } else {
            remote
                .as_deref()
                .map(|r| format!("refs/remotes/{r}/{remote_branch}"))
        };

        let (upstream, ahead, behind, commits) = if let Some(full) = &upstream_full
            && let Some(mut u) = repo.try_find_reference(full.as_str()).map_err(read_err)?
        {
            let up_id = u.peel_to_id().map_err(read_err)?.detach();
            (
                Some(u.name().shorten().to_string()),
                count_only_in(&repo, tip, up_id)?,
                count_only_in(&repo, up_id, tip)?,
                collect_commits(&repo, tip, &[up_id], MAX_PUSH_COMMITS)?,
            )
        } else {
            let mut hidden = Vec::new();
            let refs = repo.references().map_err(read_err)?;
            for r in refs.remote_branches().map_err(read_err)? {
                let mut r = r.map_err(|e| Error::Read(e.to_string()))?;
                if matches!(r.target(), gix::refs::TargetRef::Symbolic(_)) {
                    continue;
                }
                if let Ok(id) = r.peel_to_id() {
                    hidden.push(id.detach());
                }
            }
            let walk = repo
                .rev_walk([tip])
                .with_hidden(hidden.clone())
                .all()
                .map_err(read_err)?;
            let mut count = 0u32;
            for info in walk {
                info.map_err(read_err)?;
                count += 1;
            }
            (
                None,
                count,
                0u32,
                collect_commits(&repo, tip, &hidden, MAX_PUSH_COMMITS)?,
            )
        };

        let pr_url = remote
            .as_deref()
            .and_then(|r| self.remote_url(r))
            .and_then(|u| pr_url_for(&u, &remote_branch));

        Ok(PushInfo {
            branch,
            remote,
            remote_branch,
            upstream,
            ahead,
            behind,
            needs_force: behind > 0,
            commits,
            remotes,
            pr_url,
        })
    }

    /// Pushes `branch` to `remote`'s `remote_branch` with progress and cancel.
    /// Force pushes strictly use `--force-with-lease`.
    ///
    /// # Errors
    /// [`Error::Cancelled`], [`Error::Refused`] when `remote` is empty, or
    /// [`Error::Command`] with git's message.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &self,
        git: &GitCli,
        remote: &str,
        branch: &str,
        remote_branch: &str,
        set_upstream: bool,
        force_with_lease: bool,
        on_progress: &mut dyn FnMut(Progress),
        cancel: &Cancel,
    ) -> Result<(), Error> {
        let remote = remote.trim();
        if remote.is_empty() {
            return Err(Error::Refused("no remote configured to push to".into()));
        }
        let target = if remote_branch.trim().is_empty() {
            branch
        } else {
            remote_branch.trim()
        };
        let refspec = format!("refs/heads/{branch}:refs/heads/{target}");
        let mut args = vec!["push", "--progress"];
        if set_upstream {
            args.push("--set-upstream");
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        args.push(remote);
        args.push(&refspec);
        git.run_with_progress(self.root(), &args, on_progress, cancel)
            .map(drop)
    }

    /// Deletes `branch` on `remote`. Irreversible remotely.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn delete_remote_branch(
        &self,
        git: &GitCli,
        remote: &str,
        branch: &str,
    ) -> Result<(), Error> {
        git.run(self.root(), &["push", remote, "--delete", branch], None)
            .map(drop)
    }
}

fn config_get(git: &GitCli, root: &std::path::Path, key: &str) -> Result<Option<String>, Error> {
    let (code, out, err) = git.run_status(root, &["config", "--get", key], None)?;
    match code {
        0 => Ok(Some(out.trim().to_ascii_lowercase())),
        1 => Ok(None),
        _ => Err(Error::Command {
            command: format!("git config --get {key}"),
            message: err.trim().to_owned(),
        }),
    }
}

fn collect_commits(
    repo: &gix::Repository,
    tip: ObjectId,
    hidden: &[ObjectId],
    limit: usize,
) -> Result<Vec<PushCommit>, Error> {
    let walk = repo
        .rev_walk([tip])
        .with_hidden(hidden.iter().copied())
        .all()
        .map_err(read_err)?;
    let mut commits = Vec::new();
    for info in walk.take(limit) {
        let info = info.map_err(read_err)?;
        let commit = repo.find_commit(info.id).map_err(read_err)?;
        let msg = commit.message().map_err(read_err)?;
        commits.push(PushCommit {
            short_id: info.id.to_hex_with_len(7).to_string(),
            subject: msg.summary().to_string(),
        });
    }
    Ok(commits)
}
