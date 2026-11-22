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

    drop(stdin);
    let _ = child.wait();
}
