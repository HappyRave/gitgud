use gix::ObjectId;
use gix::remote::Direction;
use serde::Serialize;

use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};

/// The sidebar's branch lists (R1-A).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Branches {
    /// Sorted by name; folder grouping (`feature/…`) is the view's job.
    pub local: Vec<Branch>,
    pub remotes: Vec<Remote>,
    /// When the repo last fetched (`FETCH_HEAD`'s mtime), seconds since the Unix epoch.
    pub fetched: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Branch {
    /// Short name, e.g. `feature/login`.
    pub name: String,
    pub current: bool,
    /// Short name of the remote-tracking branch, e.g. `origin/main`.
    pub upstream: Option<String>,
    /// Commits on this branch not on its upstream.
    pub ahead: u32,
    /// Commits on its upstream not on this branch.
    pub behind: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Remote {
    pub name: String,
    /// Short names, e.g. `origin/main`, sorted; `origin/HEAD` left out.
    pub branches: Vec<String>,
}

/// A name typed for a new branch or tag, trimmed. One starting with `-` would
/// reach git as an option (`-D main` deletes main), so it is refused.
pub(crate) fn new_ref_name<'a>(name: &'a str, what: &str) -> Result<&'a str, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Refused(format!("{what} name can't be empty")));
    }
    if name.starts_with('-') {
        return Err(Error::Refused(format!(
            "a {what} name can't start with '-'"
        )));
    }
    Ok(name)
}

impl Repo {
    /// Local branches with their upstream and ahead / behind counts, and each
    /// remote's branches.
    ///
    /// # Errors
    /// [`Error::Read`] when refs or commits can't be read.
    pub fn branches(&self) -> Result<Branches, Error> {
        let repo = self.gix();
        let current = repo.head_name().map_err(read_err)?;
        let refs = repo.references().map_err(read_err)?;

        let mut local = Vec::new();
        for r in refs.local_branches().map_err(read_err)? {
            let r = r.map_err(|e| Error::Read(e.to_string()))?;
            let Some(tip) = r.target().try_id().map(ToOwned::to_owned) else {
                continue;
            };
            let upstream = repo
                .branch_remote_tracking_ref_name(r.name(), Direction::Fetch)
                .transpose()
                .map_err(read_err)?;
            let (upstream_name, ahead, behind) = match upstream {
                Some(full) => match repo.try_find_reference(full.as_ref()).map_err(read_err)? {
                    Some(mut u) => {
                        let up = u.peel_to_id().map_err(read_err)?.detach();
                        (
                            Some(full.shorten().to_string()),
                            count_only_in(&repo, tip, up)?,
                            count_only_in(&repo, up, tip)?,
                        )
                    }
                    // Configured but never fetched: show it, with nothing to compare.
                    None => (Some(full.shorten().to_string()), 0, 0),
                },
                None => (None, 0, 0),
            };
            local.push(Branch {
                name: r.name().shorten().to_string(),
                current: current.as_ref().is_some_and(|c| c.as_ref() == r.name()),
                upstream: upstream_name,
                ahead,
                behind,
            });
        }
        local.sort_by(|a, b| a.name.cmp(&b.name));

        let mut remotes: Vec<Remote> = repo
            .remote_names()
            .into_iter()
            .map(|n| Remote {
                name: n.to_string(),
                branches: Vec::new(),
            })
            .collect();
        for r in refs.remote_branches().map_err(read_err)? {
            let r = r.map_err(|e| Error::Read(e.to_string()))?;
            if matches!(r.target(), gix::refs::TargetRef::Symbolic(_)) {
                continue;
            }
            let short = r.name().shorten().to_string();
            if let Some(remote) = remotes
                .iter_mut()
                .find(|m| short.starts_with(&format!("{}/", m.name)))
            {
                remote.branches.push(short);
            }
        }
        for remote in &mut remotes {
            remote.branches.sort();
        }
        let fetched = std::fs::metadata(repo.common_dir().join("FETCH_HEAD"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_secs()).ok());
        Ok(Branches {
            local,
            remotes,
            fetched,
        })
    }

    /// Paths whose content differs between HEAD and branch `name`: what a
    /// checkout rewrites, so the op log can snapshot exactly those.
    ///
    /// # Errors
    /// [`Error::Read`] when `name` isn't a local branch.
    pub fn checkout_paths(&self, name: &str) -> Result<Vec<String>, Error> {
        let repo = self.gix();
        let full = format!("refs/heads/{name}");
        let mut target = repo
            .try_find_reference(full.as_str())
            .map_err(read_err)?
            .ok_or_else(|| Error::Refused(format!("there is no branch {name}")))?;
        let target_id = target.peel_to_id().map_err(read_err)?.detach();
        self.diff_head_to_oid(target_id)
    }

