use std::fs;
use std::path::PathBuf;

use gix::ObjectId;
use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, command_error, failure_message};
use crate::merge::Conflict;
use crate::repo::{Error, Repo, read_err};

/// Which single-commit sequencer operation is stopped on conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequencerKind {
    CherryPick,
    Revert,
}

/// Outcome of starting a cherry-pick or revert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SequencerOutcome {
    Done,
    Conflicts,
}

/// A cherry-pick or revert stopped on conflicts and waiting for Continue / Abort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencerState {
    pub kind: SequencerKind,
    pub commit: String,
    pub short_id: String,
    pub subject: String,
    pub message: String,
    pub conflicts: Vec<Conflict>,
}

/// Banner actions while a cherry-pick or revert is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequencerStep {
    Continue,
    Abort,
}

#[derive(Debug, Serialize, Deserialize)]
struct Started {
    kind: SequencerKind,
    head_oid: String,
    short_id: String,
    subject: String,
    conflicts: Vec<Conflict>,
}

impl Repo {
    /// Files cherry-picking `commit` into HEAD would leave conflicted, touching nothing.
    ///
    /// # Errors
    /// [`Error::Refused`] for a merge commit, or [`Error::Command`] on git failure.
    pub fn predict_cherry_pick(&self, git: &GitCli, commit: &str) -> Result<Vec<String>, Error> {
        let (oid, base) = self.commit_and_parent_tree(git, commit)?;
        self.predict_three_way(git, &base, "HEAD", &oid)
    }

    /// Files reverting `commit` against HEAD would leave conflicted, touching nothing.
    ///
    /// # Errors
    /// [`Error::Refused`] for a merge commit, or [`Error::Command`] on git failure.
    pub fn predict_revert(&self, git: &GitCli, commit: &str) -> Result<Vec<String>, Error> {
        let (oid, parent_tree) = self.commit_and_parent_tree(git, commit)?;
        self.predict_three_way(git, &oid, "HEAD", &parent_tree)
    }

    /// Files `commit` changes (relative to its parent, or empty tree for a root commit).
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn sequencer_paths(&self, git: &GitCli, commit: &str) -> Result<Vec<String>, Error> {
        let out = git.run(
            self.root(),
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--name-only",
                "-r",
                "-z",
                commit,
            ],
            None,
        )?;
        let mut paths: Vec<String> = out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Cherry-picks `commit` onto HEAD. Clean picks commit at once; conflicted
    /// ones stop (see [`Repo::sequencer_state`]).
    ///
    /// # Errors
    /// [`Error::Refused`] when an operation is already in progress or `commit`
    /// is a merge commit; [`Error::Command`] with git's error.
    pub fn cherry_pick(&self, git: &GitCli, commit: &str) -> Result<SequencerOutcome, Error> {
        self.run_sequencer(git, SequencerKind::CherryPick, commit)
    }

    /// Reverts `commit` on HEAD. Clean reverts commit at once; conflicted ones
    /// stop (see [`Repo::sequencer_state`]).
    ///
    /// # Errors
    /// [`Error::Refused`] when an operation is already in progress or `commit`
    /// is a merge commit; [`Error::Command`] with git's error.
    pub fn revert_commit(&self, git: &GitCli, commit: &str) -> Result<SequencerOutcome, Error> {
        self.run_sequencer(git, SequencerKind::Revert, commit)
    }

    /// The cherry-pick or revert currently stopped on conflicts, if any.
    ///
    /// # Errors
    /// [`Error::Read`] when git's index or sequencer files can't be read.
    pub fn sequencer_state(&self, git: &GitCli) -> Result<Option<SequencerState>, Error> {
        let Some((kind, head_oid)) = self.sequencer_head()? else {
            return Ok(None);
        };
        let now = self.conflicted_paths()?;
        let started: Option<Started> = fs::read(self.sequencer_file())
            .ok()
            .map(|b| serde_json::from_slice::<Started>(&b).map_err(read_err))
            .transpose()?
            .filter(|s| s.kind == kind && s.head_oid == head_oid);

        let (short_id, subject, mut conflicts) = match started {
            Some(s) => (s.short_id, s.subject, s.conflicts),
            None => {
                let (short, subj) = self.commit_summary(&head_oid)?;
                (short, subj, self.classify(git, &now)?)
            }
        };
        for c in &mut conflicts {
            c.resolved = !now.contains(&c.path);
        }
        let msg = fs::read_to_string(self.git_dir().join("MERGE_MSG")).unwrap_or_default();
        let message = msg
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();

        Ok(Some(SequencerState {
            kind,
            commit: head_oid,
            short_id,
            subject,
            message,
            conflicts,
        }))
    }

