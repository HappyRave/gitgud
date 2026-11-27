use std::fs;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, command_error, failure_message};
use crate::merge::Conflict;
use crate::repo::{Error, Repo, read_err};

/// What rebasing the current branch onto another would do, worked out with
/// `git merge-tree` step by step, without touching anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebasePlan {
    /// Commits that would be replayed, oldest first.
    pub commits: usize,
    /// Commits on the target the branch doesn't have yet.
    pub behind: usize,
    /// The first step that would stop on conflicts.
    pub conflict: Option<StepConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepConflict {
    /// 1-based.
    pub step: usize,
    pub short_id: String,
    pub subject: String,
    pub paths: Vec<String>,
}

/// A rebase stopped on conflicts (R1 state 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebaseState {
    /// The branch being rebased, short name.
    pub branch: String,
    /// What it's rebased onto: the name gitgud was given, else a short id.
    pub onto: String,
    /// 1-based step being applied, of `total`.
    pub step: usize,
    pub total: usize,
    /// Subject of the commit being applied.
    pub subject: String,
    pub conflicts: Vec<Conflict>,
    /// Stopped by an Edit step of a history plan: Continue amends the commit
    /// with what is staged.
    pub editing: bool,
}

/// The banner's three ways forward (R1 state 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RebaseStep {
    Continue,
    Skip,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RebaseOutcome {
    Done,
    Stopped,
}

/// What gitgud remembers about a rebase it runs: `.git/gitgud/rebase.json`.
#[derive(Debug, Serialize, Deserialize)]
struct Started {
    onto_oid: String,
    onto: String,
    /// The step `conflicts` belongs to.
    step: usize,
    conflicts: Vec<Conflict>,
}

impl Repo {
    /// Replays the commits `git rebase onto` would, with `merge-tree`, and reports
    /// the first conflicting step.
    ///
    /// # Errors
    /// [`Error::Command`] for an unknown `onto`.
    pub fn predict_rebase(&self, git: &GitCli, onto: &str) -> Result<RebasePlan, Error> {
        let root = self.root();
        let commits: Vec<String> = git
            .run(
                root,
                &[
                    "rev-list",
                    "--reverse",
                    "--no-merges",
                    "--cherry-pick",
                    "--right-only",
                    &format!("{onto}...HEAD"),
                ],
                None,
            )?
            .lines()
            .map(str::to_owned)
            .collect();
        let behind: usize = git
            .run(
                root,
                &["rev-list", "--count", &format!("HEAD..{onto}")],
                None,
            )?
            .trim()
            .parse()
            .map_err(read_err)?;
        let mut tree = git
            .run(root, &["rev-parse", &format!("{onto}^{{tree}}")], None)?
            .trim()
            .to_owned();
        for (i, commit) in commits.iter().enumerate() {
            let base = format!("--merge-base={commit}^");
            let args = [
                "merge-tree",
                "--write-tree",
                "--name-only",
                "--no-messages",
                "-z",
                base.as_str(),
                tree.as_str(),
                commit.as_str(),
            ];
            let (code, out, err) = git.run_status(root, &args, None)?;
            let mut records = out.split('\0').filter(|p| !p.is_empty());
            match code {
                0 => tree = records.next().unwrap_or_default().to_owned(),
                1 => {
                    let subject = git.run(root, &["log", "-1", "--format=%s", commit], None)?;
                    return Ok(RebasePlan {
                        commits: commits.len(),
                        behind,
                        conflict: Some(StepConflict {
                            step: i + 1,
                            short_id: commit[..7].to_owned(),
                            subject: subject.trim().to_owned(),
                            paths: records.skip(1).map(str::to_owned).collect(),
                        }),
                    });
                }
                _ => return Err(command_error(&args, failure_message(&out, &err))),
            }
        }
        Ok(RebasePlan {
            commits: commits.len(),
            behind,
            conflict: None,
        })
    }

    /// Files a rebase onto `onto` can rewrite: what either side changed since
    /// they split. The op log snapshots exactly these.
    ///
    /// # Errors
    /// [`Error::Command`] for an unknown `onto`.
    pub fn rebase_paths(&self, git: &GitCli, onto: &str) -> Result<Vec<String>, Error> {
        self.paths_between(git, "HEAD", onto)
    }

