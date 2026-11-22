use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};

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
        git.run(
            self.root(),
            &["commit", "-q", "--file=-"],
            Some(message.as_bytes()),
        )
        .map(drop)
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
