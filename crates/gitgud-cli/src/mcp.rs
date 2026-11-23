use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Duration;

use gitgud_core::cli::GitCli;
use gitgud_core::git;
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

/// Runs a line-delimited JSON-RPC 2.0 MCP stdio server against `repo_root`.
/// Exposes ONLY read + proposal + lock tools (no commit, checkout, merge, rebase, or push).
pub fn serve_stdio(repo_root: &Path) -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line_res in stdin.lock().lines() {
        let line = line_res.map_err(|e| e.to_string())?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: JsonRpcReq = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                writeln!(out, "{err}").map_err(|e| e.to_string())?;
                out.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };
        if let Some(resp) = handle_request(repo_root, req) {
            writeln!(out, "{resp}").map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn handle_request(repo_root: &Path, req: JsonRpcReq) -> Option<Value> {
    let _ = req.jsonrpc;
    // Notifications have no `id`.
    let id = req.id?;

    let reply = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "gitgud-mcp", "version": "0.1.0" }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let name = req.params["name"].as_str().unwrap_or("");
            let args = &req.params["arguments"];
            Ok(call_tool(repo_root, name, args))
        }
        other => Err((-32601, format!("Method not found: {other}"))),
    };

    Some(match reply {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

fn call_tool(repo_root: &Path, name: &str, args: &Value) -> Value {
    match execute_tool(repo_root, name, args) {
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

fn open_repo_and_git(repo_root: &Path) -> Result<(Repo, GitCli), String> {
    let opened = repo::open(repo_root).map_err(|e| e.to_string())?;
    let git_cli = GitCli::new(&git::locate().map_err(|e| e.to_string())?);
    Ok((opened, git_cli))
}

fn execute_tool(repo_root: &Path, name: &str, args: &Value) -> Result<Value, String> {
    let (opened, git_cli) = open_repo_and_git(repo_root)?;
    match name {
        "status" => {
            let st = agent_status(&opened, &git_cli).map_err(|e| e.to_string())?;
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
            let g = gitgud_core::graph::Graph::build(&opened, false).map_err(|e| e.to_string())?;
            let rows = g
                .rows(&opened, 0, count.min(g.len()))
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
            let created = create_proposal(&opened, &git_cli, &input).map_err(|e| e.to_string())?;
            serde_json::to_value(created).map_err(|e| e.to_string())
        }
        "proposal" => {
            if let Some(id) = args["id"].as_str() {
                let p = get_proposal(&opened, id).map_err(|e| e.to_string())?;
                serde_json::to_value(p).map_err(|e| e.to_string())
            } else {
                let list = list_proposals(&opened).map_err(|e| e.to_string())?;
                serde_json::to_value(list).map_err(|e| e.to_string())
            }
        }
        "proposal_wait" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| "missing required argument: id".to_owned())?;
            let timeout_secs = args["timeoutSecs"].as_u64().unwrap_or(300).clamp(1, 3600);
            let waited = wait_for_proposal(&opened, id, timeout_secs, Duration::from_millis(250))
                .map_err(|e| e.to_string())?;
            serde_json::to_value(waited).map_err(|e| e.to_string())
        }
        "withdraw" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| "missing required argument: id".to_owned())?;
            let w = withdraw_proposal(&opened, &git_cli, id).map_err(|e| e.to_string())?;
            serde_json::to_value(w).map_err(|e| e.to_string())
        }
        "locks_list" => {
            let list = lfs::lfs_locks_list(&git_cli, &opened).map_err(|e| e.to_string())?;
            serde_json::to_value(list).map_err(|e| e.to_string())
        }
        "locks_lock" => {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| "missing required argument: path".to_owned())?;
            lfs::lfs_lock(&git_cli, &opened, path).map_err(|e| e.to_string())?;
            Ok(json!({ "locked": path }))
        }
        "locks_unlock" => {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| "missing required argument: path".to_owned())?;
            let force = args["force"].as_bool() == Some(true);
            lfs::lfs_unlock(&git_cli, &opened, path, force).map_err(|e| e.to_string())?;
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