    /// As [`Repo::rebase_paths`], for the rebase in progress.
    ///
    /// # Errors
    /// [`Error::Refused`] when no rebase is in progress.
    pub fn rebase_paths_in_progress(&self, git: &GitCli) -> Result<Vec<String>, Error> {
        let orig = self.rebase_file("orig-head")?;
        let onto = self.rebase_file("onto")?;
        self.paths_between(git, &orig, &onto)
    }

    pub(crate) fn paths_between(
        &self,
        git: &GitCli,
        a: &str,
        b: &str,
    ) -> Result<Vec<String>, Error> {
        let mut paths = Vec::new();
        for range in [format!("{a}...{b}"), format!("{b}...{a}")] {
            let out = git.run(
                self.root(),
                &["diff", "--name-only", "--no-renames", "-z", &range],
                None,
            )?;
            paths.extend(out.split('\0').filter(|p| !p.is_empty()).map(str::to_owned));
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Rebases the current branch onto `onto`. Stops on the first conflict.
    ///
    /// # Errors
    /// [`Error::Refused`] while a merge or rebase is in progress; git's error
    /// otherwise (local changes in the way...).
    pub fn rebase(&self, git: &GitCli, onto: &str) -> Result<RebaseOutcome, Error> {
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
            &["rebase", "--no-autostash", onto],
            Some((onto, &onto_oid)),
            None,
        )
    }

    /// Continues the rebase in progress once every conflict is staged.
    ///
    /// # Errors
    /// [`Error::Refused`] while files are still conflicted; git's error (an empty
    /// commit asks for Skip).
    pub fn rebase_continue(&self, git: &GitCli) -> Result<RebaseOutcome, Error> {
        let left = self.conflicted_paths()?;
        if !left.is_empty() {
            return Err(Error::Refused(format!(
                "{} file(s) still conflicted",
                left.len()
            )));
        }
        self.run_rebase(git, &["rebase", "--continue"], None, None)
    }

    /// Drops the commit being applied and goes on with the next.
    ///
    /// # Errors
    /// git's error.
    pub fn rebase_skip(&self, git: &GitCli) -> Result<RebaseOutcome, Error> {
        self.run_rebase(git, &["rebase", "--skip"], None, None)
    }

    /// Gives up the rebase: the branch and work tree as before it started.
    ///
    /// # Errors
    /// git's error.
    pub fn rebase_abort(&self, git: &GitCli) -> Result<(), Error> {
        git.run(self.root(), &["rebase", "--abort"], None)?;
        self.forget_rebase()
    }

    /// The rebase in progress, if any.
    ///
    /// # Errors
    /// [`Error::Read`] when git's rebase files can't be read.
    pub fn rebase_state(&self, git: &GitCli) -> Result<Option<RebaseState>, Error> {
        if !self.rebasing() {
            return Ok(None);
        }
        let step: usize = self.rebase_file("msgnum")?.parse().map_err(read_err)?;
        let total: usize = self.rebase_file("end")?.parse().map_err(read_err)?;
        let onto_oid = self.rebase_file("onto")?;
        let head_name = self.rebase_file("head-name")?;
        let branch = head_name
            .strip_prefix("refs/heads/")
            .unwrap_or(&head_name)
            .to_owned();
        let subject = fs::read_to_string(self.git_dir().join("rebase-merge/message"))
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let started = self
            .rebase_record()
            .load::<Started>()?
            .filter(|s| s.onto_oid == onto_oid);
        let now = self.conflicted_paths()?;
        let mut conflicts = match &started {
            Some(s) if s.step == step => s.conflicts.clone(),
            _ => self.classify(git, &now)?,
        };
        for c in &mut conflicts {
            c.resolved = !now.contains(&c.path);
        }
        let editing = now.is_empty() && self.git_dir().join("rebase-merge/amend").exists();
        Ok(Some(RebaseState {
            branch,
            onto: started.map_or_else(|| onto_oid[..7].to_owned(), |s| s.onto),
            step,
            total,
            subject,
            conflicts,
            editing,
        }))
    }

    /// Runs a rebase command that may stop; remembers the conflicts of the step
    /// it stopped on, and forgets everything when it finished. `todo`, when
    /// given, replaces the todo list of an interactive rebase.
    pub(crate) fn run_rebase(
        &self,
        git: &GitCli,
        args: &[&str],
        onto: Option<(&str, &str)>,
        todo: Option<&str>,
    ) -> Result<RebaseOutcome, Error> {
        let mut cmd = git.command(self.root(), args);
        cmd.env("GIT_EDITOR", self.message_editor()?);
        if let Some(todo) = todo {
            cmd.env("GIT_SEQUENCE_EDITOR", self.todo_editor(todo)?);
        }
        let output = cmd
            .output()
            .map_err(|e| command_error(args, e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !self.rebasing() {
            self.forget_rebase()?;
            return if output.status.success() {
                Ok(RebaseOutcome::Done)
            } else {
                Err(command_error(args, failure_message(&stdout, &stderr)))
            };
        }
        let paths = self.conflicted_paths()?;
        if paths.is_empty() && !output.status.success() {
            // Stopped without a conflict (an empty commit...): say why, stay in progress.
            return Err(command_error(args, failure_message(&stdout, &stderr)));
        }
        let previous = self.rebase_record().load::<Started>()?;
        let (onto, onto_oid) = match (onto, &previous) {
            (Some((name, oid)), _) => (name.to_owned(), oid.to_owned()),
            (None, Some(s)) => (s.onto.clone(), s.onto_oid.clone()),
            (None, None) => {
                let oid = self.rebase_file("onto")?;
                (oid[..7].to_owned(), oid)
            }
        };
        let started = Started {
            onto_oid,
            onto,
            step: self.rebase_file("msgnum")?.parse().map_err(read_err)?,
            conflicts: self.classify(git, &paths)?,
        };
        self.rebase_record().save(&started)?;
        Ok(RebaseOutcome::Stopped)
    }

    /// An editor for the commit messages a rebase asks for: the message gitgud
    /// prepared for the step being applied (`gitgud/messages/<id>`), else the
    /// message as git offers it. Plain `sh`, no other tools.
    fn message_editor(&self) -> Result<String, Error> {
        let dir = self.gitgud_dir()?;
        let script = format!(
            "#!/bin/sh\n\
             c=\n\
             while read -r cmd id rest; do c=$id; done < {done}\n\
             [ -n \"$c\" ] || exit 0\n\
             for m in {messages}/\"$c\"*; do\n\
             \x20 [ -f \"$m\" ] || continue\n\
             \x20 while IFS= read -r line || [ -n \"$line\" ]; do printf '%s\\n' \"$line\"; done < \"$m\" > \"$1\"\n\
             done\n\
             exit 0\n",
            done = sh_quote(&self.git_dir().join("rebase-merge/done")),
            messages = sh_quote(&dir.join("messages")),
        );
        let path = dir.join("editor.sh");
        fs::write(&path, script).map_err(read_err)?;
        Ok(format!("sh {}", sh_quote(&path)))
    }

    /// A sequence editor that replaces git's todo list with `todo`.
    fn todo_editor(&self, todo: &str) -> Result<String, Error> {
        let dir = self.gitgud_dir()?;
        let file = dir.join("todo");
        fs::write(&file, todo).map_err(read_err)?;
        let script = format!(
            "#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done < {} > \"$1\"\n",
            sh_quote(&file)
        );
        let path = dir.join("sequence.sh");
        fs::write(&path, script).map_err(read_err)?;
        Ok(format!("sh {}", sh_quote(&path)))
    }

    pub(crate) fn gitgud_dir(&self) -> Result<std::path::PathBuf, Error> {
        let dir = self.git_dir().join("gitgud");
        fs::create_dir_all(&dir).map_err(read_err)?;
        Ok(dir)
    }

    pub(crate) fn rebasing(&self) -> bool {
        self.git_dir().join("rebase-merge").is_dir()
    }

    fn rebase_file(&self, name: &str) -> Result<String, Error> {
        match fs::read_to_string(self.git_dir().join("rebase-merge").join(name)) {
            Ok(s) => Ok(s.trim().to_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::Refused("no rebase is in progress".into()))
            }
            Err(e) => Err(read_err(e)),
        }
    }

    fn rebase_record(&self) -> crate::state_file::StateFile {
        crate::state_file::StateFile::new(self, "rebase.json")
    }

    fn forget_rebase(&self) -> Result<(), Error> {
        match fs::remove_dir_all(self.git_dir().join("gitgud/messages")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
            _ => {}
        }
        self.rebase_record().forget()
    }
}

/// `path` as a single-quoted `sh` word, with forward slashes.
fn sh_quote(path: &std::path::Path) -> String {
    let p = path.to_string_lossy().replace('\\', "/");
    format!("'{}'", p.replace('\'', r"'\''"))
}
