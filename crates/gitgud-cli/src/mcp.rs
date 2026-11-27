use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gitgud_core::cli::GitCli;
use gitgud_core::git;
use gitgud_core::graph::Graph;
use gitgud_core::lfs;
use gitgud_core::proposals::{
    CreateProposalInput, agent_status, create_proposal, get_proposal, list_proposals,
    wait_for_proposal, withdraw_proposal,
};
use gitgud_core::repo::{self, Repo};
use gitgud_core::worktree::Side;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct JsonRpcReq {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// The repository a server answers for, opened once: re-opening it and locating
/// git (a process) on every call made each tool call slow.
struct Ctx {
    repo: Repo,
    git: GitCli,
    /// The history graph `log` reads, built again only once refs or HEAD move.
    graph: Mutex<Option<Graph>>,
}

/// Runs a line-delimited JSON-RPC 2.0 MCP stdio server against `repo_root`.
/// Exposes ONLY read + proposal + lock tools (no commit, checkout, merge, rebase, or push).
/// Each tool call runs on its own thread, so a `proposal_wait` that blocks for
/// minutes doesn't stop `ping` or other calls from being answered.
pub fn serve_stdio(repo_root: &Path) -> Result<(), String> {
    let ctx = Arc::new(Ctx {
        repo: repo::open(repo_root).map_err(|e| e.to_string())?,
        git: GitCli::new(&git::locate().map_err(|e| e.to_string())?),
        graph: Mutex::new(None),
    });
    let out = Arc::new(Mutex::new(io::BufWriter::new(io::stdout())));
    let send = |out: &Mutex<io::BufWriter<io::Stdout>>, msg: &Value| -> Result<(), String> {
        let mut out = out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        writeln!(out, "{msg}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())
    };

    std::thread::scope(|s| {
        for line in io::stdin().lock().split(b'\n') {
            let line = line.map_err(|e| e.to_string())?;
            // A line that isn't UTF-8 is a parse error, not the end of the server.
            let text = String::from_utf8_lossy(&line);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            let req = match parse_request(trimmed) {
                Ok(r) => r,
                Err(err) => {
                    send(&out, &err)?;
                    continue;
                }
            };
            if req.method == "tools/call" {
                let (ctx, out) = (ctx.clone(), out.clone());
                s.spawn(move || {
                    if let Some(resp) = handle_request(&ctx, req)
                        && let Err(e) = send(&out, &resp)
                    {
                        eprintln!("gitgud mcp: cannot answer: {e}");
                    }
                });
            } else if let Some(resp) = handle_request(&ctx, req) {
                send(&out, &resp)?;
            }
        }
        Ok(())
    })
}

/// A request, or the JSON-RPC error to answer when it isn't one.
fn parse_request(text: &str) -> Result<JsonRpcReq, Value> {
    let error = |code: i32, message: String| json!({ "jsonrpc": "2.0", "id": null, "error": { "code": code, "message": message } });
    let value: Value =
        serde_json::from_str(text).map_err(|e| error(-32700, format!("Parse error: {e}")))?;
    serde_json::from_value(value).map_err(|e| error(-32600, format!("Invalid request: {e}")))
}

fn handle_request(ctx: &Ctx, req: JsonRpcReq) -> Option<Value> {
    let _ = req.jsonrpc;
    // Notifications have no `id`.
    let id = req.id?;

    let reply = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "gitgud-mcp", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => match req.params["name"].as_str() {
            Some(name) => Ok(call_tool(ctx, name, &req.params["arguments"])),
            None => Err((-32602, "tools/call needs a tool name".to_owned())),
        },
        other => Err((-32601, format!("Method not found: {other}"))),
    };

    Some(match reply {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

fn call_tool(ctx: &Ctx, name: &str, args: &Value) -> Value {
    match execute_tool(ctx, name, args) {
        Ok(payload) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&payload).unwrap_or_default() }],
            "structuredContent": payload
        }),
        Err(err) => json!({
            "isError": true,
            "content": [{ "type": "text", "text": err }]
        }),
    }
}

