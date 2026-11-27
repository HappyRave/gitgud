use std::collections::HashSet;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, command_error, failure_message};
use crate::graph::Ghost;
use crate::rebase::RebaseOutcome;
use crate::repo::{Error, Repo, read_err};

/// What a history plan does with one commit (spec §3.4, the todo-list verbs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

impl Action {
    fn verb(self) -> &'static str {
        match self {
            Action::Pick => "pick",
            Action::Reword => "reword",
            Action::Edit => "edit",
            Action::Squash => "squash",
            Action::Fixup => "fixup",
            Action::Drop => "drop",
        }
    }

    /// Folds into the commit before it.
    fn folds(self) -> bool {
        matches!(self, Action::Squash | Action::Fixup)
    }
}

/// A commit as the plan lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCommit {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    /// The full message.
    pub message: String,
    pub author: String,
    /// Author time, seconds since the Unix epoch.
    pub time: i64,
    /// What the plan starts with: pick, or fixup / squash for `fixup!` / `squash!` commits.
    pub action: Action,
}

/// A history plan as it opens: the commits from one up to HEAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPlan {
    /// The current branch, short name.
    pub branch: String,
    /// The parent of the oldest commit: the plan replays onto it unless moved.
    pub base: String,
    pub base_short: String,
    pub base_subject: String,
    /// Newest first, `fixup!` / `squash!` commits already moved under their target.
    pub commits: Vec<PlanCommit>,
    /// Other local branches pointing at one of the commits (`--update-refs`).
    pub dependents: Vec<String>,
    /// The branch's upstream when it has some of these commits: rewriting them
    /// means a force-push.
    pub pushed: Option<String>,
}

/// One row of the plan as the user left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub action: Action,
    /// The new message: of a reword, or of a squash chain (its newest squash wins).
    #[serde(default)]
    pub message: Option<String>,
}

/// What `merge-tree` says one step would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    Clean,
    /// Clean, but only once an earlier conflicting step is resolved.
    #[serde(rename_all = "camelCase")]
    AfterConflict {
        step: usize,
    },
    /// `tree` holds the predicted files, with conflict markers.
    Conflict {
        paths: Vec<String>,
        tree: String,
    },
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPrediction {
    pub id: String,
    /// 1-based, oldest first, as git numbers them; `None` for a drop.
    pub step: Option<usize>,
    pub outcome: Outcome,
}

/// What a plan would do, worked out without touching anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPrediction {
    /// Newest first, like the plan.
    pub steps: Vec<StepPrediction>,
    /// Commits now and after.
    pub before: usize,
    pub after: usize,
    /// Files the result differs in from HEAD; `None` when a conflict leaves it open.
    pub changed: Option<usize>,
}

impl PlanPrediction {
    /// The first step predicted to conflict, oldest first.
    pub fn first_conflict(&self) -> Option<&StepPrediction> {
        self.steps
            .iter()
            .rev()
            .find(|s| matches!(s.outcome, Outcome::Conflict { .. }))
    }
}

