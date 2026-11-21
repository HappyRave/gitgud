import json, os, subprocess, sys, time

S = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.join(os.path.dirname(S), "repos", "boat")
BIN = os.path.join(S, "agentcore", "target", "release", "agentcore.exe")
CS = "Assets/Scripts/Boat/Engine.cs"
NEW = "Assets/Scripts/Boat/Turbo.cs"
env = dict(os.environ)
env.pop("GIT_CURL_VERBOSE", None)


def git(*a):
    return subprocess.run(["git", *a], cwd=REPO, capture_output=True, text=True, env=env, check=True).stdout


def cli(*a):
    r = subprocess.run([BIN, REPO, *a], capture_output=True, text=True, env=env)
    return r.stdout + r.stderr


git("checkout", "-q", "-f", "spike-base")
git("clean", "-qfd", "Assets/Scripts")
for r in git("for-each-ref", "--format=%(refname)", "refs/gitgud/").split():
    git("update-ref", "-d", r)

# "Agent" edits: two separate hunks in one file, plus a new script without its .meta.
p = os.path.join(REPO, CS)
raw = open(p, encoding="utf-8", newline="").read()
eol = "\r\n" if "\r\n" in raw else "\n"
lines = raw.split(eol)
lines.insert(5, "// agent: hunk A")
lines.insert(len(lines) - 3, "        // agent: hunk B")
open(p, "w", encoding="utf-8", newline="").write(eol.join(lines))
open(os.path.join(REPO, NEW), "w", encoding="utf-8").write("public class Turbo {}\n")
git("add", "-N", NEW)

print("== CLI status")
print(cli("status"))
ids = [l.split()[0] for l in cli("status").splitlines()]

print("== MCP session (initialize, tools/list, status, propose)")
msgs = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "spike", "version": "0"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "propose", "arguments": {"hunks": [ids[0], ids[2]], "message": "Add turbo stub and engine note", "why": "spike: test proposal flow"}}},
]
proc = subprocess.Popen([BIN, REPO, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
t = time.time()
for m in msgs:
    proc.stdin.write(json.dumps(m) + "\n")
    proc.stdin.flush()
    if "id" in m:
        resp = json.loads(proc.stdout.readline())
        if m["method"] == "tools/list":
            print("tools:", [x["name"] for x in resp["result"]["tools"]])
        elif m["method"] == "tools/call":
            print("propose ->", resp["result"]["content"][0]["text"])
        else:
            print("initialize ok:", resp["result"]["serverInfo"]["name"])
proc.stdin.close()
proc.wait(5)
print(f"MCP round trips: {(time.time() - t) * 1000:.0f} ms")

print("== user's index untouched before approval:", repr(git("diff", "--cached", "--name-only").strip()))
print("== worktree still has all agent edits:", git("diff", "--stat").strip().splitlines()[-1])
pid = json.loads(cli("list"))[0]["id"]
print("== approve", pid, "->", cli("approve", pid).strip())
print(git("log", "-1", "--stat", "--format=%s%n%b").strip())
print("== remaining unstaged hunks (should be hunk B):")
print(cli("status"))
print("refs left:", git("for-each-ref", "refs/gitgud/").strip() or "none")
git("reset", "-q", "--hard", "spike-base")
git("clean", "-qfd", "Assets/Scripts")
