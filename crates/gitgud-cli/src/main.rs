mod mcp;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gitgud_core::cli::GitCli;
use gitgud_core::git;
use gitgud_core::graph::Graph;
use gitgud_core::lfs;
use gitgud_core::proposals::{
    CreateProposalInput, agent_status, create_proposal, get_proposal, list_proposals,
    withdraw_proposal,
};
use gitgud_core::repo;
use gitgud_core::worktree::Side;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("gitgud-cli error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    let mut repo_path = PathBuf::from(".");
    let mut json_out = false;

    let mut pos = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--repo" | "-C" => {
                i += 1;
                let val = raw.get(i).ok_or("--repo requires a path")?;
                repo_path = PathBuf::from(val);
            }
            "--json" => {
                json_out = true;
            }
            _ => pos.push(std::mem::take(&mut raw[i])),
        }
        i += 1;
    }

    if pos.is_empty() || matches!(pos[0].as_str(), "--help" | "-h" | "help") {
        print_usage();
        return Ok(());
    }

    let sub = pos[0].as_str();
    let rest = &pos[1..];
    if sub == "mcp" {
        return mcp::serve_stdio(&repo_path);
    }

    if sub == "agent" {
        return handle_agent(&repo_path, rest, json_out);
    }

    let opened = repo::open(&repo_path).map_err(|e| e.to_string())?;
    let git_cli = GitCli::new(&git::locate().map_err(|e| e.to_string())?);

    match sub {
        "status" => {
            let st = agent_status(&opened, &git_cli).map_err(|e| e.to_string())?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?
                );
            } else {
                println!("Root: {}", st.root);
                for f in &st.files {
                    println!("{} ({} hunks)", f.path, f.hunks.len());
                    for h in &f.hunks {
                        println!("  {} {:?} {}", h.id, h.side, h.preview);
                    }
                }
                for rf in &st.risk_flags {
                    println!("RISK [{}] {}: {}", rf.kind, rf.path, rf.detail);
                }
            }
        }
        "diff" => {
            let mut staged = false;
            let mut file_path: Option<String> = None;
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "--staged" => staged = true,
                    "--path" => {
                        idx += 1;
                        file_path = Some(rest.get(idx).ok_or("--path requires a file")?.clone());
                    }
                    other if file_path.is_none() => file_path = Some(other.to_owned()),
                    other => return Err(format!("unexpected argument: {other}")),
                }
                idx += 1;
            }
            let p = file_path.ok_or("diff requires <path>")?;
            let side = if staged { Side::Staged } else { Side::Unstaged };
            let d = opened
                .worktree_file_diff(&p, side)
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&d).map_err(|e| e.to_string())?
            );
        }
        "log" => {
            let mut limit = 20usize;
            let mut idx = 0;
            while idx < rest.len() {
                if rest[idx] == "-n" || rest[idx] == "--limit" {
                    idx += 1;
                    limit = rest
                        .get(idx)
                        .and_then(|s| s.parse().ok())
                        .ok_or("-n requires an integer")?;
                }
                idx += 1;
            }
            let g = Graph::build(&opened, false).map_err(|e| e.to_string())?;
            let rows = g
                .rows(&opened, 0, limit.min(g.len()))
                .map_err(|e| e.to_string())?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
                );
            } else {
                for r in rows {
                    println!("{} {} ({})", r.short_id, r.subject, r.author);
                }
            }
        }
        "show" => {
            let commit = rest.first().ok_or("show requires <commit>")?;
            if let Some(p) = rest.get(1) {
                let d = opened
                    .commit_file_diff(commit, p)
                    .map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&d).map_err(|e| e.to_string())?
                );
            } else {
                let d = opened.commit_detail(commit).map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&d).map_err(|e| e.to_string())?
                );
            }
        }
        "propose" => {
            let mut input = CreateProposalInput::default();
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "-m" | "--message" => {
                        idx += 1;
                        input.message = rest.get(idx).ok_or("-m requires a message")?.clone();
                    }
                    "--why" => {
                        idx += 1;
                        input.why = Some(rest.get(idx).ok_or("--why requires text")?.clone());
                    }
                    "--agent" => {
                        idx += 1;
                        input.agent = Some(rest.get(idx).ok_or("--agent requires a name")?.clone());
                    }
                    "--path" => {
                        idx += 1;
                        input
                            .paths
                            .push(rest.get(idx).ok_or("--path requires a path")?.clone());
                    }
                    "--hunk" => {
                        idx += 1;
                        input
                            .hunk_ids
                            .push(rest.get(idx).ok_or("--hunk requires an id")?.clone());
                    }
                    other => return Err(format!("unknown propose flag: {other}")),
                }
                idx += 1;
            }
            let summary = create_proposal(&opened, &git_cli, &input).map_err(|e| e.to_string())?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
                );
            } else {
                println!("Created proposal {} ({})", summary.id, summary.short_commit);
            }
        }
        "proposal" => {
            if let Some(id) = rest.first() {
                let p = get_proposal(&opened, id).map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?
                );
            } else {
                let list = list_proposals(&opened).map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?
                );
            }
        }
        "withdraw" => {
            let id = rest.first().ok_or("withdraw requires <id>")?;
            let w = withdraw_proposal(&opened, &git_cli, id).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&w).map_err(|e| e.to_string())?
            );
        }
        "locks" => {
            let action = rest.first().map(String::as_str).unwrap_or("list");
            match action {
                "list" => {
                    let list = lfs::lfs_locks_list(&git_cli, &opened).map_err(|e| e.to_string())?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?
                    );
                }
                "lock" => {
                    let p = rest.get(1).ok_or("locks lock requires <path>")?;
                    lfs::lfs_lock(&git_cli, &opened, p).map_err(|e| e.to_string())?;
                    if json_out {
                        println!(r#"{{"locked":"{p}"}}"#);
                    } else {
                        println!("Locked {p}");
                    }
                }
                "unlock" => {
                    let p = rest.get(1).ok_or("locks unlock requires <path>")?;
                    let force = rest.iter().any(|a| a == "--force");
                    lfs::lfs_unlock(&git_cli, &opened, p, force).map_err(|e| e.to_string())?;
                    if json_out {
                        println!(r#"{{"unlocked":"{p}"}}"#);
                    } else {
                        println!("Unlocked {p}");
                    }
                }
                other => return Err(format!("unknown locks subcommand: {other}")),
            }
        }
        other => return Err(format!("unknown command: {other}")),
    }

    Ok(())
}