impl Repo {
    /// The plan for editing the current branch from commit `from` (included) up
    /// to HEAD.
    ///
    /// # Errors
    /// [`Error::Refused`] on a detached HEAD, during a merge or rebase, when
    /// `from` isn't on the current branch, is the first commit, or the range
    /// holds a merge commit.
    pub fn history_plan(&self, git: &GitCli, from: &str) -> Result<HistoryPlan, Error> {
        let branch = self.ensure_can_edit()?;
        let root = self.root();
        let from = self.commit_id(git, from)?;
        if git
            .run_status(root, &["merge-base", "--is-ancestor", &from, "HEAD"], None)?
            .0
            != 0
        {
            return Err(Error::Refused(format!(
                "{} isn't on {branch}: check out its branch to edit it",
                &from[..7]
            )));
        }
        let base = self
            .commit_id(git, &format!("{from}^"))
            .map_err(|_| Error::Refused("the first commit can't be edited yet".into()))?;
        let commits = self.range_commits(git, &base)?;
        let (base_short, base_subject) = {
            let out = git.run(root, &["log", "-1", "--format=%h%x00%s", &base], None)?;
            let (h, s) = out.trim_end().split_once('\0').unwrap_or_default();
            (h.to_owned(), s.to_owned())
        };
        let ids: HashSet<&str> = commits.iter().map(|c| c.id.as_str()).collect();
        let mut dependents = Vec::new();
        let heads = git.run(
            root,
            &[
                "for-each-ref",
                "--format=%(objectname) %(refname:short) %(HEAD)",
                "refs/heads",
            ],
            None,
        )?;
        for line in heads.lines() {
            let mut parts = line.splitn(3, ' ');
            let (id, name, head) = (
                parts.next().unwrap_or_default(),
                parts.next().unwrap_or_default(),
                parts.next().unwrap_or_default(),
            );
            if head != "*" && ids.contains(id) {
                dependents.push(name.to_owned());
            }
        }
        let pushed = match git.run_status(
            root,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
            None,
        )? {
            (0, up, _) => {
                let up = up.trim().to_owned();
                let has = git
                    .run_status(root, &["merge-base", "--is-ancestor", &from, &up], None)?
                    .0
                    == 0;
                has.then_some(up)
            }
            _ => None,
        };
        Ok(HistoryPlan {
            branch,
            base,
            base_short,
            base_subject,
            commits: autosquash(commits),
            dependents,
            pushed,
        })
    }

    /// Replays `steps` onto `onto` with `merge-tree`, step by step, and says
    /// which would conflict.
    ///
    /// # Errors
    /// [`Error::Refused`] for a plan that doesn't match `base..HEAD` or can't
    /// run (a squash with nothing to fold into); git's error for a bad `onto`.
    pub fn predict_plan(
        &self,
        git: &GitCli,
        onto: &str,
        base: &str,
        steps: &[Step],
    ) -> Result<PlanPrediction, Error> {
        self.check_steps(git, base, steps)?;
        let root = self.root();
        let mut tree = git
            .run(root, &["rev-parse", &format!("{onto}^{{tree}}")], None)?
            .trim()
            .to_owned();
        let mut predicted = Vec::with_capacity(steps.len());
        let mut number = 0;
        let mut conflicted: Option<usize> = None;
        for step in steps.iter().rev() {
            if step.action == Action::Drop {
                predicted.push(StepPrediction {
                    id: step.id.clone(),
                    step: None,
                    outcome: Outcome::Dropped,
                });
                continue;
            }
            number += 1;
            let merge_base = format!("--merge-base={}^", step.id);
            let args = [
                "merge-tree",
                "--write-tree",
                "--name-only",
                "--no-messages",
                "-z",
                merge_base.as_str(),
                tree.as_str(),
                step.id.as_str(),
            ];
            let (code, out, err) = git.run_status(root, &args, None)?;
            let mut records = out.split('\0').filter(|p| !p.is_empty());
            let next = records.next().unwrap_or_default().to_owned();
            let outcome = match (code, conflicted) {
                (0, None) => Outcome::Clean,
                (0, Some(step)) => Outcome::AfterConflict { step },
                (1, _) => {
                    conflicted.get_or_insert(number);
                    Outcome::Conflict {
                        paths: records.map(str::to_owned).collect(),
                        tree: next.clone(),
                    }
                }
                _ => return Err(command_error(&args, failure_message(&out, &err))),
            };
            tree = next;
            predicted.push(StepPrediction {
                id: step.id.clone(),
                step: Some(number),
                outcome,
            });
        }
        predicted.reverse();
        let changed = match conflicted {
            Some(_) => None,
            None => Some(
                git.run(
                    root,
                    &["diff-tree", "-r", "--name-only", "-z", "HEAD", &tree],
                    None,
                )?
                .split('\0')
                .filter(|p| !p.is_empty())
                .count(),
            ),
        };
        Ok(PlanPrediction {
            steps: predicted,
            before: steps.len(),
            after: steps
                .iter()
                .filter(|s| !s.action.folds() && s.action != Action::Drop)
                .count(),
            changed,
        })
    }

