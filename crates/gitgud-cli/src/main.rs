mod mcp;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gitgud_core::cli::GitCli;
use gitgud_core::git;
use gitgud_core::proposals::{claude_stop_hook_snippet, run_agent_hook, write_claude_stop_hook};
use gitgud_core::repo;
use serde_json::{Value, json};

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

    // The same tools the MCP server offers, with the CLI's flags as their arguments.
    let (tool, args) = tool_call(sub, rest)?;
    let ctx = mcp::Ctx::open(&repo_path)?;
    let out = mcp::execute_tool(&ctx, tool, &args)?;
    match text_of(tool, &out).filter(|_| !json_out) {
        Some(text) => println!("{text}"),
        None => println!(
            "{}",
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
        ),
    }
    Ok(())
}

/// The MCP tool and arguments for CLI command `sub` with flags `rest`.
fn tool_call(sub: &str, rest: &[String]) -> Result<(&'static str, Value), String> {
    let value_after = |idx: usize, what: &str| -> Result<String, String> {
        rest.get(idx + 1).cloned().ok_or_else(|| what.to_owned())
    };
    match sub {
        "status" => Ok(("status", json!({}))),
        "diff" => {
            let mut staged = false;
            let mut path: Option<String> = None;
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "--staged" => staged = true,
                    "--path" => {
                        path = Some(value_after(idx, "--path requires a file")?);
                        idx += 1;
                    }
                    other if path.is_none() => path = Some(other.to_owned()),
                    other => return Err(format!("unexpected argument: {other}")),
                }
                idx += 1;
            }
            let path = path.ok_or("diff requires <path>")?;
            Ok(("diff", json!({ "path": path, "staged": staged })))
        }
        "log" => {
            let mut limit = 20u64;
            let mut idx = 0;
            while idx < rest.len() {
                if rest[idx] == "-n" || rest[idx] == "--limit" {
                    limit = value_after(idx, "-n requires an integer")?
                        .parse()
                        .map_err(|_| "-n requires an integer")?;
                    idx += 1;
                }
                idx += 1;
            }
            Ok(("log", json!({ "limit": limit })))
        }
        "show" => {
            let commit = rest.first().ok_or("show requires <commit>")?;
            Ok(("show", json!({ "commit": commit, "path": rest.get(1) })))
        }
        "propose" => {
            let (mut message, mut why, mut agent) = (String::new(), None, None);
            let (mut paths, mut hunks) = (Vec::new(), Vec::new());
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "-m" | "--message" => message = value_after(idx, "-m requires a message")?,
                    "--why" => why = Some(value_after(idx, "--why requires text")?),
                    "--agent" => agent = Some(value_after(idx, "--agent requires a name")?),
                    "--path" => paths.push(value_after(idx, "--path requires a path")?),
                    "--hunk" => hunks.push(value_after(idx, "--hunk requires an id")?),
                    other => return Err(format!("unknown propose flag: {other}")),
                }
                idx += 2;
            }
            Ok((
                "propose",
                json!({ "message": message, "why": why, "agent": agent, "paths": paths, "hunkIds": hunks }),
            ))
        }
        "proposal" => {
            let mut id: Option<String> = None;
            let mut wait = false;
            let mut timeout_secs = 300u64;
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "--wait" => wait = true,
                    "--timeout" => {
                        timeout_secs = value_after(idx, "--timeout requires seconds")?
                            .parse()
                            .map_err(|_| "--timeout requires an integer number of seconds")?;
                        idx += 1;
                    }
                    other if id.is_none() => id = Some(other.to_owned()),
                    other => return Err(format!("unexpected proposal argument: {other}")),
                }
                idx += 1;
            }
            if wait {
                let id = id.ok_or("proposal --wait requires <id>")?;
                Ok((
                    "proposal_wait",
                    json!({ "id": id, "timeoutSecs": timeout_secs }),
                ))
            } else {
                Ok(("proposal", json!({ "id": id })))
            }
        }
        "withdraw" => {
            let id = rest.first().ok_or("withdraw requires <id>")?;
            Ok(("withdraw", json!({ "id": id })))
        }
        "locks" => match rest.first().map_or("list", String::as_str) {
            "list" => Ok(("locks_list", json!({}))),
            "lock" => {
                let path = rest.get(1).ok_or("locks lock requires <path>")?;
                Ok(("locks_lock", json!({ "path": path })))
            }
            "unlock" => {
                let path = rest.get(1).ok_or("locks unlock requires <path>")?;
                let force = rest.iter().any(|a| a == "--force");
                Ok(("locks_unlock", json!({ "path": path, "force": force })))
            }
            other => Err(format!("unknown locks subcommand: {other}")),
        },
        other => Err(format!("unknown command: {other}")),
    }
}