fn execute_tool(ctx: &Ctx, name: &str, args: &Value) -> Result<Value, String> {
    let (opened, git_cli) = (&ctx.repo, &ctx.git);
    match name {
        "status" => {
            let st = agent_status(opened, git_cli).map_err(|e| e.to_string())?;
            serde_json::to_value(st).map_err(|e| e.to_string())
        }
        "diff" => {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| "missing required argument: path".to_owned())?;
            let side = if args["staged"].as_bool() == Some(true) {
                Side::Staged
            } else {
                Side::Unstaged
            };
            let d = opened
                .worktree_file_diff(path, side)
                .map_err(|e| e.to_string())?;
            serde_json::to_value(d).map_err(|e| e.to_string())
        }
        "log" => {
            let count = usize::try_from(args["limit"].as_u64().unwrap_or(20)).unwrap_or(20);
            let mut graph = ctx
                .graph
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current = match graph.as_ref() {
                Some(g) => g.is_current(opened, false).map_err(|e| e.to_string())?,
                None => false,
            };
            if !current {
                *graph = Some(Graph::build(opened, false).map_err(|e| e.to_string())?);
            }
            let g = graph.as_ref().ok_or("no graph")?;
            let rows = g
                .rows(opened, 0, count.min(g.len()))
                .map_err(|e| e.to_string())?;
            serde_json::to_value(rows).map_err(|e| e.to_string())
        }
        "show" => {
            let commit = args["commit"]
                .as_str()
                .ok_or_else(|| "missing required argument: commit".to_owned())?;
            if let Some(path) = args["path"].as_str() {
                let d = opened
                    .commit_file_diff(commit, path)
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(d).map_err(|e| e.to_string())
            } else {
                let detail = opened.commit_detail(commit).map_err(|e| e.to_string())?;
                serde_json::to_value(detail).map_err(|e| e.to_string())
            }
        }
        "propose" => {
            let input: CreateProposalInput =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            let created = create_proposal(opened, git_cli, &input).map_err(|e| e.to_string())?;
            serde_json::to_value(created).map_err(|e| e.to_string())
        }
        "proposal" => {
            if let Some(id) = args["id"].as_str() {
                let p = get_proposal(opened, id).map_err(|e| e.to_string())?;
                serde_json::to_value(p).map_err(|e| e.to_string())
            } else {
                let list = list_proposals(opened).map_err(|e| e.to_string())?;
                serde_json::to_value(list).map_err(|e| e.to_string())
            }
        }
        "proposal_wait" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| "missing required argument: id".to_owned())?;
            let timeout_secs = args["timeoutSecs"].as_u64().unwrap_or(300).clamp(1, 3600);
            let waited = wait_for_proposal(opened, id, timeout_secs, Duration::from_millis(250))
                .map_err(|e| e.to_string())?;
            serde_json::to_value(waited).map_err(|e| e.to_string())
        }
        "withdraw" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| "missing required argument: id".to_owned())?;
            let w = withdraw_proposal(opened, git_cli, id).map_err(|e| e.to_string())?;
            serde_json::to_value(w).map_err(|e| e.to_string())
        }
        "locks_list" => {
            let list = lfs::lfs_locks_list(git_cli, opened).map_err(|e| e.to_string())?;
            serde_json::to_value(list).map_err(|e| e.to_string())
        }
        "locks_lock" => {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| "missing required argument: path".to_owned())?;
            lfs::lfs_lock(git_cli, opened, path).map_err(|e| e.to_string())?;
            Ok(json!({ "locked": path }))
        }
        "locks_unlock" => {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| "missing required argument: path".to_owned())?;
            let force = args["force"].as_bool() == Some(true);
            lfs::lfs_unlock(git_cli, opened, path, force).map_err(|e| e.to_string())?;
            Ok(json!({ "unlocked": path }))
        }
        unknown => Err(format!("Unknown or disallowed tool: {unknown}")),
    }
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "status",
            "description": "Inspect working tree & staged changes with stable hunk IDs and Unity/LFS risk flags (no git add -N).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "diff",
            "description": "Read diff for a working-copy or staged file.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "staged": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "log",
            "description": "List recent commit rows from git history.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer" } }
            }
        }),
        json!({
            "name": "show",
            "description": "Show commit metadata or file diff in a commit.",
            "inputSchema": {
                "type": "object",
                "required": ["commit"],
                "properties": {
                    "commit": { "type": "string" },
                    "path": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "propose",
            "description": "Stage hunks or paths into an isolated hidden ref refs/gitgud/proposals/<id> for human review in gitgud.",
            "inputSchema": {
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": { "type": "string" },
                    "why": { "type": "string" },
                    "agent": { "type": "string" },
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "hunkIds": { "type": "array", "items": { "type": "string" } }
                }
            }
        }),
        json!({
            "name": "proposal",
            "description": "Check proposal approval status and human feedback comments.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "string" } }
            }
        }),
        json!({
            "name": "proposal_wait",
            "description": "Block up to timeoutSecs waiting for human verdict (approved, changesRequested, rejected, withdrawn) on a pending proposal.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "timeoutSecs": { "type": "integer", "minimum": 1, "maximum": 3600 }
                }
            }
        }),
        json!({
            "name": "withdraw",
            "description": "Withdraw an active proposal and clean up its hidden ref.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        }),
        json!({
            "name": "locks_list",
            "description": "List Git LFS file locks on the remote.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "locks_lock",
            "description": "Lock a binary/LFS asset path via git lfs lock.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": { "path": { "type": "string" } }
            }
        }),
        json!({
            "name": "locks_unlock",
            "description": "Unlock a Git LFS asset path.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "force": { "type": "boolean" }
                }
            }
        }),
    ]
}