    /// The text of `path` in a predicted `tree`, conflict markers included.
    ///
    /// # Errors
    /// git's error when the tree has no such file.
    pub fn predicted_file(&self, git: &GitCli, tree: &str, path: &str) -> Result<String, Error> {
        git.run(
            self.root(),
            &["cat-file", "blob", &format!("{tree}:{path}")],
            None,
        )
    }

    /// The rows a plan preview draws, newest first: one per commit the plan
    /// leaves, named after its new message, tagged with what happened to it.
    ///
    /// # Errors
    /// [`Error::Read`] when a commit can't be read.
    pub fn plan_ghosts(
        &self,
        steps: &[Step],
        prediction: &PlanPrediction,
    ) -> Result<Vec<Ghost>, Error> {
        let repo = self.gix();
        let mut ghosts = Vec::new();
        for chain in chains(steps) {
            let head = &steps[chain.head];
            let id = gix::ObjectId::from_hex(head.id.as_bytes()).map_err(read_err)?;
            let commit = repo.find_commit(id).map_err(read_err)?;
            let original = commit.message().map_err(read_err)?.summary().to_string();
            let message = chain.message(steps);
            let mut tags = Vec::new();
            if head.action == Action::Reword || message.is_some_and(|m| first_line(m) != original) {
                tags.push("reworded".to_owned());
            }
            let folded = chain.folded.len();
            if folded > 0 {
                let squash = chain
                    .folded
                    .iter()
                    .any(|&i| steps[i].action == Action::Squash);
                tags.push(match (folded, squash) {
                    (1, false) => "+ fixup folded in".to_owned(),
                    (1, true) => "+ squash".to_owned(),
                    (n, _) => format!("+ {n} folded in"),
                });
            }
            if head.action == Action::Edit {
                tags.push("stops to edit".to_owned());
            }
            let conflict = prediction.steps.iter().any(|p| {
                (p.id == head.id || chain.folded.iter().any(|&i| steps[i].id == p.id))
                    && matches!(p.outcome, Outcome::Conflict { .. })
            });
            if conflict {
                tags.push("⚠ conflict".to_owned());
            }
            ghosts.push(Ghost {
                subject: message.map_or(original, |m| first_line(m).to_owned()),
                tag: tags.join(", "),
            });
        }
        ghosts.reverse();
        Ok(ghosts)
    }

    /// Runs the plan as `git rebase -i` with a generated todo list. Stops on a
    /// conflict or an Edit step (see [`Repo::rebase_state`]).
    ///
    /// # Errors
    /// As [`Repo::predict_plan`]; [`Error::Refused`] during a merge or rebase;
    /// git's error (local changes in the way...).
    pub fn run_plan(
        &self,
        git: &GitCli,
        onto: &str,
        base: &str,
        steps: &[Step],
        update_refs: bool,
    ) -> Result<RebaseOutcome, Error> {
        self.ensure_can_edit()?;
        self.check_steps(git, base, steps)?;
        let onto_oid = self.commit_id(git, onto)?;
        let dependents = if update_refs {
            self.dependents(git, steps)?
        } else {
            Vec::new()
        };
        let todo = todo(steps, &dependents);
        self.write_messages(steps)?;
        let refs = if update_refs {
            "--update-refs"
        } else {
            "--no-update-refs"
        };
        self.run_rebase(
            git,
            &[
                "rebase",
                "-i",
                "--no-autostash",
                "--no-autosquash",
                "--empty=drop",
                refs,
                "--onto",
                &onto_oid,
                base,
            ],
            Some((onto, &onto_oid)),
            Some(&todo),
        )
    }

