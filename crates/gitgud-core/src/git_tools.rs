use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::Serialize;

use crate::cli::{GitCli, no_console_window};
use crate::git::{
    self, GitChoice, GitInfo, GitSource, GitVersion, MIN_GIT, MIN_GIT_LFS, version_text,
};
use crate::repo::Error;

const GIT_FOR_WINDOWS: &str = "https://github.com/git-for-windows/git";
const DOWNLOAD_PAGE: &str = "https://git-scm.com/download/win";

/// One git executable as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInstall {
    pub path: String,
    /// The full release, `2.56.1.windows.1`.
    pub version: Option<String>,
    /// Why it can't be used: it doesn't run, or is older than [`MIN_GIT`].
    pub error: Option<String>,
    pub lfs_version: Option<String>,
    pub lfs_ok: bool,
    /// Every `credential.helper` it sees, in order (the last one asked first).
    pub credential_helpers: Vec<String>,
    /// Git Credential Manager's version, when this git can run it.
    pub credential_manager: Option<String>,
}

/// The gits on this machine, which one is chosen, and which one runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInventory {
    pub choice: GitChoice,
    pub active: Option<GitInfo>,
    pub active_error: Option<String>,
    pub system: Option<GitInstall>,
    pub embedded: Option<GitInstall>,
    pub custom: Option<GitInstall>,
    pub min_git: String,
    pub settings_path: Option<String>,
}

/// Lists the system, embedded and custom gits and the one `choice` resolves to.
#[must_use]
pub fn inventory(choice: &GitChoice) -> GitInventory {
    let custom_path = match choice {
        GitChoice::Custom { path } => Some(path.clone()),
        _ => None,
    };
    let (active, system, embedded, custom) = std::thread::scope(|s| {
        let active = s.spawn(|| git::locate_choice(choice));
        let system = s.spawn(|| git::system_git().map(|p| probe(p, GitSource::System)));
        let embedded = s.spawn(|| git::embedded_git().map(|p| probe(p, GitSource::Bundled)));
        let custom = s.spawn(|| custom_path.map(|p| probe(p, GitSource::Custom)));
        (
            active.join().expect("locate panicked"),
            system.join().expect("probe panicked"),
            embedded.join().expect("probe panicked"),
            custom.join().expect("probe panicked"),
        )
    });
    let (active, active_error) = match active {
        Ok(info) => (Some(info), None),
        Err(e) => (None, Some(e.to_string())),
    };
    GitInventory {
        choice: choice.clone(),
        active,
        active_error,
        system,
        embedded,
        custom,
        min_git: MIN_GIT.to_string(),
        settings_path: crate::git_choice::settings_path().map(|p| p.display().to_string()),
    }
}

fn probe(path: PathBuf, source: GitSource) -> GitInstall {
    let mut out = GitInstall {
        path: path.display().to_string(),
        version: None,
        error: None,
        lfs_version: None,
        lfs_ok: false,
        credential_helpers: Vec::new(),
        credential_manager: None,
    };
    if !path.is_file() {
        out.error = Some(git::Error::NotAFile(path).to_string());
        return out;
    }
    let text = match version_text(&path) {
        Ok(text) => text,
        Err(e) => {
            out.error = Some(e.to_string());
            return out;
        }
    };
    let version = match GitVersion::parse(&text) {
        Ok(v) => v,
        Err(e) => {
            out.error = Some(e.to_string());
            return out;
        }
    };
    out.version = Some(release_of(&text).to_owned());
    if version < MIN_GIT {
        out.error = Some(
            git::Error::TooOld {
                found: version,
                min: MIN_GIT,
            }
            .to_string(),
        );
    }
    let cli = GitCli::new(&GitInfo {
        path,
        version,
        source,
    });
    let cwd = std::env::temp_dir();
    let lfs = cli
        .run(&cwd, &["lfs", "version"], None)
        .ok()
        .and_then(|s| GitVersion::parse_lfs(&s).ok());
    out.lfs_version = lfs.map(|v| v.to_string());
    out.lfs_ok = lfs.is_some_and(|v| v >= MIN_GIT_LFS);
    out.credential_helpers = cli
        .run(&cwd, &["config", "--get-all", "credential.helper"], None)
        .map(|s| s.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    out.credential_manager = cli
        .run(&cwd, &["credential-manager", "--version"], None)
        .ok()
        .and_then(|s| {
            let v = s.trim();
            // Drop the build hash: `2.9.1+6760f0e...`.
            let v = v.split('+').next().unwrap_or(v);
            (!v.is_empty()).then(|| v.to_owned())
        });
    out
}

/// `2.56.1.windows.1` from `git version 2.56.1.windows.1`.
fn release_of(version_output: &str) -> &str {
    version_output
        .trim()
        .strip_prefix("git version ")
        .unwrap_or(version_output.trim())
}

/// Orders releases: `2.56.1.windows.2` > `2.56.1.windows.1` > `2.56.0`; `None` for release
/// candidates and anything else that isn't a plain release.
fn release_key(release: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = release.split('.');
    let mut num = || parts.next()?.parse::<u32>().ok();
    let (major, minor, patch) = (num()?, num()?, num()?);
    let rest: Vec<&str> = parts.collect();
    let build = match rest.as_slice() {
        [] => 0,
        ["windows", n] => n.parse().ok()?,
        _ => return None,
    };
    Some((major, minor, patch, build))
}

/// The newest Git for Windows release, and whether the system / embedded gits are behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestGit {
    pub version: String,
    pub system_behind: bool,
    pub embedded_behind: bool,
}

