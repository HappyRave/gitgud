use std::collections::HashMap;

use gix::ObjectId;
use serde::Serialize;

use crate::branches::count_only_in;
use crate::cli::GitCli;
use crate::rebase::RebaseOutcome;
use crate::repo::{Error, Repo, read_err};

const TRUNK_CANDIDATES: &[&str] = &["main", "master", "develop", "trunk"];
const MAX_ANCESTRY_WALK: usize = 2048;

/// One branch level inside a stacked branch chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackLevel {
    pub branch: String,
    pub base: String,
    pub commits: u32,
    pub current: bool,
    pub needs_restack: bool,
    pub pr_url: Option<String>,
}

/// A chain of dependent branches rooted on the repository trunk branch (depth >= 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchStack {
    pub root: String,
    pub levels: Vec<StackLevel>,
}

struct BranchTip {
    name: String,
    tip: ObjectId,
    current: bool,
}

impl Repo {
    /// Detects stacked local branch chains (depth >= 2) rooted on the trunk branch.
    ///
    /// # Errors
    /// [`Error::Read`] when refs or commit history cannot be read.
    pub fn branch_stacks(&self) -> Result<Vec<BranchStack>, Error> {
        // Many history walks per pair of branches; asked after every refresh, while
        // the refs seldom move.
        let key = self.refs_key();
        let mut kept = self
            .stacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((k, stacks)) = kept.as_ref()
            && *k == key
        {
            return Ok(stacks.clone());
        }
        let stacks = self.find_stacks()?;
        *kept = Some((key, stacks.clone()));
        Ok(stacks)
    }

    fn find_stacks(&self) -> Result<Vec<BranchStack>, Error> {
        let gix_repo = self.gix();
        let current = gix_repo.head_name().map_err(read_err)?;
        let refs = gix_repo.references().map_err(read_err)?;

        let mut branches = Vec::new();
        for r in refs.local_branches().map_err(read_err)? {
            let r = r.map_err(|e| Error::Read(e.to_string()))?;
            let Some(tip) = r.target().try_id().map(ToOwned::to_owned) else {
                continue;
            };
            branches.push(BranchTip {
                name: r.name().shorten().to_string(),
                tip,
                current: current.as_ref().is_some_and(|c| c.as_ref() == r.name()),
            });
        }

        let Some(trunk_idx) = find_trunk_index(&gix_repo, &branches) else {
            return Ok(Vec::new());
        };
        let trunk = &branches[trunk_idx];

        let mut active: Vec<&BranchTip> = Vec::new();
        for (idx, b) in branches.iter().enumerate() {
            if idx == trunk_idx || b.tip == trunk.tip {
                continue;
            }
            if shares_history_above_trunk(&gix_repo, b.tip, trunk.tip)? {
                active.push(b);
            }
        }
        if active.len() < 2 {
            return Ok(Vec::new());
        }

        let by_name: HashMap<&str, &BranchTip> =
            active.iter().map(|b| (b.name.as_str(), *b)).collect();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();

        for b in &active {
            let mut best_parent: Option<(&str, u32)> = None;
            for p in &active {
                if p.name == b.name || p.tip == b.tip {
                    continue;
                }
                if is_ancestor(&gix_repo, p.tip, b.tip)? {
                    let dist = count_only_in(&gix_repo, b.tip, p.tip)?;
                    if dist > 0 && best_parent.is_none_or(|(_, best)| dist < best) {
                        best_parent = Some((p.name.as_str(), dist));
                    }
                }
            }
            let parent_name = best_parent.map_or(trunk.name.as_str(), |(name, _)| name);
            children
                .entry(parent_name.to_owned())
                .or_default()
                .push(b.name.clone());
        }
        for list in children.values_mut() {
            list.sort();
        }

        let mut chains: Vec<Vec<(String, String)>> = Vec::new();
        let mut path: Vec<(String, String)> = Vec::new();
        collect_chains(&trunk.name, &children, &mut path, &mut chains);

        let mut stacks = Vec::new();
        for chain in chains {
            if chain.len() < 2 {
                continue;
            }
            let mut levels = Vec::with_capacity(chain.len());
            let mut prev_needs_restack = false;
            for (branch_name, base_name) in chain {
                let b_tip = by_name[branch_name.as_str()];
                let base_tip = if base_name == trunk.name {
                    trunk.tip
                } else {
                    by_name[base_name.as_str()].tip
                };
                let commits = count_only_in(&gix_repo, b_tip.tip, base_tip)?;
                let direct_stale = !is_ancestor(&gix_repo, base_tip, b_tip.tip)?;
                let needs_restack = direct_stale || prev_needs_restack;
                prev_needs_restack = needs_restack;
                let pr_url = self.pr_url_with_base(&branch_name, Some(&base_name))?;
                levels.push(StackLevel {
                    branch: branch_name,
                    base: base_name,
                    commits,
                    current: b_tip.current,
                    needs_restack,
                    pr_url,
                });
            }
            stacks.push(BranchStack {
                root: trunk.name.clone(),
                levels,
            });
        }
        Ok(stacks)
    }