    /// Rewords the HEAD commit: `commit --amend --only`, so the index stays out
    /// of it. Hooks run.
    ///
    /// # Errors
    /// [`Error::Refused`] during a merge or rebase; git's error (empty message, a hook).
    pub fn reword_head(&self, git: &GitCli, message: &str) -> Result<(), Error> {
        self.ensure_can_edit()?;
        git.run(
            self.root(),
            &[
                "commit",
                "-q",
                "--amend",
                "--only",
                "--allow-empty",
                "--file=-",
            ],
            Some(message.as_bytes()),
        )
        .map(drop)
    }

    /// The base and steps that fold `commit` into its parent (Fixup) or drop it,
    /// leaving every other commit up to HEAD as it is.
    ///
    /// # Errors
    /// As [`Repo::history_plan`]; [`Error::Refused`] for another action, or a
    /// fixup into the first commit.
    pub fn quick_steps(
        &self,
        git: &GitCli,
        commit: &str,
        action: Action,
    ) -> Result<(String, Vec<Step>), Error> {
        if !matches!(action, Action::Fixup | Action::Drop) {
            return Err(Error::Refused(
                "only fixup and drop apply at once; use the plan".into(),
            ));
        }
        let id = self.commit_id(git, commit)?;
        let from = if action == Action::Fixup {
            format!("{id}^")
        } else {
            id.clone()
        };
        let plan = self.history_plan(git, &from)?;
        let steps = plan
            .commits
            .iter()
            .map(|c| Step {
                id: c.id.clone(),
                action: if c.id == id { action } else { Action::Pick },
                message: None,
            })
            .collect::<Vec<_>>();
        // The plan may have moved a fixup! commit; a quick edit keeps the order.
        let order = self.range_commits(git, &plan.base)?;
        let steps = order
            .iter()
            .map(|c| {
                steps
                    .iter()
                    .find(|s| s.id == c.id)
                    .cloned()
                    .expect("same commits")
            })
            .collect();
        Ok((plan.base, steps))
    }

    /// The op-log label of a plan: `Rebase feature · reword 1, fixup 1`.
    pub fn plan_label(branch: &str, onto: Option<&str>, steps: &[Step]) -> String {
        let mut counts: Vec<(Action, usize)> = Vec::new();
        for s in steps.iter().filter(|s| s.action != Action::Pick) {
            match counts.iter_mut().find(|(a, _)| *a == s.action) {
                Some((_, n)) => *n += 1,
                None => counts.push((s.action, 1)),
            }
        }
        let what = counts
            .iter()
            .map(|(a, n)| format!("{} {n}", a.verb()))
            .collect::<Vec<_>>()
            .join(", ");
        let onto = onto.map(|o| format!(" onto {o}")).unwrap_or_default();
        match what.is_empty() {
            true => format!("Rebase {branch}{onto}"),
            false => format!("Rebase {branch}{onto} · {what}"),
        }
    }

    /// Whether revisions `a` and `b` name the same commit.
    ///
    /// # Errors
    /// git's error for an unknown revision.
    pub fn resolves_to(&self, git: &GitCli, a: &str, b: &str) -> Result<bool, Error> {
        Ok(self.commit_id(git, a)? == self.commit_id(git, b)?)
    }

    /// The subject of `commit`.
    ///
    /// # Errors
    /// git's error for an unknown commit.
    pub fn subject(&self, git: &GitCli, commit: &str) -> Result<String, Error> {
        Ok(git
            .run(self.root(), &["log", "-1", "--format=%s", commit], None)?
            .trim_end()
            .to_owned())
    }

    /// The current branch, if history can be edited now.
    pub(crate) fn ensure_can_edit(&self) -> Result<String, Error> {
        self.ensure_idle()?;
        let repo = self.gix();
        let head = repo.head().map_err(read_err)?;
        match head.referent_name() {
            Some(name) if !head.is_unborn() => Ok(name.shorten().to_string()),
            _ => Err(Error::Refused(
                "check out a branch to edit its history".into(),
            )),
        }
    }

