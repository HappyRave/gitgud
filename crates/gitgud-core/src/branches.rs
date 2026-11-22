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
        Ok(Branches { local, remotes })
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
        let target_tree = target
            .peel_to_commit()
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
}

/// Commits reachable from `tip` but not from `other`.
fn count_only_in(repo: &gix::Repository, tip: ObjectId, other: ObjectId) -> Result<u32, Error> {
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