/// Asks GitHub for Git for Windows' release tags (through `git ls-remote`, so the user's proxy
/// and certificates apply) and compares them with the installed gits.
///
/// # Errors
/// [`Error::Command`] when the tags can't be listed; [`Error::Refused`] when none parse.
pub fn latest_git(git: &GitCli) -> Result<LatestGit, Error> {
    let out = git.run_background(
        &std::env::temp_dir(),
        &[
            "ls-remote",
            "--tags",
            "--refs",
            GIT_FOR_WINDOWS,
            "refs/tags/v2.*",
        ],
    )?;
    let version = newest_release(&out)
        .ok_or_else(|| Error::Refused("Git for Windows lists no release tags".into()))?;
    let behind = |path: Option<PathBuf>| {
        path.and_then(|p| version_text(&p).ok())
            .and_then(|t| release_key(release_of(&t)))
            .zip(release_key(&version))
            .is_some_and(|(have, latest)| have < latest)
    };
    Ok(LatestGit {
        system_behind: behind(git::system_git()),
        embedded_behind: behind(git::embedded_git()),
        version,
    })
}

fn newest_release(ls_remote: &str) -> Option<String> {
    ls_remote
        .lines()
        .filter_map(|line| line.split('\t').nth(1)?.strip_prefix("refs/tags/v"))
        .filter_map(|tag| Some((release_key(tag)?, tag)))
        .max_by_key(|(key, _)| *key)
        .map(|(_, tag)| tag.to_owned())
}

/// What `git update-git-for-windows` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SystemUpdate {
    UpToDate { message: String },
    InstallerStarted { message: String },
}

/// Runs Git for Windows' own updater on the system git. It downloads the newest installer and
/// starts it (UAC, progress window), closing open Git Bash windows as it always does.
///
/// # Errors
/// [`Error::Refused`] without a system git; [`Error::Command`] when the updater fails.
pub fn update_system_git() -> Result<SystemUpdate, Error> {
    let path = git::system_git().ok_or_else(|| {
        Error::Refused("There is no system git on PATH to update. Install Git for Windows.".into())
    })?;
    let args = ["update-git-for-windows", "--yes"];
    let mut cmd = Command::new(&path);
    cmd.args(args).current_dir(std::env::temp_dir());
    no_console_window(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| crate::cli::command_error(&args, e.to_string()))?;
    let message = crate::cli::failure_message(
        &String::from_utf8_lossy(&out.stdout),
        &String::from_utf8_lossy(&out.stderr),
    );
    // The updater returns 2 once it has started the installer.
    match out.status.code() {
        Some(0) => Ok(SystemUpdate::UpToDate { message }),
        Some(2) => Ok(SystemUpdate::InstallerStarted { message }),
        _ => Err(crate::cli::command_error(&args, message)),
    }
}

/// Starts `winget install Git.Git` in its own console window, where the user answers its prompts.
///
/// # Errors
/// [`Error::Refused`] when winget isn't available.
pub fn install_system_git() -> Result<(), Error> {
    let mut cmd = Command::new("winget");
    cmd.args(["install", "--id", "Git.Git", "-e", "--source", "winget"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        cmd.creation_flags(CREATE_NEW_CONSOLE);
    }
    cmd.spawn().map(drop).map_err(|e| {
        Error::Refused(format!(
            "Could not start winget ({e}). Download Git for Windows from {DOWNLOAD_PAGE}"
        ))
    })
}

/// One config key as the system and the embedded git see it outside any repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRow {
    pub scope: String,
    pub key: String,
    pub system: Vec<String>,
    pub embedded: Vec<String>,
    pub differs: bool,
}