    /// Paths whose content differs between HEAD and revision `rev`.
    ///
    /// # Errors
    /// [`Error::Command`] when `rev` doesn't resolve to a commit.
    pub fn checkout_rev_paths(&self, git: &GitCli, rev: &str) -> Result<Vec<String>, Error> {
        let hex = git.run(
            self.root(),
            &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
            None,
        )?;
        let id = ObjectId::from_hex(hex.trim().as_bytes()).map_err(read_err)?;
        self.diff_head_to_oid(id)
    }

    fn diff_head_to_oid(&self, target_id: ObjectId) -> Result<Vec<String>, Error> {
        let repo = self.gix();
        let target_tree = repo
            .find_commit(target_id)
            .map_err(read_err)?
            .tree()
            .map_err(read_err)?;
        let head_tree = match repo.head_tree() {
            Ok(t) => t,
            Err(_) if repo.head().map_err(read_err)?.is_unborn() => repo.empty_tree(),
            Err(e) => return Err(read_err(e)),
        };
        let changes = repo
            .diff_tree_to_tree(
                &head_tree,
                &target_tree,
                gix::diff::Options::default().with_rewrites(None),
            )
            .map_err(read_err)?;
        let mut paths: Vec<String> = changes
            .iter()
            .filter(|c| !c.entry_mode().is_tree())
            .map(|c| c.location().to_string())
            .collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Switches to local branch `name`. Local changes travel along when they
    /// can; git refuses (and nothing changes) when they'd be overwritten.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn checkout(&self, git: &GitCli, name: &str) -> Result<(), Error> {
        git.run(self.root(), &["switch", "--no-guess", name], None)
            .map(drop)
    }

    /// Creates branch `name` at `at` (or HEAD when `None`), optionally checking it out.
    ///
    /// # Errors
    /// [`Error::Refused`] for an empty name, or [`Error::Command`] with git's message.
    pub fn create_branch(
        &self,
        git: &GitCli,
        name: &str,
        at: Option<&str>,
        checkout: bool,
    ) -> Result<(), Error> {
        let name = new_ref_name(name, "branch")?;
        if checkout {
            let mut args = vec!["switch", "-c", name];
            if let Some(target) = at {
                args.push(target);
            }
            git.run(self.root(), &args, None).map(drop)
        } else {
            let mut args = vec!["branch", name];
            if let Some(target) = at {
                args.push(target);
            }
            git.run(self.root(), &args, None).map(drop)
        }
    }

    /// Renames local branch `old` to `new`.
    ///
    /// # Errors
    /// [`Error::Refused`] for an empty name, or [`Error::Command`] with git's message.
    pub fn rename_branch(&self, git: &GitCli, old: &str, new: &str) -> Result<(), Error> {
        let new = new_ref_name(new, "branch")?;
        git.run(self.root(), &["branch", "-m", old, new], None)
            .map(drop)
    }

    /// Deletes local branch `name`, refusing to delete the current branch.
    ///
    /// # Errors
    /// [`Error::Refused`] when `name` is checked out, or [`Error::Command`] when
    /// `force` is false and the branch has unmerged commits.
    pub fn delete_branch(&self, git: &GitCli, name: &str, force: bool) -> Result<(), Error> {
        let repo = self.gix();
        if let Ok(Some(head)) = repo.head_name()
            && head.shorten() == name
        {
            return Err(Error::Refused(format!(
                "can't delete {name} while it is checked out"
            )));
        }
        let flag = if force { "-D" } else { "-d" };
        git.run(self.root(), &["branch", flag, name], None)
            .map(drop)
    }

    /// Sets or clears the upstream tracking branch of `branch`.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn set_upstream(
        &self,
        git: &GitCli,
        branch: &str,
        upstream: Option<&str>,
    ) -> Result<(), Error> {
        match upstream {
            Some(u) => {
                let flag = format!("--set-upstream-to={u}");
                git.run(self.root(), &["branch", &flag, branch], None)
                    .map(drop)
            }
            None => git
                .run(self.root(), &["branch", "--unset-upstream", branch], None)
                .map(drop),
        }
    }

    /// Creates local branch `local_name` tracking `remote_branch` and checks it out.
    ///
    /// # Errors
    /// [`Error::Refused`] for an empty name, or [`Error::Command`] with git's message.
    pub fn checkout_remote(
        &self,
        git: &GitCli,
        remote_branch: &str,
        local_name: &str,
    ) -> Result<(), Error> {
        let local_name = local_name.trim();
        if local_name.is_empty() {
            return Err(Error::Refused("branch name can't be empty".into()));
        }
        git.run(
            self.root(),
            &["switch", "-c", local_name, "--track", remote_branch],
            None,
        )
        .map(drop)
    }
}

/// Commits reachable from `tip` but not from `other`.
pub(crate) fn count_only_in(
    repo: &gix::Repository,
    tip: ObjectId,
    other: ObjectId,
) -> Result<u32, Error> {
    let walk = repo
        .rev_walk([tip])
        .with_hidden([other])
        .all()
        .map_err(read_err)?;
    let mut n = 0u32;
    for info in walk {
        info.map_err(read_err)?;
        n += 1;
    }
    Ok(n)
}
