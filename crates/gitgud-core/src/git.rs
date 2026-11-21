use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Serialize, Serializer};

/// Oldest system git gitgud accepts (spec §2).
pub const MIN_GIT: GitVersion = GitVersion {
    major: 2,
    minor: 45,
    patch: 2,
};

/// A git release, compared by major, minor, then patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Parses `git --version` output such as `git version 2.56.1.windows.1`.
    ///
    /// # Errors
    /// [`Error::UnparsableVersion`] when the output has no `major.minor` number.
    pub fn parse(output: &str) -> Result<Self, Error> {
        let bad = || Error::UnparsableVersion(output.trim().to_owned());
        let rest = output.trim().strip_prefix("git version ").ok_or_else(bad)?;
        let mut parts = rest.split('.').map(leading_number);
        let major = parts.next().flatten().ok_or_else(bad)?;
        let minor = parts.next().flatten().ok_or_else(bad)?;
        let patch = parts.next().flatten().unwrap_or(0);
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

fn leading_number(s: &str) -> Option<u32> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s[..end].parse().ok()
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Serialize for GitVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// The git executable gitgud runs, and its version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitInfo {
    pub path: PathBuf,
    pub version: GitVersion,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git was not found on PATH")]
    NotFound,
    #[error("could not run {path}: {source}")]
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unexpected `git --version` output: {0:?}")]
    UnparsableVersion(String),
    #[error("git {found} is too old; gitgud needs {min} or newer")]
    TooOld { found: GitVersion, min: GitVersion },
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Finds `git` on the process `PATH` and checks it against [`MIN_GIT`].
///
/// # Errors
/// See [`locate_in`].
pub fn locate() -> Result<GitInfo, Error> {
    locate_in(std::env::var_os("PATH").unwrap_or_default())
}

/// Finds `git` on the given search path and checks it against [`MIN_GIT`].
///
/// # Errors
/// [`Error::NotFound`], [`Error::Spawn`], [`Error::UnparsableVersion`] or [`Error::TooOld`].
pub fn locate_in(search_path: impl AsRef<OsStr>) -> Result<GitInfo, Error> {
    let cwd = std::env::current_dir().map_err(|_| Error::NotFound)?;
    let path = which::which_in("git", Some(search_path), cwd).map_err(|_| Error::NotFound)?;
    let version = version_of(&path)?;
    if version < MIN_GIT {
        return Err(Error::TooOld {
            found: version,
            min: MIN_GIT,
        });
    }
    Ok(GitInfo { path, version })
}

fn version_of(git: &Path) -> Result<GitVersion, Error> {
    let spawn_err = |source| Error::Spawn {
        path: git.to_owned(),
        source,
    };
    let mut cmd = Command::new(git);
    cmd.arg("--version");
    no_console_window(&mut cmd);
    let out = cmd.output().map_err(spawn_err)?;
    if !out.status.success() {
        return Err(Error::UnparsableVersion(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    GitVersion::parse(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(windows)]
fn no_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_console_window(_: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_plain_version() {
        assert_eq!(
            GitVersion::parse("git version 2.45.2\n").unwrap(),
            v(2, 45, 2)
        );
    }

    #[test]
    fn parses_windows_build() {
        assert_eq!(
            GitVersion::parse("git version 2.56.1.windows.1").unwrap(),
            v(2, 56, 1)
        );
    }

    #[test]
    fn parses_apple_build() {
        assert_eq!(
            GitVersion::parse("git version 2.39.3 (Apple Git-146)").unwrap(),
            v(2, 39, 3)
        );
    }

    #[test]
    fn parses_release_candidate() {
        assert_eq!(
            GitVersion::parse("git version 2.47.0.rc1").unwrap(),
            v(2, 47, 0)
        );
        assert_eq!(
            GitVersion::parse("git version 2.47.0-rc1").unwrap(),
            v(2, 47, 0)
        );
    }

    #[test]
    fn missing_patch_is_zero() {
        assert_eq!(GitVersion::parse("git version 2.50").unwrap(), v(2, 50, 0));
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "hello",
            "git version",
            "git version x.y",
            "git version 2",
        ] {
            assert!(
                matches!(GitVersion::parse(bad), Err(Error::UnparsableVersion(_))),
                "{bad:?} should not parse"
            );
        }
    }

    #[test]
    fn orders_by_component_not_text() {
        assert!(v(2, 45, 10) > v(2, 45, 2));
        assert!(v(2, 100, 0) > v(2, 45, 2));
        assert!(v(3, 0, 0) > v(2, 99, 99));
        assert!(v(2, 45, 1) < MIN_GIT);
        assert!(v(2, 45, 2) >= MIN_GIT);
    }

    #[test]
    fn displays_and_serializes_as_dotted_string() {
        assert_eq!(v(2, 56, 1).to_string(), "2.56.1");
        let too_old = Error::TooOld {
            found: v(2, 30, 0),
            min: MIN_GIT,
        };
        assert_eq!(
            too_old.to_string(),
            "git 2.30.0 is too old; gitgud needs 2.45.2 or newer"
        );
    }
}