/// [`compare_config`] for the system git and the embedded one.
///
/// # Errors
/// [`Error::Git`] when either is missing or too old; as [`compare_config`].
pub fn compare_installed_config() -> Result<Vec<ConfigRow>, Error> {
    let system = GitCli::new(&git::locate_choice(&GitChoice::System)?);
    let embedded = GitCli::new(&git::locate_choice(&GitChoice::Embedded)?);
    compare_config(&system, &embedded)
}

/// Compares the system- and global-scope config two gits read, differing keys first.
///
/// # Errors
/// [`Error::Command`] when either git can't list its config.
pub fn compare_config(system: &GitCli, embedded: &GitCli) -> Result<Vec<ConfigRow>, Error> {
    let cwd = std::env::temp_dir();
    let args = ["config", "--list", "--show-scope", "--includes", "-z"];
    let a = parse_config_list(&system.run(&cwd, &args, None)?);
    let b = parse_config_list(&embedded.run(&cwd, &args, None)?);
    let mut keys: Vec<&(String, String)> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut rows: Vec<ConfigRow> = keys
        .into_iter()
        .map(|k| {
            let system = a.get(k).cloned().unwrap_or_default();
            let embedded = b.get(k).cloned().unwrap_or_default();
            ConfigRow {
                scope: k.0.clone(),
                key: k.1.clone(),
                differs: system != embedded,
                system,
                embedded,
            }
        })
        .collect();
    rows.sort_by_key(|r| !r.differs);
    Ok(rows)
}

/// `git config --list --show-scope -z` → values by (scope, key), system and global only.
fn parse_config_list(out: &str) -> BTreeMap<(String, String), Vec<String>> {
    let mut map: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut tokens = out.split('\0');
    while let (Some(scope), Some(entry)) = (tokens.next(), tokens.next()) {
        if scope != "system" && scope != "global" {
            continue;
        }
        let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
        // The embedded git reads its own gitconfig and then Git for Windows' one it includes, so
        // the same value often comes twice in a row; that's one effective setting.
        let values = map.entry((scope.to_owned(), key.to_owned())).or_default();
        if values.last().map(String::as_str) != Some(value) {
            values.push(value.to_owned());
        }
    }
    map
}

/// The result of reaching a repository's remote with the chosen git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCheck {
    pub remote: String,
    pub url: String,
    pub ok: bool,
    pub elapsed_ms: u64,
    /// git's error when it failed.
    pub message: Option<String>,
}

/// Lists the upstream's remote (else `origin`, else the first remote) with `ls-remote`, never
/// prompting, so stored credentials and SSH keys are what's tested.
///
/// # Errors
/// [`Error::Refused`] when the repository has no remote; [`Error::Command`] when git can't read it.
pub fn check_remote(git: &GitCli, repo: &Path) -> Result<RemoteCheck, Error> {
    let remotes: Vec<String> = git
        .run(repo, &["remote"], None)?
        .lines()
        .map(str::to_owned)
        .collect();
    let upstream = git
        .run(repo, &["rev-parse", "--abbrev-ref", "@{upstream}"], None)
        .unwrap_or_default();
    let remote = pick_remote(&remotes, upstream.trim())
        .ok_or_else(|| Error::Refused("This repository has no remote to check.".into()))?;
    let url = git
        .run(repo, &["remote", "get-url", &remote], None)?
        .trim()
        .to_owned();
    let start = Instant::now();
    let result = git.run_background(repo, &["ls-remote", "--heads", &remote]);
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(RemoteCheck {
        remote,
        url,
        ok: result.is_ok(),
        elapsed_ms,
        message: result.err().map(|e| e.to_string()),
    })
}

