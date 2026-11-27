use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

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

/// What the AI CLI is asked; the staged changes ([`build_commit_prompt`]) come on stdin.
const INSTRUCTIONS: &str = "Write a git commit message for the staged changes given on stdin. \
Line 1: a concise imperative subject under 72 characters. Add a blank line and a short bullet \
body only if genuinely needed. Output ONLY the plain commit message text: no code fences, no \
quotes, no commentary.";

/// The staged changes for the prompt: `--cached` diff stat and patch, the patch cut
/// to a bounded size on a UTF-8 boundary.
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
    format!("Stat:\n{stat}\n\nDiff:\n{clipped}")
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
    // Plain diffs, whatever color.diff or diff.external the user set.
    let stat = git.run(
        repo.root(),
        &["diff", "--cached", "--stat", "--no-color", "--no-ext-diff"],
        None,
    )?;
    let diff = git.run(
        repo.root(),
        &["diff", "--cached", "--no-color", "--no-ext-diff"],
        None,
    )?;
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

    let changes = build_commit_prompt(&stat, &diff);
    let full = format!("{INSTRUCTIONS}\n\n{changes}");
    // The changes go on stdin: npm's .cmd shims run through cmd.exe, whose
    // command line stops at 8,191 characters.
    let (args, stdin): (Vec<&str>, &str) = match chosen {
        "claude" | "gemini" => (vec!["-p", INSTRUCTIONS], &changes),
        "codex" => (vec!["exec", "-"], &full),
        other => {
            return Err(Error::Refused(format!(
                "Unsupported AI provider '{other}'; expected claude, codex, or gemini"
            )));
        }
    };
    // `which` finds `codex.cmd`; `Command::new("codex")` would only look for codex.exe.
    let program = which::which(chosen).map_err(|e| Error::Command {
        command: chosen.to_owned(),
        message: e.to_string(),
    })?;
    let out = run_cli(chosen, &program, &args, stdin)?;
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

/// Runs an AI CLI with `stdin` piped in, from a folder of gitgud's own: run in the
/// repo, a cloned repo's agent settings and hooks would apply.
fn run_cli(
    name: &str,
    program: &std::path::Path,
    args: &[&str],
    stdin: &str,
) -> Result<std::process::Output, Error> {
    let cwd = env::temp_dir().join("gitgud-ai");
    std::fs::create_dir_all(&cwd).map_err(crate::repo::read_err)?;
    let mut cmd = Command::new(program);
    cmd.current_dir(&cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_console_window(&mut cmd);
    let spawn_err = |e: std::io::Error| Error::Command {
        command: name.to_owned(),
        message: e.to_string(),
    };
    let mut child = cmd.spawn().map_err(spawn_err)?;
    std::thread::scope(|s| {
        if let Some(mut pipe) = child.stdin.take() {
            s.spawn(move || pipe.write_all(stdin.as_bytes()));
        }
        child.wait_with_output()
    })
    .map_err(spawn_err)
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

    /// An npm-style `.cmd` shim, as codex and gemini install, gets a prompt far
    /// past cmd.exe's command-line limit, and runs outside the repo.
    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_gets_a_long_prompt_on_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("fake-ai.cmd");
        std::fs::write(&shim, "@cd\r\n@findstr \"^\"\r\n").unwrap();
        let prompt = format!("{}\nend\n", "x".repeat(20_000));

        let out = run_cli("fake-ai", &shim, &["-p", INSTRUCTIONS], &prompt).unwrap();

        assert!(out.status.success(), "{out:?}");
        let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
        let (cwd, echoed) = text.split_once('\n').unwrap();
        assert!(cwd.ends_with("gitgud-ai"), "{cwd}");
        assert_eq!(echoed, prompt);
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