/// The plain-text answer of `tool` for a person, where it has one (else JSON).
fn text_of(tool: &str, out: &Value) -> Option<String> {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    match tool {
        "status" => {
            let mut lines = vec![format!("Root: {}", s(&out["root"]))];
            for f in list(&out["files"]) {
                let hunks = list(&f["hunks"]);
                lines.push(format!("{} ({} hunks)", s(&f["path"]), hunks.len()));
                for h in hunks {
                    lines.push(format!(
                        "  {} {} {}",
                        s(&h["id"]),
                        s(&h["side"]),
                        s(&h["preview"])
                    ));
                }
            }
            for r in list(&out["riskFlags"]) {
                lines.push(format!(
                    "RISK [{}] {}: {}",
                    s(&r["kind"]),
                    s(&r["path"]),
                    s(&r["detail"])
                ));
            }
            Some(lines.join("\n"))
        }
        "log" => Some(
            list(out)
                .iter()
                .map(|r| {
                    format!(
                        "{} {} ({})",
                        s(&r["shortId"]),
                        s(&r["subject"]),
                        s(&r["author"])
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "propose" => Some(format!(
            "Created proposal {} ({})",
            s(&out["id"]),
            s(&out["shortCommit"])
        )),
        "locks_lock" => Some(format!("Locked {}", s(&out["locked"]))),
        "locks_unlock" => Some(format!("Unlocked {}", s(&out["unlocked"]))),
        _ => None,
    }
}

fn handle_agent(repo_root: &Path, args: &[String], json_out: bool) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("setup") => handle_agent_setup(repo_root, &args[1..], json_out),
        Some("hook") => handle_agent_hook(repo_root, &args[1..], json_out),
        _ => Err(
            "expected `agent setup [--target claude|codex|cursor|generic] [--write] [--with-hooks]` or `agent hook [--event stop]`"
                .into(),
        ),
    }
}

fn handle_agent_setup(repo_root: &Path, args: &[String], json_out: bool) -> Result<(), String> {
    let mut target = "generic";
    let mut write_file = false;
    let mut with_hooks = false;
    let mut i = 0;
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
            "--with-hooks" => with_hooks = true,
            other => return Err(format!("unknown agent setup flag: {other}")),
        }
        i += 1;
    }
    if with_hooks && target != "claude" {
        return Err("--with-hooks currently supports --target claude only".into());
    }

    let skill_text = gitgud_core::proposals::agent_setup_text();
    let rel = gitgud_core::proposals::agent_setup_rel_path(target);
    let dest = repo_root.join(rel);

    if write_file {
        gitgud_core::proposals::write_agent_setup(repo_root, target).map_err(|e| e.to_string())?;
    }
    let hook_rel = if with_hooks && write_file {
        Some(write_claude_stop_hook(repo_root).map_err(|e| e.to_string())?)
    } else {
        None
    };

    if json_out {
        println!(
            "{}",
            serde_json::json!({
                "target": target,
                "path": dest.to_string_lossy(),
                "written": write_file,
                "content": skill_text,
                "withHooks": with_hooks,
                "hookPath": hook_rel.as_deref(),
                "hookSnippet": if with_hooks { Some(claude_stop_hook_snippet()) } else { None },
            })
        );
    } else if write_file {
        println!("Wrote {}", dest.display());
        if let Some(hr) = &hook_rel {
            println!("Updated {}", repo_root.join(hr).display());
        }
    } else {
        println!("Target: {}\n\n{skill_text}", dest.display());
        if with_hooks {
            println!("\nHook snippet:\n{}", claude_stop_hook_snippet());
        }
    }
    Ok(())
}

fn handle_agent_hook(repo_root: &Path, args: &[String], json_out: bool) -> Result<(), String> {
    let mut event = "stop";
    let mut agent = "claude";
    let mut message: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--event" => {
                i += 1;
                event = args
                    .get(i)
                    .map(String::as_str)
                    .ok_or("--event requires a value")?;
            }
            "--agent" => {
                i += 1;
                agent = args
                    .get(i)
                    .map(String::as_str)
                    .ok_or("--agent requires a value")?;
            }
            "-m" | "--message" => {
                i += 1;
                message = Some(args.get(i).ok_or("-m requires a message")?.clone());
            }
            other => return Err(format!("unknown agent hook flag: {other}")),
        }
        i += 1;
    }
    if event != "stop" {
        return Err(format!("unsupported hook event: {event} (supported: stop)"));
    }

    let opened = repo::open(repo_root).map_err(|e| e.to_string())?;
    let git_cli = GitCli::new(&git::locate().map_err(|e| e.to_string())?);
    let created =
        run_agent_hook(&opened, &git_cli, agent, message.as_deref()).map_err(|e| e.to_string())?;

    if json_out {
        match &created {
            Some(p) => println!("{}", serde_json::json!({ "created": true, "proposal": p })),
            None => println!(
                "{}",
                serde_json::json!({ "created": false, "reason": "no_new_changes" })
            ),
        }
    } else if let Some(p) = &created {
        println!("Created proposal {} ({})", p.id, p.short_commit);
    } else {
        println!("No new changes to propose");
    }
    Ok(())
}

fn print_usage() {
    println!(
        "\
gitgud-cli [--repo <path>] [--json] <command>
  status | diff [--staged] [--path <file>] | log [-n <N>] | show <commit> [<path>]
  propose -m <msg> [--why <why>] [--agent <name>] [--path <p>...] [--hunk <id>...]
  proposal [<id>] [--wait] [--timeout <secs>] | withdraw <id>
  locks [list | lock <path> | unlock <path> [--force]]
  agent setup [--target claude|codex|cursor|generic] [--write] [--with-hooks]
  agent hook [--event stop] [--agent <name>] [-m <msg>]
  mcp [--repo <path>]"
    );
}
