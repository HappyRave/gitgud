use std::env;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, no_console_window};
use crate::repo::{Error, Repo};

const MAX_DIFF_BYTES: usize = 14_000;
const CLI_CANDIDATES: &[&str] = &["claude", "codex", "gemini"];
const ENV_KEY_VARS: &[&str] = &["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY"];

/// Availability of non-interactive coding CLIs and API key env vars (boolean only; keys are never read or exposed).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderStatus {
    pub cli_available: Vec<String>,
    pub configured_env_key: bool,
}

/// Detects installed AI CLI tools on `PATH` and whether standard API key env vars are set.
#[must_use]
pub fn detect_ai_providers() -> AiProviderStatus {
    let cli_available = CLI_CANDIDATES
        .iter()
        .filter(|name| which::which(name).is_ok())
        .map(|s| (*s).to_owned())
        .collect();
    let configured_env_key = ENV_KEY_VARS.iter().any(|var| env::var_os(var).is_some());
    AiProviderStatus {
        cli_available,
        configured_env_key,
    }
}

/// Builds a concise commit-message prompt from `--cached` diff stat and patch, bounded safely on UTF-8 boundaries.
#[must_use]
pub fn build_commit_prompt(stat: &str, diff: &str) -> String {
    let mut clipped = diff;
    if clipped.len() > MAX_DIFF_BYTES {
        let mut end = MAX_DIFF_BYTES;
        while end > 0 && !clipped.is_char_boundary(end) {
            end -= 1;
        }
        clipped = &clipped[..end];
    }
    format!(
        "Write a git commit message for the staged changes below.\n\
         Rules:\n\
         - Line 1: concise imperative subject under 72 characters.\n\
         - Optional blank line + short bullet body only if genuinely needed.\n\
         - Output ONLY plain commit message text (no code fences, no quotes, no commentary).\n\n\
         Stat:\n{stat}\n\nDiff:\n{clipped}"
    )
}

/// Strips accidental markdown fences or surrounding quotes returned by an AI CLI.
#[must_use]
pub fn sanitize_commit_message(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("```") {
        let after_lang = rest.split_once('\n').map_or(rest, |(_, body)| body);
        if let Some(inner) = after_lang.strip_suffix("```") {
            s = inner.trim();
        }
    }
    if ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
        && s.len() >= 2
    {
        s = s[1..s.len() - 1].trim();
    }
    s.to_owned()
}

/// Drafts a commit message for currently staged changes using an installed AI CLI (`claude`, `codex`, or `gemini`).
///
/// # Errors
/// [`Error::Refused`] when nothing is staged or no provider is available; [`Error::Command`] when git or the AI CLI fails.
pub fn draft_commit_message(
    git: &GitCli,
    repo: &Repo,
    provider: Option<&str>,
) -> Result<String, Error> {
    let stat = git.run(repo.root(), &["diff", "--cached", "--stat"], None)?;
    let diff = git.run(repo.root(), &["diff", "--cached"], None)?;
    if stat.trim().is_empty() && diff.trim().is_empty() {
        return Err(Error::Refused(
            "Stage at least one file before drafting a commit message".into(),
        ));
    }

    let status = detect_ai_providers();
    let requested = provider
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"));
    let chosen = requested
        .or_else(|| status.cli_available.first().map(String::as_str))
        .ok_or_else(|| {
            Error::Refused(
                "No AI CLI found on PATH (claude, codex, or gemini). Install one to draft commit messages."
                    .into(),
            )
        })?;

    let prompt = build_commit_prompt(&stat, &diff);
    let args: Vec<&str> = match chosen {
        "claude" => vec!["-p", &prompt],
        "codex" => vec!["exec", &prompt],
        "gemini" => vec!["-p", &prompt],
        other => {
            return Err(Error::Refused(format!(
                "Unsupported AI provider '{other}'; expected claude, codex, or gemini"
            )));
        }
    };

    let mut cmd = Command::new(chosen);
    cmd.current_dir(repo.root()).args(&args);
    no_console_window(&mut cmd);
    let out = cmd.output().map_err(|e| Error::Command {
        command: chosen.to_owned(),
        message: e.to_string(),
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        return Err(Error::Command {
            command: chosen.to_owned(),
            message: if stderr.is_empty() { stdout } else { stderr },
        });
    }

    let msg = sanitize_commit_message(&String::from_utf8_lossy(&out.stdout));
    if msg.is_empty() {
        return Err(Error::Refused(format!(
            "{chosen} returned an empty commit message"
        )));
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GitCli;
    use crate::git;
    use crate::repo;
    use crate::testing::TestRepo;

    #[test]
    fn truncates_diff_safely_on_utf8_boundary() {
        let diff = "é".repeat(MAX_DIFF_BYTES);
        let prompt = build_commit_prompt(" 1 file changed", &diff);
        assert!(prompt.len() <= MAX_DIFF_BYTES + 400);
        assert!(prompt.contains("Diff:\n"));
    }

    #[test]
    fn sanitizes_fenced_and_quoted_messages() {
        assert_eq!(
            sanitize_commit_message("```text\nAdd stacked branches\n```"),
            "Add stacked branches"
        );
        assert_eq!(
            sanitize_commit_message("  \"Fix camera jitter\"  "),
            "Fix camera jitter"
        );
    }

    #[test]
    fn refuses_to_draft_without_staged_changes() {
        let tr = TestRepo::init();
        tr.write("a.txt", "hello\n").commit_all("init");
        let repo = repo::open(tr.path()).unwrap();
        let cli = GitCli::new(&git::locate().unwrap());
        let err = draft_commit_message(&cli, &repo, Some("claude")).unwrap_err();
        assert!(matches!(err, Error::Refused(_)), "{err:?}");
    }
}
