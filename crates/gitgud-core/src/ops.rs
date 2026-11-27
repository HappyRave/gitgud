use std::collections::HashSet;

use serde::Deserialize;

use crate::cli::{GitCli, command_error, failure_message};
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
        if paths.len() >= crate::prewarm::PREWARM_MIN
            && let Err(e) = crate::prewarm::prewarm_blobs(git, self, paths)
        {
            // Only a head start for `git add` below, which writes whatever is missing.
            eprintln!("gitgud: writing blobs ahead of git add failed, staging the slow way: {e}");
        }
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

    /// Returns true if a git commit-graph file is present.
    #[must_use]
    pub fn has_commit_graph(&self) -> bool {
        let objects_info = self.gix().common_dir().join("objects/info");
        objects_info.join("commit-graph").is_file()
            || objects_info
                .join("commit-graphs/commit-graph-chain")
                .is_file()
    }

    /// Writes a reachable changed-paths commit-graph (`git commit-graph write`).
    ///
    /// # Errors
    /// [`Error::Command`] on failure.
    pub fn write_commit_graph(&self, git: &GitCli) -> Result<(), Error> {
        git.run(
            self.root(),
            &["commit-graph", "write", "--reachable", "--changed-paths"],
            None,
        )
        .map(drop)
    }

    /// Commits reachable from `tip_ref` (e.g. `origin/main`) that are not in `exclude_ref` (e.g. `HEAD`).
    /// Returns an empty list if either ref does not resolve.
    ///
    /// # Errors
    /// [`Error::Read`] when commits cannot be traversed.
    pub fn commits_only_in(
        &self,
        tip_ref: &str,
        exclude_ref: &str,
        limit: usize,
    ) -> Result<Vec<String>, Error> {
        let repo = self.gix();
        let Some(tip) = resolve_oid(&repo, tip_ref)? else {
            return Ok(Vec::new());
        };
        let Some(exclude) = resolve_oid(&repo, exclude_ref)? else {
            return Ok(Vec::new());
        };
        let walk = repo
            .rev_walk([tip])
            .with_hidden([exclude])
            .all()
            .map_err(read_err)?;
        let mut out = Vec::new();
        for info in walk.take(limit) {
            let info = info.map_err(read_err)?;
            out.push(info.id.to_string());
        }
        Ok(out)
    }

    /// Refreshes index stat info (`git update-index -q --refresh`) after bulk IDE/Unity file touches.
    ///
    /// # Errors
    /// [`Error::Command`] when git fails with an unexpected status code.
    pub fn refresh_index(&self, git: &GitCli) -> Result<(), Error> {
        let args = ["update-index", "-q", "--refresh"];
        let (code, stdout, stderr) = git.run_status(self.root(), &args, None)?;
        if code == 0 || code == 1 {
            Ok(())
        } else {
            Err(command_error(&args, failure_message(&stdout, &stderr)))
        }
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

fn resolve_oid(repo: &gix::Repository, spec: &str) -> Result<Option<gix::ObjectId>, Error> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed == "HEAD" {
        return Ok(repo.head_id().ok().map(gix::Id::detach));
    }
    let candidates = [
        trimmed.to_owned(),
        format!("refs/remotes/{trimmed}"),
        format!("refs/heads/{trimmed}"),
    ];
    let mut seen = HashSet::new();
    for name in candidates {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Ok(Some(mut r)) = repo.try_find_reference(&name)
            && let Ok(oid) = r.peel_to_id()
        {
            return Ok(Some(oid.detach()));
        }
    }
    Ok(gix::ObjectId::from_hex(trimmed.as_bytes()).ok())
}
