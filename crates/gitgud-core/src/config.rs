use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::git::GitInfo;
use crate::repo::Error;

/// Git `user.name` and `user.email` configured at a scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityConfig {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
}

/// Installed tool versions shown in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDiagnostics {
    pub git_path: String,
    pub git_version: String,
    pub lfs_version: Option<String>,
}

/// Scope for reading or updating git configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigScope {
    Global,
    Local,
}

/// Reads `user.name` and `user.email` from global config or from `repo`'s local config.
///
/// # Errors
/// [`Error::Command`] if `git config` fails unexpectedly.
pub fn read_identity(git: &GitCli, repo: Option<&Path>) -> Result<IdentityConfig, Error> {
    let cwd = repo.unwrap_or_else(|| Path::new("."));
    let scope_flag = if repo.is_some() {
        "--local"
    } else {
        "--global"
    };
    Ok(IdentityConfig {
        user_name: read_config_key(git, cwd, scope_flag, "user.name")?,
        user_email: read_config_key(git, cwd, scope_flag, "user.email")?,
    })
}

/// Writes or unsets `user.name` and `user.email` in global or local config.
///
/// # Errors
/// [`Error::Command`] if `git config` fails.
pub fn write_identity(
    git: &GitCli,
    repo: Option<&Path>,
    name: &str,
    email: &str,
) -> Result<(), Error> {
    let cwd = repo.unwrap_or_else(|| Path::new("."));
    let scope_flag = if repo.is_some() {
        "--local"
    } else {
        "--global"
    };
    write_config_key(git, cwd, scope_flag, "user.name", name)?;
    write_config_key(git, cwd, scope_flag, "user.email", email)?;
    Ok(())
}

/// Collects installed git and git-lfs version information.
#[must_use]
pub fn tool_diagnostics(git: &GitCli, info: &GitInfo) -> ToolDiagnostics {
    let lfs_version = git
        .run(Path::new("."), &["lfs", "version"], None)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    ToolDiagnostics {
        git_path: info.path.to_string_lossy().into_owned(),
        git_version: info.version.to_string(),
        lfs_version,
    }
}

fn read_config_key(
    git: &GitCli,
    cwd: &Path,
    scope_flag: &str,
    key: &str,
) -> Result<Option<String>, Error> {
    let (code, stdout, stderr) =
        git.run_status(cwd, &["config", scope_flag, "--get", key], None)?;
    if code == 0 {
        let val = stdout.trim().to_owned();
        Ok((!val.is_empty()).then_some(val))
    } else if code == 1 {
        Ok(None)
    } else {
        let msg = stderr.trim();
        Err(Error::Command {
            command: format!("git config {scope_flag} --get {key}"),
            message: if msg.is_empty() {
                format!("git config exited with status {code}")
            } else {
                msg.to_owned()
            },
        })
    }
}

fn write_config_key(
    git: &GitCli,
    cwd: &Path,
    scope_flag: &str,
    key: &str,
    value: &str,
) -> Result<(), Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        let (code, _, _) = git.run_status(cwd, &["config", scope_flag, "--unset", key], None)?;
        // Exit code 5 means key was already absent.
        if code == 0 || code == 5 {
            Ok(())
        } else {
            git.run(cwd, &["config", scope_flag, "--unset", key], None)
                .map(drop)
        }
    } else {
        git.run(cwd, &["config", scope_flag, key, trimmed], None)
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    use crate::testing::TestRepo;

    #[test]
    fn reads_writes_and_unsets_local_identity() {
        let repo = TestRepo::init();
        let info = git::locate().unwrap();
        let cli = GitCli::new(&info);

        write_identity(&cli, Some(repo.path()), "Max Dev", "max@example.org").unwrap();
        let id = read_identity(&cli, Some(repo.path())).unwrap();
        assert_eq!(
            id,
            IdentityConfig {
                user_name: Some("Max Dev".into()),
                user_email: Some("max@example.org".into()),
            }
        );

        write_identity(&cli, Some(repo.path()), "", "").unwrap();
        let cleared = read_identity(&cli, Some(repo.path())).unwrap();
        assert_eq!(
            cleared,
            IdentityConfig {
                user_name: None,
                user_email: None,
            }
        );

        let diag = tool_diagnostics(&cli, &info);
        assert!(!diag.git_version.is_empty());
    }
}
