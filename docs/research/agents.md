# Agent interfaces: how agents touch Git, and how gitgud should take their work (track C)

Research, 2026-11-21. All links checked 2026-11-21; GitHub stars via `gh api` the same day. Labels: **fact**,
**Claude's pick**, **open**. Screenshots in `shots/workflows.json` (`wf-` prefix).

Max's answer 5 (decided 2026-11-21): "a MCP or CLI for agent to easily interact and prepare the commits for the
user". AI agents stage and draft commits; Max reviews them in gitgud.

## 1. Existing git MCP servers

| Server | ★ | What it exposes | Notable mechanics | Gap for gitgud |
|---|---|---|---|---|
| [Official `mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git) (in modelcontextprotocol/servers) | 92.1k (whole monorepo) | 14 tools: `git_status`, `git_diff_unstaged/_staged`, `git_diff`, `git_commit`, `git_add`, `git_reset`, `git_log`, `git_create_branch`, `git_checkout`, `git_merge`, `git_show`, `git_branch`, `git_remote`; 2 prompts: `git_commit_message`, `git_summarize_changes` | Treats repo config as hostile: **no hooks run**, signing forced off, repo-defined filter and merge drivers emptied, no lazy fetch in partial clones, no auto-gc | `git_add` takes **file paths only** (no hunks); commits land directly on the branch; no review step; would break LFS/`UnityYAMLMerge` set up per-repo |
| [GitHub MCP server](https://github.com/github/github-mcp-server) | 34.6k | Host API toolsets (`repos`, `issues`, `pull_requests`, `actions`, `code_security`, `copilot`…), e.g. `create_branch`, `push_files`, `create_pull_request`, `assign_copilot_to_issue`, `request_copilot_review` | Remote, no local working tree; writes go straight to GitHub | Out of scope (no in-app host APIs, decided) |
| [cyanheads/git-mcp-server](https://github.com/cyanheads/git-mcp-server) | 245 | ~30 tools: add, blame, branch, checkout, cherry_pick, clean, clone, commit, diff, fetch, log, merge, pull, push, rebase, reflog, reset, stash, tag, **worktree**, plus `git_wrapup_instructions`, `git_changelog_analyze` | STDIO or HTTP; signing via env (`GIT_SIGN_COMMITS`) | Full CLI mirror; no human gate |
| [idosal/git-mcp](https://github.com/idosal/git-mcp) | 8.5k | Docs/code of a GitHub project for context | Not a VCS tool | n/a |
| GitButler `but` | gitbutler repo 22.0k | Skill + CLI (see §2). Its MCP docs page and Claude Code hooks page now return 404 | — | open: whether the MCP server still ships |
| Graphite "GT MCP" | — | Listed in Graphite CLI docs sidebar ([docs](https://graphite.com/docs/cli-quick-start)) | not read | open |

Takeaway (Claude's pick): every existing server either mirrors the git CLI 1:1 or hits a host API. **None models
"propose → human approves"**, and none stages hunks. That is gitgud's slot.

## 2. GitButler: the closest prior art

fact ([overview](https://docs.gitbutler.com/ai-agents/overview), [getting started](https://docs.gitbutler.com/ai-agents/getting-started), [parallel agents](https://docs.gitbutler.com/ai-agents/parallel-agents)):

- `but agent setup` wizard installs a GitButler **skill** for Codex, Claude Code, Cursor, Copilot, Windsurf/Devin, OpenCode and writes workflow instructions (shows every file and the exact text before writing).
- The skill tells the agent to use `but` instead of `git checkout/stash/add/commit/rebase`. Docs say plainly that instructions steer and do not restrict.
- Commands: `but commit` (selected files **or hunks by ID**), `amend`, `squash`, `uncommit`, `pr`, `merge` (only when authorized), `oplog` (restore earlier state); move/reorder/reword/absorb/split also exist.
- Parallel agents share **one working directory**; each session's commits go to its own branch. Overlapping files must be called out by the agents.
- Policy knob: stop after local commits vs allow push/PR.
- Self-run benchmark ([vcbench.dev](https://vcbench.dev)): ~80% fewer commands, ~60% faster than plain Git, similar reliability (their own benchmark; treat as vendor claim).
- Earlier integration: Claude Code hooks `but claude pre-tool / post-tool / stop` assigned each session's edits to a branch ([skills.sh mirror](https://skills.sh/outfitter-dev/agents/gitbutler-multi-agent)); docs page now 404. Trigger.dev switched from hooks to the skill because hooks assume 1 session = 1 branch ([2026-04-16](https://trigger.dev/blog/parallel-agents-gitbutler)).

What to take (Claude's pick): hunk IDs addressable by the agent; an op log the agent and human both can roll back;
a stop-before-push policy. What to avoid: forcing everyone onto a virtual-branch workspace model to get there.

## 3. How agents commit today

| Agent | Isolation | Commits? | Attribution | Human review surface | Source |
|---|---|---|---|---|---|
| Claude Code | `--worktree` → `.claude/worktrees/<name>`, branch `worktree-<name>`; subagents `isolation: worktree`; edits to main checkout blocked while isolated | Only when asked; can create PRs via `gh` | Configurable `attribution.commit` / `attribution.pr` (old `includeCoAuthoredBy` deprecated); default trailer observed in this session: `Co-Authored-By: Claude … <noreply@anthropic.com>` | Desktop diff view with per-line comments that go back to Claude; manual mode accept/reject per edit; PR card with CI, auto-fix, merge-when-ready | [worktrees](https://code.claude.com/docs/en/worktrees), [settings ref](https://code.claude.com/docs/en/settings-reference), [desktop](https://code.claude.com/docs/en/desktop) |
| Codex app | Codex-managed worktrees in detached HEAD, deleted when thread archived; `.worktreeinclude` | Commit, push, PR from the app | — | **Review pane**: scopes Unstaged / Staged / Commit / Branch / Last turn; stage, unstage, revert whole diff, file or chunk; inline comments; `/review` findings as inline comments | [review](https://developers.openai.com/codex/app/review), [worktrees](https://developers.openai.com/codex/app/worktrees) |
| Cursor | Worktree per agent; `.cursor/worktrees.json` setup scripts; keeps newest 25 | Commit/PR from Agents Window | — | `/apply-worktree` brings results into main checkout; `/best-of-n` runs one task on several models to compare | [docs](https://cursor.com/docs/configuration/worktrees) |
| Aider | none | **Auto-commits every edit**; commits your dirty files first | `(aider)` suffix on author/committer, or `--attribute-co-authored-by` trailer; messages by a weak model, Conventional Commits by default | `/diff`, `/undo` | [docs](https://aider.chat/docs/git.html) |
| GitHub Copilot cloud agent | Cloud sandbox / Actions, one branch, one PR per task, 59-min cap | Pushes to its branch, opens PR | — | Host PR. Your own approval doesn't count if approvals are required; workflows don't run until a human clicks "Approve and run workflows"; iterate with `@copilot` comments | [about](https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent), [review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/review-copilot-prs) |
| GitButler-wrapped agent | one workdir, branch per session | `but commit` with hunk IDs | — | GitButler desktop / `but` | §2 |

Orchestrators ("review the agent's work" desktop apps):

| Tool | ★ | Mechanic | Status |
|---|---|---|---|
| [Conductor](https://www.conductor.build/) | closed | Mac app: parallel Claude Code / Codex / Cursor agents in isolated workspaces, review and merge | fact (details not on page) |
| [Sculptor](https://github.com/imbue-ai/sculptor) | 284 | Workspace = isolated copy; review changes, merge to main | research preview |
| [Crystal → Nimbalyst](https://github.com/stravu/crystal) | 3.1k | Parallel sessions in worktrees, compare approaches | renamed |
| [vibe-kanban](https://github.com/BloopAI/vibe-kanban) | 28.8k | Kanban issue → workspace (branch, terminal, dev server) → inline diff comments → PR | **sunsetting** (README banner) |

Pattern (fact, from the rows above): isolation = worktree; review = a diff with line comments fed back to the agent;
exit = PR. Nobody offers "split this into clean commits before it leaves my machine" except GitButler.

Data that sizes the problem: agents start >40% of GitHub PRs (Sept 2026) and 87% of them touch multiple files
([Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/));
58.7% of developers don't plan to let AI do "committing and reviewing code" ([SO 2025](https://survey.stackoverflow.co/2025/ai)).

## 4. jj and agents

- jj snapshots the working copy into the `@` commit on every command, so there's no "forgot to stage" failure; `jj workspace add` is its worktree; op log gives undo of any operation ([jj op log](https://docs.jj-vcs.dev/latest/operation-log/)). fact
- Advocacy: agents need a simpler state model, non-blocking conflicts, reliable undo, stable change IDs and provenance ([Amplify Partners, 2026-05-20](https://amplifypartners.com/blog-posts/will-agents-like-git-any-more-than-we-do)). Opinion source.
- Community skills for agents using jj exist (e.g. [skills.sh jj](https://skills.sh/schpet/toolbox/jj)); no first-party agent integration found. open
- Claude's pick: borrow the *properties* (every agent action snapshotted, undoable, identified by a stable proposal ID), stay on plain Git.

## 5. What should an agent hand to a human?

Claude's pick: a **proposal** = an ordered set of draft commits, not a branch and not a pile of staged files.

| Field | Why | Source of the idea |
|---|---|---|
| Base commit (OID) | Lets gitgud detect drift and replay onto current HEAD | `git replay` ([2.54](https://github.blog/open-source/git/highlights-from-git-2-54/)) |
| 1..n draft commits, each = hunks + message | Reviewable units; Max can approve some, not all | GitButler hunk IDs; stacked PRs |
| Per-commit "why" (1–3 lines) separate from the message | Review context without polluting history | Codex review prompts ask for evidence |
| Evidence: commands run + result (tests, build, Unity compile) | 66% top frustration is "almost right" ([SO 2025](https://survey.stackoverflow.co/2025/ai)) | — |
| Risk flags computed by gitgud, not the agent: files outside declared scope, binaries/LFS, `.meta` without its asset (or vice versa), lockable file not locked by Max, deleted files | Unity specifics; agents can't be trusted to self-report | LFS locking ([wiki](https://github.com/git-lfs/git-lfs/wiki/File-Locking)) |
| Agent identity + session link | Attribution trailer and "go back to the conversation" | Claude Code attribution, `--from-pr` |

Not in the hand-off: pushes, branch moves, force anything. The agent never touches Max's index or refs. Claude's pick

## 6. gitgud CLI/MCP + GUI approval flow

### Options

| | Mechanism | + | − |
|---|---|---|---|
| A | Agent stages into the real index and writes a draft message file; Max commits | Trivial; works with any tool | Fights Max's own staging; one proposal at a time; parallel agents collide |
| B | Agent commits on its own branch/worktree; gitgud shows "agent branches" for review/merge | Matches Claude Code / Codex / Cursor today; zero new concepts | Splitting/rewording happens after the fact (history edits, P3); worktree setup cost (W5) |
| **C** | **Proposals as hidden refs**: agent calls `propose(hunks, message, why)`; gitgud builds commits with plumbing (no index, no worktree change) under `refs/gitgud/proposals/<id>`, metadata in a sidecar JSON; GUI inbox to review, edit, approve | Max's index and branches untouched; multiple agents in parallel; edit before it becomes history; survives restarts; plain Git objects | New concept to learn; needs hunk-ID stability; conflicts when Max edits the same lines meanwhile |
| D | Virtual-branch workspace (GitButler model) | Proven | Replaces Max's mental model; heavy |

**Claude's pick: C, with B as a supported input** (gitgud also lists `.claude/worktrees`, Codex and Cursor worktrees
and lets Max turn a worktree branch into a proposal). Max decides.

### Flow (Claude's pick)

1. Agent (Claude Code via MCP or shell via `gitgud` CLI) calls `gitgud.status` → files + hunks with stable IDs (hash of path + base blob + hunk content), LFS/lock/`.meta` state.
2. Agent calls `gitgud.propose({base, commits:[{hunks:[ids], message, why}], evidence, scope})`. gitgud validates (hunks still match, risk flags), writes objects, returns `proposal_id`, status `pending`. Working tree untouched.
3. GUI: inbox badge. Review view = commits on the left, diff with flags on the right. Max can: edit message, move hunks between commits, drop a commit, leave line comments.
4. Max chooses **Approve** → gitgud applies via the `git` CLI onto the current branch (commit hooks and signing run here, as Max, which the official MCP server can't do), then clears those hunks from the worktree diff. Or **Request changes** → comments stored; agent reads them with `gitgud.feedback(proposal_id)`. Or **Reject**.
5. Every step is an op-log entry; "Undo approve" restores the previous refs/index.
6. Push stays a separate human action (policy option later: allow agent push for approved proposals).

### Surface (Claude's pick, minimal)

| MCP tool / CLI | Does |
|---|---|
| `status` / `gitgud status --json` | Files, hunks with IDs, flags |
| `diff`, `log`, `show` | Read-only, via `gix` |
| `propose` / `gitgud propose` | Create proposal (above) |
| `proposal` / `gitgud proposal <id>` | Status: pending / changes requested / approved / rejected + comments |
| `withdraw` | Agent cancels its proposal |

Nothing else in v1: no `commit`, `checkout`, `merge`, `push`, `rebase` tools. One core in `gitgud-core`, two thin
adapters (CLI, MCP stdio), per the stack in `CLAUDE.md`.

### Open

- Hunk IDs: how to keep them stable when Max edits nearby lines between `status` and `propose`.
- Does Claude Code's PostToolUse/Stop hook make auto-proposals worthwhile, or is an explicit `propose` call better? (GitButler moved from hooks to an explicit skill, [Trigger.dev](https://trigger.dev/blog/parallel-agents-gitbutler).)
- Ship a `gitgud agent setup` like `but agent setup` that writes the instructions/skill file? Likely yes.
- MCP elicitation vs polling for "wait for Max's verdict".
- Unity validation rules (which `.meta` checks, lock policy) need Max's input.