    /// Finishes or aborts the cherry-pick or revert in progress.
    ///
    /// # Errors
    /// [`Error::Refused`] when no sequencer op is running or files remain
    /// conflicted; [`Error::Command`] with git's message.
    pub fn sequencer_step(&self, git: &GitCli, step: SequencerStep) -> Result<(), Error> {
        let Some((kind, _)) = self.sequencer_head()? else {
            return Err(Error::Refused(
                "no cherry-pick or revert is in progress".into(),
            ));
        };
        let sub = match kind {
            SequencerKind::CherryPick => "cherry-pick",
            SequencerKind::Revert => "revert",
        };
        match step {
            SequencerStep::Continue => {
                let left = self.conflicted_paths()?;
                if !left.is_empty() {
                    return Err(Error::Refused(format!(
                        "{} file(s) still conflicted",
                        left.len()
                    )));
                }
                let args = [sub, "--continue"];
                let mut cmd = git.command(self.root(), &args);
                cmd.env("GIT_EDITOR", "true");
                let out = cmd
                    .output()
                    .map_err(|e| command_error(&args, e.to_string()))?;
                if !out.status.success() {
                    return Err(command_error(
                        &args,
                        failure_message(
                            &String::from_utf8_lossy(&out.stdout),
                            &String::from_utf8_lossy(&out.stderr),
                        ),
                    ));
                }
                self.forget_sequencer()
            }
            SequencerStep::Abort => {
                git.run(self.root(), &[sub, "--abort"], None)?;
                self.forget_sequencer()
            }
        }
    }

    fn run_sequencer(
        &self,
        git: &GitCli,
        kind: SequencerKind,
        commit: &str,
    ) -> Result<SequencerOutcome, Error> {
        if self.rebasing()
            || self.git_dir().join("MERGE_HEAD").exists()
            || self.sequencer_head()?.is_some()
        {
            return Err(Error::Refused(
                "finish or abort the operation in progress first".into(),
            ));
        }
        let (oid, _) = self.commit_and_parent_tree(git, commit)?;
        let args: Vec<&str> = match kind {
            SequencerKind::CherryPick => vec!["cherry-pick", &oid],
            SequencerKind::Revert => vec!["revert", "--no-edit", &oid],
        };
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        if code == 0 {
            return Ok(SequencerOutcome::Done);
        }
        let Some((_, head_oid)) = self.sequencer_head()? else {
            return Err(command_error(&args, failure_message(&out, &err)));
        };
        let paths = self.conflicted_paths()?;
        if paths.is_empty() {
            let abort = match kind {
                SequencerKind::CherryPick => "cherry-pick",
                SequencerKind::Revert => "revert",
            };
            let _ = git.run(self.root(), &[abort, "--abort"], None);
            return Err(command_error(&args, failure_message(&out, &err)));
        }
        let (short_id, subject) = self.commit_summary(&head_oid)?;
        let started = Started {
            kind,
            head_oid,
            short_id,
            subject,
            conflicts: self.classify(git, &paths)?,
        };
        fs::create_dir_all(self.sequencer_file().parent().expect("has parent"))
            .map_err(read_err)?;
        fs::write(
            self.sequencer_file(),
            serde_json::to_vec(&started).map_err(read_err)?,
        )
        .map_err(read_err)?;
        Ok(SequencerOutcome::Conflicts)
    }

    fn commit_and_parent_tree(
        &self,
        git: &GitCli,
        commit: &str,
    ) -> Result<(String, String), Error> {
        let repo = self.gix();
        let oid = match ObjectId::from_hex(commit.trim().as_bytes()) {
            Ok(id) => id,
            Err(_) => {
                let hex = git.run(
                    self.root(),
                    &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
                    None,
                )?;
                ObjectId::from_hex(hex.trim().as_bytes()).map_err(read_err)?
            }
        };
        let c = repo.find_commit(oid).map_err(read_err)?;
        let parents: Vec<ObjectId> = c.parent_ids().map(gix::Id::detach).collect();
        if parents.len() > 1 {
            return Err(Error::Refused(
                "cherry-pick and revert on merge commits aren't supported yet".into(),
            ));
        }
        let parent_tree = match parents.first() {
            Some(p) => repo
                .find_commit(*p)
                .map_err(read_err)?
                .tree_id()
                .map_err(read_err)?
                .to_string(),
            None => repo.empty_tree().id().to_string(),
        };
        Ok((oid.to_string(), parent_tree))
    }

    fn predict_three_way(
        &self,
        git: &GitCli,
        base: &str,
        ours: &str,
        theirs: &str,
    ) -> Result<Vec<String>, Error> {
        let base_arg = format!("--merge-base={base}");
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            base_arg.as_str(),
            ours,
            theirs,
        ];
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        match code {
            0 => Ok(Vec::new()),
            1 => Ok(out
                .split('\0')
                .skip(1)
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect()),
            _ => Err(command_error(&args, failure_message(&out, &err))),
        }
    }

    fn commit_summary(&self, hex: &str) -> Result<(String, String), Error> {
        let repo = self.gix();
        let oid = ObjectId::from_hex(hex.as_bytes()).map_err(read_err)?;
        let commit = repo.find_commit(oid).map_err(read_err)?;
        let msg = commit.message().map_err(read_err)?;
        Ok((
            oid.to_hex_with_len(7).to_string(),
            msg.summary().to_string(),
        ))
    }

    pub(crate) fn sequencer_head(&self) -> Result<Option<(SequencerKind, String)>, Error> {
        for (file, kind) in [
            ("CHERRY_PICK_HEAD", SequencerKind::CherryPick),
            ("REVERT_HEAD", SequencerKind::Revert),
        ] {
            match fs::read_to_string(self.git_dir().join(file)) {
                Ok(s) => return Ok(Some((kind, s.trim().to_owned()))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(read_err(e)),
            }
        }
        Ok(None)
    }

    fn sequencer_file(&self) -> PathBuf {
        self.git_dir().join("gitgud").join("sequencer.json")
    }

    fn forget_sequencer(&self) -> Result<(), Error> {
        match fs::remove_file(self.sequencer_file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
            _ => Ok(()),
        }
    }
}