    fn commit_id(&self, git: &GitCli, rev: &str) -> Result<String, Error> {
        Ok(git
            .run(
                self.root(),
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{rev}^{{commit}}"),
                ],
                None,
            )?
            .trim()
            .to_owned())
    }

    /// `base..HEAD`, newest first, refused if it holds a merge.
    fn range_commits(&self, git: &GitCli, base: &str) -> Result<Vec<PlanCommit>, Error> {
        let out = git.run(
            self.root(),
            &[
                "log",
                "-z",
                "--topo-order",
                "--format=%H%x00%h%x00%an%x00%at%x00%P%x00%B",
                &format!("{base}..HEAD"),
            ],
            None,
        )?;
        let fields: Vec<&str> = out.split('\0').collect();
        let mut commits = Vec::new();
        for c in fields.chunks(6).filter(|c| c.len() == 6) {
            if c[4].split(' ').count() > 1 {
                return Err(Error::Refused(
                    "history with merge commits can't be edited yet".into(),
                ));
            }
            let message = c[5].trim_end().to_owned();
            commits.push(PlanCommit {
                id: c[0].trim_start_matches('\n').to_owned(),
                short_id: c[1].to_owned(),
                subject: first_line(&message).to_owned(),
                message,
                author: c[2].to_owned(),
                time: c[3].parse().map_err(read_err)?,
                action: Action::Pick,
            });
        }
        Ok(commits)
    }

    /// Refuses steps that aren't exactly `base..HEAD`, or that git couldn't run.
    fn check_steps(&self, git: &GitCli, base: &str, steps: &[Step]) -> Result<(), Error> {
        let range = self.range_commits(git, base)?;
        let want: HashSet<&str> = range.iter().map(|c| c.id.as_str()).collect();
        let got: HashSet<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        if want != got || got.len() != steps.len() {
            return Err(Error::Refused(
                "the plan no longer matches the branch: reopen it".into(),
            ));
        }
        match steps.iter().rev().find(|s| s.action != Action::Drop) {
            Some(oldest) if oldest.action.folds() => Err(Error::Refused(
                "the oldest commit has nothing to squash or fix up into".into(),
            )),
            _ => Ok(()),
        }
    }

    /// Other local branches whose tip is one of the plan's commits: full ref and tip.
    fn dependents(&self, git: &GitCli, steps: &[Step]) -> Result<Vec<(String, String)>, Error> {
        let out = git.run(
            self.root(),
            &[
                "for-each-ref",
                "--format=%(objectname) %(refname) %(HEAD)",
                "refs/heads",
            ],
            None,
        )?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut p = l.splitn(3, ' ');
                let (id, name, head) = (p.next()?, p.next()?, p.next()?);
                (head != "*" && steps.iter().any(|s| s.id == id))
                    .then(|| (name.to_owned(), id.to_owned()))
            })
            .collect())
    }

    /// The messages the editor script hands git: `gitgud/messages/<id>`, by the
    /// step at which git asks (the reword, or the last step of a squash chain).
    fn write_messages(&self, steps: &[Step]) -> Result<(), Error> {
        let dir = self.gitgud_dir()?.join("messages");
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
            _ => {}
        }
        fs::create_dir_all(&dir).map_err(read_err)?;
        for chain in chains(steps) {
            let head = &steps[chain.head];
            if let (Action::Reword, Some(m)) = (head.action, &head.message) {
                fs::write(dir.join(&head.id), with_newline(m)).map_err(read_err)?;
            }
            let squash = chain
                .folded
                .iter()
                .any(|&i| steps[i].action == Action::Squash);
            let squash_message = chain
                .folded
                .iter()
                .rev()
                .find_map(|&i| steps[i].message.as_ref());
            if let (true, Some(m), Some(&last)) = (squash, squash_message, chain.folded.last()) {
                fs::write(dir.join(&steps[last].id), with_newline(m)).map_err(read_err)?;
            }
        }
        Ok(())
    }
}

