use gix::remote::Direction;
use serde::{Deserialize, Serialize};

use crate::repo::{Error, Repo, read_err};

/// Hosting provider inferred from a remote URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostKind {
    GitHub,
    Bitbucket,
    Other,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRemote {
    host_kind: HostKind,
    repo_path: String,
}

/// Detects the hosting provider of a git remote URL.
#[must_use]
pub fn detect_host(url: &str) -> HostKind {
    parse_remote(url).map_or(HostKind::None, |r| r.host_kind)
}

/// Builds the browser URL to open or create a pull request for `branch` on `url`,
/// supporting GitHub and Bitbucket Cloud.
#[must_use]
pub fn pr_url_for(url: &str, branch: &str) -> Option<String> {
    let parsed = parse_remote(url)?;
    if parsed.repo_path.is_empty() || branch.trim().is_empty() {
        return None;
    }
    match parsed.host_kind {
        HostKind::GitHub => {
            let encoded_path = branch
                .split('/')
                .map(percent_encode)
                .collect::<Vec<_>>()
                .join("/");
            Some(format!(
                "https://github.com/{}/pull/new/{encoded_path}",
                parsed.repo_path
            ))
        }
        HostKind::Bitbucket => Some(format!(
            "https://bitbucket.org/{}/pull-requests/new?source={}&t=1",
            parsed.repo_path,
            percent_encode(branch)
        )),
        HostKind::Other | HostKind::None => None,
    }
}

impl Repo {
    /// Host of the repository's primary remote (`origin`, else the first configured remote).
    #[must_use]
    pub fn primary_host(&self) -> HostKind {
        self.primary_remote_url()
            .map_or(HostKind::None, |u| detect_host(&u))
    }

    /// Pull request creation URL for `branch` on its remote (or `origin` / first remote).
    ///
    /// # Errors
    /// [`Error::Read`] when git config can't be read.
    pub fn pr_url(&self, branch: &str) -> Result<Option<String>, Error> {
        let repo = self.gix();
        let remote_name = repo
            .try_find_reference(&format!("refs/heads/{branch}"))
            .map_err(read_err)?
            .and_then(|r| {
                repo.branch_remote_name(r.name(), Direction::Push)
                    .or_else(|| repo.branch_remote_name(r.name(), Direction::Fetch))
            })
            .map(|n| n.as_bstr().to_string());
        let url = match remote_name {
            Some(name) => self.remote_url(&name),
            None => self.primary_remote_url(),
        };
        Ok(url.and_then(|u| pr_url_for(&u, branch)))
    }

    pub(crate) fn remote_url(&self, name: &str) -> Option<String> {
        let repo = self.gix();
        let remote = repo.find_remote(name).ok()?;
        remote
            .url(Direction::Push)
            .or_else(|| remote.url(Direction::Fetch))
            .map(|u| u.to_bstring().to_string())
    }

    pub(crate) fn primary_remote_url(&self) -> Option<String> {
        let repo = self.gix();
        let names: Vec<String> = repo
            .remote_names()
            .into_iter()
            .map(|n| n.to_string())
            .collect();
        let name = if names.iter().any(|n| n == "origin") {
            Some("origin".to_owned())
        } else {
            let mut sorted = names;
            sorted.sort();
            sorted.into_iter().next()
        }?;
        self.remote_url(&name)
    }
}

fn parse_remote(url: &str) -> Option<ParsedRemote> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let (host, raw_path) = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git://"))
    {
        let after_auth = rest.rsplit_once('@').map_or(rest, |(_, after)| after);
        let (host_port, path) = after_auth.split_once('/')?;
        let host = host_port.split(':').next()?;
        (host, path)
    } else if !url.contains("://") && !url.contains('\\') {
        // SCP syntax: `[user@]host:owner/repo.git`. Skip single-letter Windows drive paths.
        let (left, path) = url.split_once(':')?;
        if left.len() == 1 && left.as_bytes()[0].is_ascii_alphabetic() {
            return None;
        }
        let host = left.rsplit_once('@').map_or(left, |(_, h)| h);
        (host, path)
    } else {
        return None;
    };

    let host_lower = host.to_ascii_lowercase();
    let host_kind = if host_lower == "github.com" {
        HostKind::GitHub
    } else if host_lower == "bitbucket.org" {
        HostKind::Bitbucket
    } else if !host_lower.is_empty() {
        HostKind::Other
    } else {
        HostKind::None
    };

    let trimmed = raw_path.trim_matches('/');
    let repo_path = trimmed.strip_suffix(".git").unwrap_or(trimmed).to_owned();
    Some(ParsedRemote {
        host_kind,
        repo_path,
    })
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_github_and_bitbucket_urls() {
        for url in [
            "https://github.com/Max/boat.git",
            "git@github.com:Max/boat.git",
            "ssh://git@github.com/Max/boat",
            "https://user@github.com/Max/boat",
        ] {
            assert_eq!(detect_host(url), HostKind::GitHub, "{url}");
            assert_eq!(
                pr_url_for(url, "feature/login"),
                Some("https://github.com/Max/boat/pull/new/feature/login".into())
            );
        }

        for url in [
            "https://max@bitbucket.org/studio/arcade.git",
            "git@bitbucket.org:studio/arcade.git",
            "ssh://git@bitbucket.org:22/studio/arcade.git",
        ] {
            assert_eq!(detect_host(url), HostKind::Bitbucket, "{url}");
            assert_eq!(
                pr_url_for(url, "feature/login"),
                Some(
                    "https://bitbucket.org/studio/arcade/pull-requests/new?source=feature%2Flogin&t=1"
                        .into()
                )
            );
        }

        assert_eq!(detect_host("https://gitlab.com/a/b.git"), HostKind::Other);
        assert_eq!(pr_url_for("https://gitlab.com/a/b.git", "main"), None);
        assert_eq!(detect_host("C:/repos/bare.git"), HostKind::None);
        assert_eq!(detect_host("C:\\repos\\bare.git"), HostKind::None);
    }

    #[test]
    fn encodes_branch_names_safely() {
        assert_eq!(
            pr_url_for("https://github.com/o/r.git", "fix/bug #1"),
            Some("https://github.com/o/r/pull/new/fix/bug%20%231".into())
        );
    }
}