fn pick_remote(remotes: &[String], upstream: &str) -> Option<String> {
    remotes
        .iter()
        .filter(|r| upstream.starts_with(&format!("{r}/")))
        .max_by_key(|r| r.len())
        .or_else(|| remotes.iter().find(|r| *r == "origin"))
        .or_else(|| remotes.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn release_keys_order_windows_builds_and_skip_candidates() {
        assert!(release_key("2.56.1.windows.2") > release_key("2.56.1.windows.1"));
        assert!(release_key("2.56.1.windows.1") > release_key("2.56.0"));
        assert!(release_key("2.57.0") > release_key("2.56.9.windows.3"));
        assert_eq!(release_key("2.57.0-rc1.windows.1"), None);
        assert_eq!(release_key("2.57.0.windows.1.arm"), None);
        assert_eq!(
            release_of("git version 2.56.1.windows.1\n"),
            "2.56.1.windows.1"
        );
    }

    #[test]
    fn newest_release_ignores_rcs_and_compares_numerically() {
        let out = "a\trefs/tags/v2.9.0.windows.1\n\
                   b\trefs/tags/v2.56.1.windows.1\n\
                   c\trefs/tags/v2.57.0-rc2.windows.1\n\
                   d\trefs/tags/v2.56.1.windows.2\n\
                   e\trefs/tags/v2.56.0.windows.1\n";
        assert_eq!(newest_release(out).as_deref(), Some("2.56.1.windows.2"));
        assert_eq!(newest_release(""), None);
    }

    #[test]
    fn parses_config_list_by_scope_and_key() {
        let out = "system\0core.autocrlf\ntrue\0global\0user.name\nMax\0system\0core.autocrlf\ntrue\0\
                   system\0credential.helper\nmanager\0system\0credential.helper\nwincred\0\
                   local\0core.bare\nfalse\0global\0novalue\0";
        let map = parse_config_list(out);
        let get = |s: &str, k: &str| map.get(&(s.to_owned(), k.to_owned())).cloned();
        assert_eq!(get("system", "core.autocrlf"), Some(vec!["true".into()]));
        assert_eq!(
            get("system", "credential.helper"),
            Some(vec!["manager".into(), "wincred".into()])
        );
        assert_eq!(get("global", "novalue"), Some(vec![String::new()]));
        assert_eq!(get("local", "core.bare"), None);
    }

    #[test]
    fn picks_the_upstream_remote_then_origin_then_first() {
        let r = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            pick_remote(&r(&["origin", "fork", "fork/x"]), "fork/x/main").as_deref(),
            Some("fork/x")
        );
        assert_eq!(
            pick_remote(&r(&["a", "origin"]), "").as_deref(),
            Some("origin")
        );
        assert_eq!(pick_remote(&r(&["a", "b"]), "").as_deref(), Some("a"));
        assert_eq!(pick_remote(&[], ""), None);
    }

    #[test]
    fn inventory_reports_the_active_git() {
        let inv = inventory(&GitChoice::Auto);
        let active = inv.active.expect("a git for the tests");
        assert!(active.version >= MIN_GIT);
        let install = match active.source {
            GitSource::System => inv.system,
            GitSource::Bundled => inv.embedded,
            GitSource::Custom => inv.custom,
        }
        .expect("the active git is listed");
        assert_eq!(install.error, None);
        assert!(install.version.is_some());
    }

    #[test]
    fn custom_choice_must_be_a_new_enough_git() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("git.exe");
        let inv = inventory(&GitChoice::Custom {
            path: missing.clone(),
        });
        assert!(inv.active.is_none());
        assert!(inv.active_error.unwrap().contains("not a git executable"));
        assert!(inv.custom.unwrap().error.is_some());

        let real = git::locate_choice(&GitChoice::Auto).unwrap().path;
        let info = git::locate_choice(&GitChoice::Custom { path: real }).unwrap();
        assert_eq!(info.source, GitSource::Custom);
    }

    #[test]
    fn compares_config_and_flags_differences() {
        let cli = GitCli::new(&git::locate_choice(&GitChoice::Auto).unwrap());
        let rows = compare_config(&cli, &cli).unwrap();
        assert!(rows.iter().all(|r| !r.differs));
    }

    #[test]
    fn checks_a_reachable_remote_and_reports_a_broken_one() {
        let origin = TestRepo::init();
        origin.write("a.txt", "a\n").commit_all("first");
        let repo = TestRepo::init();
        let cli = GitCli::new(&git::locate_choice(&GitChoice::Auto).unwrap());
        let url = origin.path().display().to_string();
        cli.run(repo.path(), &["remote", "add", "origin", &url], None)
            .unwrap();
        let ok = check_remote(&cli, repo.path()).unwrap();
        assert!(ok.ok, "{:?}", ok.message);
        assert_eq!(ok.remote, "origin");

        cli.run(
            repo.path(),
            &["remote", "set-url", "origin", &format!("{url}-gone")],
            None,
        )
        .unwrap();
        let bad = check_remote(&cli, repo.path()).unwrap();
        assert!(!bad.ok);
        assert!(bad.message.is_some());

        let lonely = TestRepo::init();
        assert!(matches!(
            check_remote(&cli, lonely.path()),
            Err(Error::Refused(_))
        ));
    }
}
