use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use gitgud_core::testing::TestRepo;
use serde_json::{Value, json};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_gitgud-cli")
}

#[test]
fn cli_status_propose_list_withdraw_and_agent_setup() {
    let repo = TestRepo::init();
    repo.write("a.txt", "v1\n").commit_all("initial");
    repo.write("a.txt", "v2\n");

    let out = Command::new(bin())
        .args(["--repo", repo.path().to_str().unwrap(), "--json", "status"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let st: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(st["files"][0]["path"], "a.txt");
    let hunk_id = st["files"][0]["hunks"][0]["id"].as_str().unwrap();

    let prop_out = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "propose",
            "-m",
            "feat: cli proposal",
            "--why",
            "from cli test",
            "--agent",
            "cli",
            "--hunk",
            hunk_id,
        ])
        .output()
        .unwrap();
    assert!(
        prop_out.status.success(),
        "{}",
        String::from_utf8_lossy(&prop_out.stderr)
    );
    let prop: Value = serde_json::from_slice(&prop_out.stdout).unwrap();
    assert_eq!(prop["status"], "pending");
    let pid = prop["id"].as_str().unwrap();

    let wd_out = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "withdraw",
            pid,
        ])
        .output()
        .unwrap();
    assert!(wd_out.status.success());
    let wd: Value = serde_json::from_slice(&wd_out.stdout).unwrap();
    assert_eq!(wd["status"], "withdrawn");

    let setup_out = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "agent",
            "setup",
            "--target",
            "generic",
            "--write",
        ])
        .output()
        .unwrap();
    assert!(setup_out.status.success());
    assert!(repo.path().join("GITGUD_AGENT.md").is_file());
}

#[test]
fn mcp_stdio_exposes_safe_tools_and_creates_proposals() {
    let repo = TestRepo::init();
    repo.write("f.txt", "before\n").commit_all("initial");
    repo.write("f.txt", "after\n");

    let mut child = Command::new(bin())
        .args(["--repo", repo.path().to_str().unwrap(), "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    let mut rpc = |req: Value| -> Value {
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };

    let init = rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }));
    assert_eq!(init["result"]["serverInfo"]["name"], "gitgud-mcp");

    let list = rpc(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"status"));
    assert!(names.contains(&"propose"));
    assert!(names.contains(&"proposal_wait"));
    assert!(!names.contains(&"commit"));
    assert!(!names.contains(&"push"));

    let prop_call = rpc(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "propose",
            "arguments": { "message": "feat: mcp proposal", "paths": ["f.txt"] }
        }
    }));
    assert_eq!(
        prop_call["result"]["structuredContent"]["status"],
        "pending"
    );
    let mcp_pid = prop_call["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let wd_call = rpc(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "withdraw",
            "arguments": { "id": mcp_pid }
        }
    }));
    assert_eq!(
        wd_call["result"]["structuredContent"]["status"],
        "withdrawn"
    );

    let wait_call = rpc(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "proposal_wait",
            "arguments": { "id": mcp_pid, "timeoutSecs": 1 }
        }
    }));
    assert_eq!(wait_call["result"]["structuredContent"]["timedOut"], false);
    assert_eq!(
        wait_call["result"]["structuredContent"]["proposal"]["status"],
        "withdrawn"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_answers_ping_while_a_proposal_wait_blocks() {
    let repo = TestRepo::init();
    repo.write("f.txt", "before\n").commit_all("initial");
    repo.write("f.txt", "after\n");
    let mut child = Command::new(bin())
        .args(["--repo", repo.path().to_str().unwrap(), "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut send = |req: Value| {
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
    };
    let mut read = || {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };

    send(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "propose", "arguments": { "message": "m", "paths": ["f.txt"] } } }));
    let id = read()["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "proposal_wait", "arguments": { "id": id, "timeoutSecs": 3 } } }));
    send(json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }));
    send(json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {} }));
    send(json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "log" } }));

    let first = read();
    assert_eq!(first["id"], 3, "ping first: {first}");
    let rest: Vec<Value> = (0..3).map(|_| read()).collect();
    let by_id = |id: i64| rest.iter().find(|v| v["id"] == id).unwrap();
    assert_eq!(by_id(4)["error"]["code"], -32602, "no tool name");
    assert!(
        by_id(5)["result"]["structuredContent"].is_array(),
        "log answered"
    );
    assert_eq!(rest[2]["id"], 2, "the wait answers last");
    assert_eq!(by_id(2)["result"]["structuredContent"]["timedOut"], true);

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn cli_proposal_wait_and_agent_stop_hook() {
    let repo = TestRepo::init();
    repo.write("a.txt", "v1\n").commit_all("initial");
    std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
    std::fs::write(
        repo.path().join(".claude/settings.local.json"),
        serde_json::to_string_pretty(&json!({
            "permissions": { "allow": ["Bash(ls:*)"] }
        }))
        .unwrap(),
    )
    .unwrap();

    for _ in 0..2 {
        let setup_out = Command::new(bin())
            .args([
                "--repo",
                repo.path().to_str().unwrap(),
                "--json",
                "agent",
                "setup",
                "--target",
                "claude",
                "--write",
                "--with-hooks",
            ])
            .output()
            .unwrap();
        assert!(
            setup_out.status.success(),
            "{}",
            String::from_utf8_lossy(&setup_out.stderr)
        );
    }
    let settings: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.path().join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["permissions"]["allow"][0], "Bash(ls:*)");
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);

    repo.write(".gitignore", ".claude/settings.local.json\n")
        .commit_all("setup claude config");

    let clean_hook = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "agent",
            "hook",
            "--event",
            "stop",
        ])
        .output()
        .unwrap();
    assert!(clean_hook.status.success());
    let clean_json: Value = serde_json::from_slice(&clean_hook.stdout).unwrap();
    assert_eq!(clean_json["created"], false);

    repo.write("a.txt", "v2\n");
    let first_hook = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "agent",
            "hook",
            "--event",
            "stop",
        ])
        .output()
        .unwrap();
    assert!(first_hook.status.success());
    let first_json: Value = serde_json::from_slice(&first_hook.stdout).unwrap();
    assert_eq!(first_json["created"], true);
    let pid = first_json["proposal"]["id"].as_str().unwrap().to_owned();

    // Repeating with identical dirty worktree does not create a duplicate WIP proposal.
    let dup_hook = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "agent",
            "hook",
            "--event",
            "stop",
        ])
        .output()
        .unwrap();
    assert!(dup_hook.status.success());
    let dup_json: Value = serde_json::from_slice(&dup_hook.stdout).unwrap();
    assert_eq!(dup_json["created"], false);

    // Withdraw and verify `--wait` returns immediately with the verdict.
    let wd_out = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "withdraw",
            &pid,
        ])
        .output()
        .unwrap();
    assert!(wd_out.status.success());

    let wait_out = Command::new(bin())
        .args([
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
            "proposal",
            &pid,
            "--wait",
            "--timeout",
            "2",
        ])
        .output()
        .unwrap();
    assert!(wait_out.status.success());
    let waited: Value = serde_json::from_slice(&wait_out.stdout).unwrap();
    assert_eq!(waited["timedOut"], false);
    assert_eq!(waited["proposal"]["status"], "withdrawn");
}