/// A commit the plan keeps and the steps folded into it, as indexes into the
/// (newest-first) steps; `folded` oldest first.
struct Chain {
    head: usize,
    folded: Vec<usize>,
}

impl Chain {
    /// The message the result gets, if the plan sets one.
    fn message<'a>(&self, steps: &'a [Step]) -> Option<&'a str> {
        let squash = self
            .folded
            .iter()
            .rev()
            .find(|&&i| steps[i].action == Action::Squash)
            .and_then(|&i| steps[i].message.as_deref());
        let head = &steps[self.head];
        squash.or(match head.action {
            Action::Reword => head.message.as_deref(),
            _ => None,
        })
    }
}

/// The plan's kept commits, oldest first.
fn chains(steps: &[Step]) -> Vec<Chain> {
    let mut chains: Vec<Chain> = Vec::new();
    for (i, step) in steps.iter().enumerate().rev() {
        match step.action {
            Action::Drop => {}
            a if a.folds() => {
                if let Some(c) = chains.last_mut() {
                    c.folded.push(i);
                }
            }
            _ => chains.push(Chain {
                head: i,
                folded: Vec::new(),
            }),
        }
    }
    chains
}

/// The todo list for `steps` (newest first): oldest first, with an
/// `update-ref` after the result each dependent branch points at.
fn todo(steps: &[Step], dependents: &[(String, String)]) -> String {
    let oldest_first: Vec<&Step> = steps.iter().rev().collect();
    let mut out = String::new();
    for (i, step) in oldest_first.iter().enumerate() {
        out.push_str(&format!("{} {}\n", step.action.verb(), step.id));
        let chain_ends = oldest_first
            .get(i + 1)
            .is_none_or(|next| !next.action.folds());
        if !chain_ends {
            continue;
        }
        // This step closes its chain: branches on any commit of the chain move here.
        let mut start = i;
        while start > 0 && oldest_first[start].action.folds() {
            start -= 1;
        }
        for s in &oldest_first[start..=i] {
            for (name, _) in dependents.iter().filter(|(_, tip)| *tip == s.id) {
                out.push_str(&format!("update-ref {name}\n"));
            }
        }
    }
    out
}

/// Applies `fixup!` / `squash!` commits as git's `--autosquash` does: each
/// moves just above (after) its target and gets the matching action. Takes and
/// returns newest first.
fn autosquash(mut newest_first: Vec<PlanCommit>) -> Vec<PlanCommit> {
    newest_first.reverse();
    let commits = newest_first;
    let mut order: Vec<usize> = Vec::with_capacity(commits.len());
    let mut actions = vec![Action::Pick; commits.len()];
    for (i, c) in commits.iter().enumerate() {
        let target = squash_target(&c.subject).and_then(|(action, wanted)| {
            order
                .iter()
                .position(|&j| {
                    commits[j].subject == wanted
                        || (wanted.len() >= 4 && commits[j].id.starts_with(wanted))
                })
                .or_else(|| {
                    order
                        .iter()
                        .position(|&j| commits[j].subject.starts_with(wanted))
                })
                .map(|pos| (action, pos))
        });
        match target {
            Some((action, pos)) => {
                let mut at = pos + 1;
                while at < order.len() && actions[order[at]].folds() {
                    at += 1;
                }
                order.insert(at, i);
                actions[i] = action;
            }
            None => order.push(i),
        }
    }
    order
        .into_iter()
        .rev()
        .map(|i| PlanCommit {
            action: actions[i],
            ..commits[i].clone()
        })
        .collect()
}

