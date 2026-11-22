use serde::Deserialize;

use crate::cli::GitCli;
use crate::repo::{EntryKind, Error, Repo, read_err};

/// Options for creating or amending a commit (spec §3.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitOptions {
    #[serde(default)]
    pub amend: bool,
    #[serde(default)]
    pub signoff: bool,
    #[serde(default)]
    pub no_verify: bool,
}

/// How `git reset` moves HEAD, the index and the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

/// Writes on the working copy, through the git CLI so hooks run.
impl Repo {
    /// Stages `paths` as they are on disk: changes, new files and deletions.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn stage(&self, git: &GitCli, paths: &[String]) -> Result<(), Error> {
        self.with_paths(git, &["add", "-A"], paths)
    }

    /// Puts `paths` in the index back to HEAD, leaving the work tree alone. On a
    /// branch with no commits yet, removes them from the index.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message, [`Error::Read`] when HEAD can't be read.
    pub fn unstage(&self, git: &GitCli, paths: &[String]) -> Result<(), Error> {
        let unborn = self.gix().head().map_err(read_err)?.is_unborn();
        if unborn {
            self.with_paths(git, &["rm", "--cached", "-r", "-q"], paths)
        } else {
            self.with_paths(git, &["restore", "--staged"], paths)
        }
    }

    /// Commits the index with `message`. Hooks run; a failing hook fails the commit.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message: nothing staged, an empty message,
    /// a rejecting hook.
    pub fn commit(&self, git: &GitCli, message: &str) -> Result<(), Error> {
        self.commit_with(git, message, CommitOptions::default())
    }

    /// Commits or amends the index with `message` and `options`.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn commit_with(
        &self,
        git: &GitCli,
        message: &str,
        options: CommitOptions,
    ) -> Result<(), Error> {
        let mut args = vec!["commit", "-q", "--file=-"];
        if options.amend {
            args.push("--amend");
        }
        if options.signoff {
            args.push("--signoff");
        }
        if options.no_verify {
            args.push("--no-verify");
        }
        git.run(self.root(), &args, Some(message.as_bytes()))
            .map(drop)
    }

    /// Full commit message of HEAD, or `None` when HEAD is unborn.
    ///
    /// # Errors
    /// [`Error::Read`] when HEAD can't be read.
    pub fn head_commit_message(&self) -> Result<Option<String>, Error> {
        let repo = self.gix();
        if repo.head().map_err(read_err)?.is_unborn() {
            return Ok(None);
        }
        let commit = repo.head_commit().map_err(read_err)?;
        let raw = commit.message_raw().map_err(read_err)?;
        Ok(Some(raw.to_string().trim_end_matches('\n').to_owned()))
    }

    /// Resets the current branch to `target` with `mode`.
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn reset(&self, git: &GitCli, target: &str, mode: ResetMode) -> Result<(), Error> {
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        git.run(self.root(), &["reset", "-q", flag, target], None)
            .map(drop)
    }

    /// Tracked work-tree paths a `--hard` reset to `target` can overwrite or delete.
    ///
    /// # Errors
    /// [`Error::Command`] when `target` doesn't resolve, or [`Error::Read`].
    pub fn reset_hard_paths(&self, git: &GitCli, target: &str) -> Result<Vec<String>, Error> {
        let mut paths = self.checkout_rev_paths(git, target)?;
        for entry in self.status()?.entries {
            if entry.kind != EntryKind::Untracked {
                paths.push(entry.path);
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn with_paths(&self, git: &GitCli, args: &[&str], paths: &[String]) -> Result<(), Error> {
        if paths.is_empty() {
            return Ok(());
        }
        // Literal pathspecs from stdin: no glob surprises, no command-line limit.
        let mut full = vec!["--literal-pathspecs"];
        full.extend_from_slice(args);
        full.extend(["--pathspec-from-file=-", "--pathspec-file-nul"]);
        git.run(self.root(), &full, Some(paths.join("\0").as_bytes()))
            .map(drop)
    }
}