fn handle_agent(repo_root: &Path, args: &[String], json_out: bool) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("setup") {
        return Err(
            "expected `agent setup [--target claude|codex|cursor|generic] [--write]`".into(),
        );
    }
    let mut target = "generic";
    let mut write_file = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                target = args
                    .get(i)
                    .map(String::as_str)
                    .ok_or("--target requires a value")?;
            }
            "--write" => write_file = true,
            other => return Err(format!("unknown agent setup flag: {other}")),
        }
        i += 1;
    }

    let skill_text = "\
# gitgud Agent Workflow Instructions

Never commit, checkout, merge, rebase, or push directly with `git` when working in a `gitgud` repository.
Instead, stage and propose changes cleanly for human review inside gitgud:

1. Inspect status & stable hunk IDs: `gitgud-cli status --json` (or MCP `status`).
2. Propose logical commits without modifying the user's index:
   `gitgud-cli propose -m \"<commit message>\" --why \"<why & test evidence>\" --agent <name> --json`
3. Check human feedback or approval status:
   `gitgud-cli proposal <id> --json` (or MCP `proposal`).
4. For Unity binary/LFS assets, acquire locks via `gitgud-cli locks lock <path>`.
";

    let dest = match target {
        "claude" => repo_root.join(".claude/skills/gitgud/SKILL.md"),
        "codex" => repo_root.join(".codex/skills/gitgud/SKILL.md"),
        "cursor" => repo_root.join(".cursor/rules/gitgud.mdc"),
        _ => repo_root.join("GITGUD_AGENT.md"),
    };

    if write_file {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&dest, skill_text).map_err(|e| e.to_string())?;
    }

    if json_out {
        println!(
            "{}",
            serde_json::json!({
                "target": target,
                "path": dest.to_string_lossy(),
                "written": write_file,
                "content": skill_text
            })
        );
    } else if write_file {
        println!("Wrote {}", dest.display());
    } else {
        println!("Target: {}\n\n{skill_text}", dest.display());
    }
    Ok(())
}

fn print_usage() {
    println!(
        "\
gitgud-cli [--repo <path>] [--json] <command>
  status | diff [--staged] [--path <file>] | log [-n <N>] | show <commit> [<path>]
  propose -m <msg> [--why <why>] [--agent <name>] [--path <p>...] [--hunk <id>...]
  proposal [<id>] | withdraw <id>
  locks [list | lock <path> | unlock <path> [--force]]
  agent setup [--target claude|codex|cursor|generic] [--write]
  mcp [--repo <path>]"
    );
}