/// `fixup! X` / `squash! X` (repeated prefixes allowed): the action and X.
fn squash_target(subject: &str) -> Option<(Action, &str)> {
    let (action, mut rest) = match subject.strip_prefix("fixup! ") {
        Some(r) => (Action::Fixup, r),
        None => (Action::Squash, subject.strip_prefix("squash! ")?),
    };
    while let Some(r) = rest
        .strip_prefix("fixup! ")
        .or_else(|| rest.strip_prefix("squash! "))
    {
        rest = r;
    }
    Some((action, rest))
}

fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
}

fn with_newline(message: &str) -> String {
    format!("{}\n", message.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, subject: &str) -> PlanCommit {
        PlanCommit {
            id: id.into(),
            short_id: id.into(),
            subject: subject.into(),
            message: subject.into(),
            author: String::new(),
            time: 0,
            action: Action::Pick,
        }
    }

    fn shape(commits: &[PlanCommit]) -> Vec<(&str, Action)> {
        commits.iter().map(|c| (c.id.as_str(), c.action)).collect()
    }

    fn step(id: &str, action: Action) -> Step {
        Step {
            id: id.into(),
            action,
            message: None,
        }
    }

    #[test]
    fn autosquash_moves_fixups_above_their_target() {
        // Newest first, as git log lists them.
        let plan = autosquash(vec![
            commit("d", "fixup! a"),
            commit("c", "squash! b"),
            commit("b", "b"),
            commit("a", "a"),
        ]);
        assert_eq!(
            shape(&plan),
            [
                ("c", Action::Squash),
                ("b", Action::Pick),
                ("d", Action::Fixup),
                ("a", Action::Pick),
            ]
        );
    }

    #[test]
    fn autosquash_keeps_fixup_order_and_matches_prefixes_and_ids() {
        let plan = autosquash(vec![
            commit("e", "fixup! fixup! Add thing"),
            commit("d", "fixup! 1234abcd"),
            commit("c", "fixup! Add"),
            commit("b", "other"),
            commit("1234abcdef", "Add thing"),
        ]);
        assert_eq!(
            shape(&plan),
            [
                ("b", Action::Pick),
                ("e", Action::Fixup),
                ("d", Action::Fixup),
                ("c", Action::Fixup),
                ("1234abcdef", Action::Pick),
            ]
        );
    }

    #[test]
    fn autosquash_leaves_fixups_without_a_target_alone() {
        let plan = autosquash(vec![commit("b", "fixup! nowhere"), commit("a", "a")]);
        assert_eq!(shape(&plan), [("b", Action::Pick), ("a", Action::Pick)]);
    }

    #[test]
    fn todo_is_oldest_first_with_update_refs_after_each_chain() {
        let steps = [
            step("d", Action::Pick),
            step("c", Action::Fixup),
            step("b", Action::Reword),
            step("a", Action::Drop),
        ];
        let deps = [
            ("refs/heads/x".to_owned(), "c".to_owned()),
            ("refs/heads/y".to_owned(), "a".to_owned()),
        ];
        assert_eq!(
            todo(&steps, &deps),
            "drop a\nupdate-ref refs/heads/y\nreword b\nfixup c\nupdate-ref refs/heads/x\npick d\n"
        );
    }

    #[test]
    fn chains_group_folds_under_the_commit_below_them() {
        let steps = [
            step("e", Action::Squash),
            step("d", Action::Drop),
            step("c", Action::Fixup),
            step("b", Action::Pick),
            step("a", Action::Pick),
        ];
        let c = chains(&steps);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].head, c[0].folded.as_slice()), (4, &[][..]));
        assert_eq!((c[1].head, c[1].folded.as_slice()), (3, &[2, 0][..]));
    }

    #[test]
    fn labels_count_each_action() {
        let steps = [
            step("c", Action::Fixup),
            step("b", Action::Reword),
            step("a", Action::Pick),
        ];
        assert_eq!(
            Repo::plan_label("feature", None, &steps),
            "Rebase feature · fixup 1, reword 1"
        );
        assert_eq!(
            Repo::plan_label("f", Some("main"), &[step("a", Action::Pick)]),
            "Rebase f onto main"
        );
    }
}