    /// Files `restack` of `top_branch` onto `onto` can modify.
    ///
    /// # Errors
    /// [`Error::Command`] when git fails to diff refs.
    pub fn restack_paths(
        &self,
        git: &GitCli,
        top_branch: &str,
        onto: &str,
    ) -> Result<Vec<String>, Error> {
        let mut paths = self.paths_between(git, "HEAD", top_branch)?;
        paths.extend(self.paths_between(git, top_branch, onto)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Rebases a stacked branch chain ending at `top_branch` onto `onto` with `--update-refs`.
    ///
    /// # Errors
    /// [`Error::Refused`] if an operation is already in progress, or [`Error::Command`] on failure.
    pub fn restack(
        &self,
        git: &GitCli,
        top_branch: &str,
        onto: &str,
    ) -> Result<RebaseOutcome, Error> {
        self.ensure_idle()?;
        let onto_oid = git
            .run(
                self.root(),
                &["rev-parse", "--verify", &format!("{onto}^{{commit}}")],
                None,
            )?
            .trim()
            .to_owned();
        self.run_rebase(
            git,
            &[
                "rebase",
                "--update-refs",
                "--no-autostash",
                onto,
                top_branch,
            ],
            Some((onto, &onto_oid)),
            None,
        )
    }
}

fn find_trunk_index(repo: &gix::Repository, branches: &[BranchTip]) -> Option<usize> {
    if let Ok(Some(origin_head)) = repo.try_find_reference("refs/remotes/origin/HEAD")
        && let gix::refs::TargetRef::Symbolic(target) = origin_head.target()
        && let Some(short) = target
            .as_bstr()
            .to_string()
            .strip_prefix("refs/remotes/origin/")
        && let Some(idx) = branches.iter().position(|b| b.name == short)
    {
        return Some(idx);
    }
    for candidate in TRUNK_CANDIDATES {
        if let Some(idx) = branches.iter().position(|b| b.name == *candidate) {
            return Some(idx);
        }
    }
    None
}

fn is_ancestor(repo: &gix::Repository, anc: ObjectId, desc: ObjectId) -> Result<bool, Error> {
    Ok(count_only_in(repo, anc, desc)? == 0)
}

fn shares_history_above_trunk(
    repo: &gix::Repository,
    branch_tip: ObjectId,
    trunk_tip: ObjectId,
) -> Result<bool, Error> {
    let walk = repo
        .rev_walk([branch_tip])
        .with_hidden([trunk_tip])
        .all()
        .map_err(read_err)?;
    let mut visited = 0usize;
    for info in walk.take(MAX_ANCESTRY_WALK) {
        let info = info.map_err(read_err)?;
        let commit = repo.find_commit(info.id).map_err(read_err)?;
        if commit.parent_ids().next().is_none() {
            return Ok(false);
        }
        visited += 1;
    }
    Ok(visited > 0)
}

fn collect_chains(
    parent: &str,
    children: &HashMap<String, Vec<String>>,
    path: &mut Vec<(String, String)>,
    out: &mut Vec<Vec<(String, String)>>,
) {
    let Some(kids) = children.get(parent) else {
        if !path.is_empty() {
            out.push(path.clone());
        }
        return;
    };
    if kids.is_empty() {
        if !path.is_empty() {
            out.push(path.clone());
        }
        return;
    }
    for kid in kids {
        path.push((kid.clone(), parent.to_owned()));
        collect_chains(kid, children, path, out);
        path.pop();
    }
}
