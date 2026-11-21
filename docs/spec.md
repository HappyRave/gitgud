# gitgud spec

Status: **approved by Max, 2026-11-21** (Max: "Approve, then commit"), after an adversarial review the
same day (section 10). With approval, every **pick** and tier below is decided as written. **Proposal**
and **open** items stay as labelled. Built from `docs/research/`, the answered open questions
(`docs/research/open-questions.md`) and four design rounds (`docs/lab/decisions.md`, mocks in
`docs/lab/index.html`).

Labels:

- **decided**: Max said it; the source is given (plan §0, Q1–Q19, R1–R4), and Max's words are in
  section 9.
- **decided (came with R*-A)**: part of a layout Max picked, not discussed separately.
- **pick**: Claude's pick, not yet approved.
- **proposal**: an idea, weaker than a pick.
- **open**: needs an answer, a test or a spike.

Design rounds decided how things look and behave. **Which tier (must / should / later) a feature sits in
is a pick throughout**, except where Q5–Q7 put something in or out of the MVP. Approving this spec turns
the picks into decisions.

## 1. Positioning

**Who:** Max first: a Unity developer on Windows with 150+ repos on Bitbucket and GitHub, many with
LFS, today on Sourcetree and IDE git. Open source later; no onboarding or support work until it's good.
(decided: plan §0.1, Q2, Q3, R3 brief)

**What it does better than the tools in `docs/research/landscape.md`** (pick):

1. **The best Git client for Unity.**
   - LFS done properly: status without smudging, image diffs, and lock panels on hosts that support
     LFS locking. That's GitHub for Max; Bitbucket Cloud has no lock API (O1).
   - Unity Smart Merge set up and checked by gitgud.
   - A `.meta` guard that treats an asset and its meta as one unit.
   - A structural prefab diff.
   - No new VCS, no extra server.
   - Hands-on: none of Fork, Sublime Merge or GitButler offers UnityYAMLMerge, and GitButler bypasses
     LFS filters.
2. **Every local action undoable.**
   - An op log behind Ctrl+Z, discards and stashes included.
   - It saves refs and the index before every action, and dirty files before any action that could
     lose them.
   - Pushes and remote deletes can't be undone remotely; gitgud says so and offers a revert commit.
   - Hands-on: only GitButler could recover a discard.
3. **Agents propose, you approve.**
   - A CLI and MCP server on the same core.
   - Agent work arrives as proposals under hidden refs and is reviewed in gitgud.
   - Approval commits through the `git` CLI, so hooks and signing run.

Underneath all three: **fast on big and LFS repos** (Max's pain 1), with targets in section 6 and a
Sourcetree baseline still to measure (O9).

**Name:** gitgud, kept despite `zeuros/gitgud` (decided, Q1).

## 2. Platform and stack

| Item | Value | Label |
|---|---|---|
| Platforms | Windows now; no Windows-only APIs, so macOS/Linux are packaging later | decided, plan §0.2 |
| Shell | Tauri 2, Rust backend, SvelteKit (Svelte 5 runes) | decided, CLAUDE.md |
| Reads | `gix` for status, log, graph, diff, blame | decided, CLAUDE.md |
| Writes and network | spawn the `git` CLI (hooks, credential helpers, SSH, signing as in a terminal) | decided, CLAUDE.md |
| Which git | system `git` when ≥ 2.45.2, else the bundled Git, said once in the status strip, no prompt | decided, Q17 (behaviour pick) |
| Bundled Git | includes git-lfs; git-lfs floor 3.3 (`locks --json`, `GIT_LFS_PROGRESS`), checked like git | pick (floor: open, verify) |
| Agent adapters | thin CLI and MCP (stdio) on `gitgud-core` | pick, spikes 4.7, agents.md §6 |
| Graph rendering | lanes computed in Rust, sent as checkpoints + visible rows; graph column on canvas | pick, spikes 4.3 |
| File watching | `notify` per non-ignored top-level dir, ~150 ms debounce, paused during gitgud's own git ops | pick, spikes 4.4 |
| WebView keys | browser accelerator keys disabled (`AreBrowserAcceleratorKeysEnabled = false`) so Ctrl+P / Ctrl+F / F5 / F7 reach the app | pick |
| Telemetry | none; a local latency panel only | pick, ux-principles 1 |

**Rules from the spikes** (pick):

- Rename detection off or capped for large ranges, and run lazily per file.
- Refresh the index once after a mass touch such as a Unity reimport. The `gix` write-back is untested, so the CLI fallback is `git update-index -q --refresh`.
- Write the commit-graph on first open, with one confirm.
- Always set `GIT_TERMINAL_PROMPT=0`.
- Background network uses `GCM_INTERACTIVE=never` and SSH `BatchMode=yes`.
- LFS progress comes from `GIT_LFS_PROGRESS`.
- Cancel kills the process tree through a Windows Job Object, then cleans up `tmp_pack_*` and partial clone directories.
- Offer `core.longpaths` on clone.

**Unity detection** (pick): `ProjectSettings/ProjectVersion.txt` at the repo root or one level down.

- It drives the "Unity projects" filter, Smart Merge, the `.meta` guard and lock surfaces.
- These Unity surfaces appear only when a Unity project is detected (ux-principles 8).
- Unity installs are found through Unity Hub's editor list and matched to `ProjectVersion.txt`.

## 3. MVP scope

**Must** = v1 doesn't ship without it. **Should** = v1 if time allows. **Later** = after v1. Tiers are
picks unless marked. Effort flags come from the research; section 7 proposes splitting v1 into phases.

### 3.1 Projects and windows

| Feature | Tier | Label |
|---|---|---|
| Library tab, always first: every repo with branch, changes, ahead/behind (with its age), locks held, last activity, host | must | decided, R3 + R3 Q4 |
| Nested folders, one folder per repo, drag to sort | must | decided, R3 Q2 Q3 |
| Smart filters: Uncommitted, Not pushed, Locks I hold, Unity projects, Missing on disk | must | decided (came with R3-A) |
| Repos as tabs; Ctrl+P go to repo | must | decided (came with R3-A) |
| Reopen last session's tabs | must | decided, R3 Q5 |
| Library status local only, cached, refreshed for visible rows; "Fetch all visible"; "Locks I hold" from the last lock refresh, with its age | must | decided, R3 Q4 (lock source pick) |
| Scan a folder: add many repos, folders suggested from disk layout | must | decided (came with R3-A) |
| Clone with progress, cancel and LFS progress | must | pick |
| One window; tabs only (no multi-window) | must | pick (multi-window: later) |
| Ultrawide: inspector splits into two columns above ~2400 px | should | decided, R1 Q5 |
| Compact / Standard density (20 / 24 px rows) | should | decided, R1 Q4 |
| Light / dark following the OS | must | pick, ux-principles 15 |

### 3.2 Daily loop (layout R1-A: sidebar | graph | inspector)

| Feature | Tier | Label |
|---|---|---|
| Commit graph, all branches, WIP row on top, stash rows | must | decided, R1 |
| Sidebar: branches (folder-grouped), remotes, stashes, locks, proposals, op log | must | decided, R1 |
| Inspector modes: working copy, commit, stash, history plan, proposal, op log | must | decided, R1–R3 (stash mode pick) |
| Hunk and line staging; per-hunk Stage / Discard on the hunk's own row | must | decided, R1 |
| Text diff: syntax highlight, intra-line changes; unified under ~1400 px, split above, Ctrl+D toggles | must | pick, ux-principles 14 |
| Image diff (side by side, swipe, onion) in staging, commits and conflicts | must | decided, Q6 |
| Image formats: PNG, JPG, TGA, flat PSD; TGA/PSD decoded in Rust (WebView2 can't) | must | pick |
| Commit box: message, Commit, ⋯ for amend / sign-off / no-verify | must | pick, ux-principles 8 |
| Branches: create, checkout, rename, delete (undoable), set upstream | must | pick |
| Tags: create, delete, push | should | pick |
| Fetch, pull, push via the CLI with progress and cancel | must | pick |
| Pull: honours `pull.rebase` / `pull.ff`; with no config, fast-forward only, and on divergence asks merge or rebase once per repo | must | pick |
| Force-push only as `--force-with-lease`, named in the dialog | must | pick, ux-principles 5 |
| Force-push offered right away after rewriting a pushed branch | must | decided, R2 Q4 |
| Open / create pull request in the browser (GitHub and Bitbucket URL formats), per branch and per stack level | must | decided, plan §0.3 + Q8 |
| Merge another branch into the current one, with conflict prediction | must | pick |
| Rebase the current branch onto another (non-interactive), with conflict prediction | must | pick |
| Cherry-pick with conflict prediction | should | pick |
| Revert a commit (the undo for pushed work) | must | pick, ux-principles 3 |
| Reset branch to a commit: soft / mixed / hard, explained in plain words, undoable | must | pick |
| Stash: save (message, include untracked), apply, pop, drop; stash view in the inspector | must | pick |
| File history and blame | should | pick |
| Live state: outside changes appear without a refresh button | must | pick, ux-principles 10 |
| Detached HEAD shown loudly with "Create branch here" | must | decided (R1 state 4) |
| Unity open while checkout / merge / rebase touches `Assets/` (detected via `Temp/UnityLockfile`): one-line warning, continue or cancel | must | pick |
| AI commit message draft via installed Claude Code / Codex CLI, optional API key | should | decided, Q13 |

### 3.3 Safety and undo

| Feature | Tier | Label |
|---|---|---|
| Op log per repo under a private ref, behind Ctrl+Z | must | decided, Q9 |
| Before every action: save refs + a copy of the `.git/index` file (~1 ms, no git process) | must | decided 2026-11-21, after spike S1 (Max: "Split it") |
| Before actions that can lose working-tree content (discard, checkout over changes, hard reset, stash, rebase / merge start): also snapshot the dirty tracked and untracked (not ignored) files through a persistent snapshot index; LFS files only when dirty and about to be overwritten | must | decided 2026-11-21, after spike S1 (Max: "Split it") |
| Retention: last 30 days or 2,000 entries, whichever is larger; size shown in the op log, counting LFS objects only the op log uses; prune with preview | must | pick |
| Redo: Ctrl+Y and Ctrl+Shift+Z | must | pick, ux-principles 3 |
| Op log panel (sidebar entry, Ctrl+3) with Restore on any entry | must | decided (came with R1-A) |
| Operations done outside gitgud (CLI, IDE, agents) show as "outside gitgud" entries from the reflog, restorable as far as the reflog allows | should | pick, ux-principles 3 |
| Library has its own undo (folder moves, remove repo); Ctrl+Z in the Library tab acts on it | must | pick |
| Confirm dialogs only for what the op log can't undo: push, force-push, deleting a remote branch | must | pick, ux-principles 5 |
| In-progress operation banner (rebase / merge / cherry-pick / revert) with Continue / Skip / Abort | must | decided (R1 state 1) |
| Errors: what happened in plain words, the git concept, one fix button, raw stderr collapsed | must | pick, ux-principles 13 |

### 3.4 History edits (R2-A)

| Feature | Tier | Label |
|---|---|---|
| Plan in the inspector: per-commit action P / R / E / S / F / D, drag or Alt+↑↓, newest first | must | decided, R2 + R2 Q3 |
| Graph preview: dashed "after" rows over faded current commits; nothing runs until Start | must | decided, R2 |
| Conflict prediction per step before Start; "Show the conflict" / "Keep the old base" / "Start anyway" | must | decided, Q11 + (came with R2-A) |
| `fixup!` / `squash!` applied automatically when the plan opens | must | decided, R2 Q2 |
| Single-commit edits apply at once, Ctrl+Z undoes (rules below) | must | decided, R2 Q5 |
| "Update dependent branches" (`--update-refs`); stacks shown in the graph, restacked, PR link per level | should | decided, Q8 |
| Rewrite logic in `gitgud-core` (`gix` + `git replay` / rebase), stop on conflict | must | decided, Q10 Q11 |

**Single-commit edit rules** (pick, within R2 Q5):

- R on the branch tip renames inline in the graph. R on an older commit opens the plan with that row set to Reword.
- F (fold into parent) and D (drop) on one non-merge commit of the current branch run at once.
- If the branch is on the remote, the result offers force-push with lease, as the Done panel does.
- On a merge commit, or on another branch, they open the plan instead.
- Del deletes a branch only when a branch label or a sidebar branch is focused, never on a commit row.

### 3.5 Merges and conflicts (R4-A)

During a merge, rebase or cherry-pick with conflicts, the conflict view **replaces the graph and the
inspector**; the sidebar stays (decided, came with R4-A).

| Feature | Tier | Label |
|---|---|---|
| Conflict list grouped by kind (Unity YAML, code, LFS binary); resolved files staged as you go | must | decided (came with R4-A) |
| Unity YAML: property table (object › component › property; base / yours / theirs / result; 1 / 2 / E), fileIDs resolved to names, raw YAML tab | must (v1.1) | decided, R4; v1.0 shows the verify comparison as a plain list |
| Smart Merge (UnityYAMLMerge, `--force`) offered at the first merge that hits Unity files | must | decided, R4 Q2 |
| Smart Merge setup writes only `.git/config` + `.git/info/attributes`; "Resolve as text this time"; "Other Unity version…" | must | decided (came with R4-A) |
| After every automatic Smart Merge: compare both sides per property | must | decided (came with R4-A) |
| "Merged, verify" flags block Commit merge until confirmed | must | decided, R4 Q4 |
| What Smart Merge merged on its own: listed, collapsed | must | decided, R4 Q3 |
| Code: 3-way text merge, three columns (yours / result / theirs), arrow per hunk, "use both" | must | decided, R4 Q5 |
| LFS binary: pick a side with the image diff | must | decided (R4 state 3) |
| Post-merge check: prefabs re-read, no missing references, no duplicate fileIDs | must (v1.1) | decided (came with R4-A) |
| A conflict pick is an op-log entry (Ctrl+Z undoes it); text undo inside the result editor is separate | must | pick |

### 3.6 Unity and LFS

| Feature | Tier | Label |
|---|---|---|
| **Unity YAML parser + project GUID / fileID index**: shared by the property table, structural prefab diff, post-merge reference check and `.meta` GUID checks. The research puts it at weeks of work | must (v1.1) | pick (effort flag: open-questions "Challenges") |
| LFS-aware status via `gix` without smudging; pointer vs hydrated state | must | pick, gamedev T9 |
| Lock panel (sidebar) + lock badge and owner on file rows; lock / unlock from the context menu | must | pick, gamedev T1 |
| Lock surfaces hidden, with the reason, when the host has no lock API (`/locks/verify` → 404, e.g. Bitbucket Cloud) | must | pick, O1 |
| "Locked by someone else" warning inline when you edit or stage such a file | must | decided (R1 state 2) |
| Unlock-on-push checklist: your locks in the pushed commits, ticked by default, never silent | must | pick, gamedev T2 |
| Lock refresh: on fetch, on tab open, and every 10 min for the active tab; age shown wherever locks show | must | pick |
| Remote refresh for open tabs: fetch when a tab opens if the last fetch is over 10 min old, and every 10 min for the active tab (the Library stays local-only, R3 Q4) | must | pick |
| `.meta` guard: asset + meta stage / unstage / discard / rename as one unit; orphan or missing meta, GUID collisions and case mismatches shown inline in the working copy and blocking nothing | must | pick, gamedev T6 + spikes 4.6 |
| Structural prefab diff in history and staging (changed objects and fields) | must (v1.1) | decided, Q6 (phase: decided, section 7) |
| Auto-lock suggestion on first modify of a lockable file | should | pick, gamedev T3 |
| One-click Unity repo setup (`.gitattributes`, `.gitignore`, Force Text check) | should | pick, gamedev T12 |
| LFS cache size and `git lfs prune` with preview | later | proposal, gamedev T10 |
| Rendered prefab thumbnails | later | decided, Q6 |
| Sparse checkout / `lfs.fetchinclude` UI | later | decided, Q7 |
| Unity editor package | later | decided, Q5 |
| Artist mode | later | decided, Q3 |

### 3.7 Agents

All agent rows are v1.1 (decided, section 7).

| Feature | Tier | Label |
|---|---|---|
| CLI and MCP: `status`, `diff`, `log`, `show`, `propose`, `proposal`, `withdraw`; nothing that commits, checks out, merges, rebases or pushes | must (v1.1) | decided, Q12 (surface pick, agents.md §6) |
| `status` covers untracked files without `git add -N`; hunks with stable IDs | must (v1.1) | pick, spikes 4.7; IDs depend on O2 |
| Proposals under `refs/gitgud/proposals/<id>`; Max's index and branches untouched until approval | must (v1.1) | decided, Q12 |
| Inbox in the sidebar, ghost row in the graph, review in the inspector (message, why, risk flags, per-hunk ticks) | must (v1.1) | decided (R1 state 3) |
| Approve commits through the `git` CLI onto the current branch; if HEAD moved since the proposal, replay it first (`git replay` / cherry-pick) and show the result | must (v1.1) | pick, agents.md §6 + spikes 4.7 |
| Reject; Request changes with comments the agent reads via `proposal` | must (v1.1) | pick |
| Unity risk flags (missing `.meta`, unlocked lockable file, LFS pointer issues) flag, never block | must (v1.1) | decided, Q14 |
| Agent lock tools (`locks list / lock / unlock`) | should | pick, gamedev T14 |
| `gitgud agent setup` writes a skill / instructions file | should | proposal |
| Agent worktrees (Claude Code / Codex / Cursor) listed and turned into proposals | later | proposal, agents.md §6 |

### 3.8 Non-goals for v1

- In-app pull request APIs; host features go through browser URLs (decided, plan §0.3).
- A private lock server; locks are plain LFS locks (pick, gamedev T4).
- Non-Git backends, Lore (decided, Q4).
- A virtual-branch workspace (GitButler model): it bypasses LFS filters (pick, hands-on).
- Background fetch of repos that aren't open (decided, R3 Q4).
- Multi-window (pick; later).
- Telemetry (pick).
- A code editor beyond the merge result and commit messages (pick).
- Git Flow, submodule management UI, bisect UI (proposal: later if asked).

## 4. Screens

Inside the R1-A frame unless noted. "Drawn" refers to `docs/lab/index.html`.

| # | Screen | Where | Drawn |
|---|---|---|---|
| 1 | Library | tab 1 | R3-A, R3 states 3–4 |
| 2 | Repo window: sidebar, graph, inspector (working copy) | each repo tab | R1-A |
| 3 | Commit detail (incl. image diff, structural prefab diff) | inspector | R1 state 4 (prefab diff: no) |
| 4 | Stash detail | inspector | no |
| 5 | Op log | inspector + sidebar | R1 state 6 |
| 6 | History plan, quick edits, done panel | inspector + graph preview | R2-A + states (quick edits: no) |
| 7 | Conflicts: list + property table / 3-way text / binary pick / summary / Smart Merge setup | replaces graph + inspector | R4-A + states |
| 8 | Proposal review, request-changes comments | inspector | R1 state 3 (comments: no) |
| 9 | Locks: sidebar section, file-row badges, unlock-on-push checklist | sidebar + dialog | R1 state 2 (checklist: no) |
| 10 | Command palette (Ctrl+K), go to repo (Ctrl+P), shortcut sheet (?) | overlay | R3 state 1 (others: no) |
| 11 | Error panel (plain words + fix + raw stderr) | inline where the action was | no |
| 12 | Dialogs: clone, scan a folder, push / force-push with lease, pull strategy, reset mode, branch create / rename, Smart Merge setup | modal or inline | R3 state 3, R4 state 4 (others: no) |
| 13 | Settings: identity, git path and version, Unity installs, AI CLI / key, theme, density, keymap | tab or modal | no |

Not drawn yet from decided rounds: ultrawide split and Compact density (R1), "Fetch all visible" and
the "behind" age (R3), quick edits (R2). Everything marked "no" needs a design round or a sketch before
it's built (O3).

## 5. Keyboard map

One keymap file, with a `mod` key (Ctrl on Windows, Cmd later). Every command is also in the palette
with its shortcut and git equivalent. Rows are pick unless marked.

**Focus rule** (pick): while a text field has focus, single-key shortcuts and the op-log Ctrl+Z are off.
Ctrl+Z / Ctrl+Y there undo and redo text; Esc leaves the field. F6 / Shift+F6 move focus between panes.
Brackets need AltGr on many layouts, so no shortcut uses them.

| Scope | Keys |
|---|---|
| Global | Ctrl+K commands · Ctrl+P go to repo · ? shortcut sheet · Ctrl+Tab / Ctrl+Shift+Tab next / previous tab · Ctrl+W close tab (not the Library) · Ctrl+Z undo (decided, Q9) · Ctrl+Y / Ctrl+Shift+Z redo · Ctrl+1 working copy · Ctrl+2 focus graph · Ctrl+3 op log (decided, came with R1-A) · Ctrl+Shift+F fetch · Ctrl+Shift+L pull · Ctrl+Shift+P push · Ctrl+F find · Ctrl+, settings · F6 next pane · Esc cancel / back |
| Library | type to filter · ↑↓ move · Enter open in a tab · F2 rename folder · Del remove from gitgud (Library undo) |
| Graph | ↑↓ move · Enter show commit · Ctrl+C copy hash · Ctrl+B new branch here · Shift+E edit history from here · R / F / D quick edits (section 3.4) · on a branch label: F2 rename, Del delete (undoable) |
| Working copy | ↑↓ files · Alt+↑↓ next / previous hunk · S stage hunk · U unstage hunk · A stage all · Space stage / unstage file · ⌫ discard hunk · Shift+⌫ discard file (both undoable) · Ctrl+D split / unified · C focus commit message · Ctrl+Enter commit |
| History plan | P pick · R reword · E edit · S squash · F fixup · D drop · Alt+↑↓ move · Ctrl+Enter start · Esc cancel (decided, R2) |
| Conflicts | ↑↓ rows or files · 1 yours · 2 theirs · E edit value · Alt+← / Alt+→ / Alt+↓ use yours / theirs / both (text merge) · F7 / Shift+F7 next / previous conflict · Ctrl+Enter mark file resolved |
| Proposal | ↑↓ hunks · Space tick / untick · Ctrl+Enter approve and commit · Ctrl+Shift+R request changes · Ctrl+Shift+X reject |

## 6. Performance targets

Targets are pick. Evidence is fact from `docs/research/spikes.md` (Max's PC) unless noted.

| Target | Evidence / precondition |
|---|---|
| Local read action (select, switch file, open palette) paints in ≤ 100 ms | ux-principles 1 |
| Stage / unstage hunk ≤ 150 ms including its op-log entry | `git apply --cached` 27 ms + index copy ~1 ms (spikes 4.8) |
| Op-log entry for ref / index actions ≤ 10 ms | index file copy, no git process (spikes 4.8) |
| Working-tree snapshot before a destructive action ≤ 250 ms with dirty text files; ≤ 1.5 s with 700 MB of dirty LFS files about to be overwritten | 200 ms clean, 1,416 ms with 673 MB (spikes 4.8; S1b may lower it) |
| Open a repo: first 500 commits drawn ≤ 300 ms **with a commit-graph**; ≤ 1 s without (lanes for visible rows from a partial walk) | `gix` 500 commits 2.3 ms; topology 106 ms with commit-graph, full walk 2,624 ms without |
| Full graph topology of a 350k-commit repo ≤ 1 s in the background, with commit-graph | 106 ms measured |
| Scrolling at display refresh, ≤ 1% frames over budget, also on scrollbar jumps | canvas 0% slow frames at 100 fps |
| Status, clean: ≤ 50 ms on a small Unity repo (~1.4k files); ≤ 300 ms on 65k files warm | BoatAttack 26–31 ms; rust 278 ms `gix` (CLI 106 ms) |
| Status, cold cache on 65k files: show cached status at once, then fresh ≤ 10 s | `gix` 6,957 ms cold |
| After a mass touch: one index refresh, then status back to warm figures; refresh ≤ 2 s per 200 LFS files | CLI refresh 1,902 ms for 185 files (`gix` write-back untested) |
| Change outside gitgud visible ≤ 300 ms on small repos, ≤ 1 s on 65k files | watcher p95 0.35 ms + 150 ms debounce + status |
| Library of 150+ repos opens ≤ 200 ms from cache; visible rows refreshed ≤ 1 s; all repos ≤ 2 s in the background | one screen of rows 160 ms; 152 repos incl. rust 0.5–1 s on 8 threads (spikes 4.9) |
| Cancel any git operation ≤ 250 ms to stop; cleanup runs in the background | tree kill 170–190 ms |
| Memory: ≤ 300 MB total app with one 350k-commit repo; + ≤ 50 MB per extra open tab | 123 MB JS heap measured with all rows in JS; Rust side not measured |
| LFS pull / clone / checkout at least as fast as the `git` CLI (≥ 35 MB/s on BoatAttack) and faster than Sourcetree | CLI 39 MB/s (903 MB in 23.3 s); Sourcetree baseline not run (O9) |
| Image diff of a 2048² texture shown ≤ 300 ms; PSD up to 100 MB ≤ 1.5 s | not measured |
| Lock list refresh ≤ 2 s on a normal host | not measured |
| App cold start to Library ≤ 1 s | not measured |

## 7. Phasing

**Decided 2026-11-21: v1.0, then v1.1** (Max: "v1.0 then v1.1"). Reason: the research puts the Unity
YAML parser and GUID index at weeks of work, and the op log and rebase engine are new code.

- **v1.0:**
  - the Library and the daily loop;
  - pull / push / merge / rebase / revert / reset;
  - op log and history plan;
  - 3-way text merge;
  - LFS status, locks and image diff;
  - Smart Merge driver with verify flags (the per-property comparison shown as a plain list, not
    the table yet);
  - `.meta` guard.
- **v1.1:**
  - Unity YAML parser + GUID index;
  - property table, structural prefab diff, post-merge reference check;
  - agents (CLI / MCP / inbox).

Rows in section 3 marked **must (v1.1)** belong to v1.1; every other must is v1.0.

## 8. Open

| # | Question | Label |
|---|---|---|
| O1 | ~~Bitbucket Cloud or Data Center?~~ Answered 2026-11-21: **Bitbucket Cloud** (Max: "Bitbucket Cloud"). Cloud has no LFS lock API (gamedev.md §2.9), so lock features work only on the GitHub repos; on Bitbucket repos the lock surfaces hide with the reason. Q2's "lock features target both" can't hold for Bitbucket | decided |
| O2 | Hunk-ID stability when Max edits near an agent's hunks between `status` and `propose` | open, design (blocks the agent `status` row) |
| O3 | Screens marked "no" in section 4 need a design round or sketch | open, design |
| O4 | Narrator (screen reader) with the canvas graph and virtualised lists | open, test |
| O5 | 4K DPR-2 and weaker-laptop rendering | open, test |
| O6 | Agent waiting for a verdict: MCP elicitation vs polling `proposal` | open, design |
| O7 | Hooks-driven auto-proposals vs explicit `propose` calls | open, design |
| O8 | git-lfs floor version (3.3 assumed) | open, verify |
| O9 | Sourcetree baseline: clone, LFS pull, status, open on BoatAttack and a 150-repo setup | open, measure (by hand or with Max) |
| S1 | ~~Op-log snapshot cost~~ Done 2026-11-21 (spikes 4.8): ~23 ms per git process on Windows; led to the split op log (section 3.3) | done |
| S1b | In-process (`gix`) working-tree snapshot, including whether `gix` runs the LFS clean filter | open, spike |
| S2 | ~~Library refresh with 150+ repos~~ Done 2026-11-21 (spikes 4.9): targets hold. Still unmeasured: a cold disk cache, and 150 real Unity repos | done |

## 9. Decision log

Max's verdicts with dates and his words. Full text: `docs/lab/decisions.md` (R1–R4),
`docs/research/open-questions.md` (Q1–Q19), plan section 0.

| Area | Decision | Max's words | Date | Source |
|---|---|---|---|---|
| Foundations | Audience, platforms, hosts, game-dev focus, AI, pains | plan §0 answers, e.g. "a MCP or CLI for agent to easily interact and prepare the commits for the user" | 2026-11-21 | plan §0 |
| Name | Keep gitgud | "Keep gitgud for good" | 2026-11-21 | Q1 |
| Hosts | Bitbucket and GitHub | "bitbucket and github" | 2026-11-21 | Q2 |
| Users | Solo first, artists after v1 | "Solo first, artists later" | 2026-11-21 | Q3 |
| Backends | Git only | "Ignore, Git-only" | 2026-11-21 | Q4 |
| Unity package | After v1 | "After v1" | 2026-11-21 | Q5 |
| Previews | Textures + prefab object list in the MVP | "Textures + prefab object list" | 2026-11-21 | Q6 |
| Sparse checkout UI | Later | "Later" | 2026-11-21 | Q7 |
| Stacks | Show + restack, PR links via browser | "Show + restack" | 2026-11-21 | Q8 |
| Undo | Own op log behind Ctrl+Z | "Own op log, Ctrl+Z" | 2026-11-21 | Q9 |
| Rewrites | Own logic in core | "Own it in core" | 2026-11-21 | Q10 |
| Rebase conflicts | Classic stop + preview | "Classic stop + preview" | 2026-11-21 | Q11 |
| Agents | Proposals + review inbox | "Proposals + review inbox" | 2026-11-21 | Q12 |
| AI messages | Installed CLI, API key optional | "Installed CLI, API key optional" | 2026-11-21 | Q13 |
| Agent checks | Flag all, block none | "Flag all, block none" | 2026-11-21 | Q14 |
| Git version | System git, bundled fallback | "Bundle our own Git", then "System Git, bundled as fallback" | 2026-11-21 | Q17 |
| Main window | R1-A Graph centre; 24 px + Compact; ultrawide inspector split | "A, as picked"; "24 px default + compact toggle"; "Inspector splits in two" | 2026-11-21 | R1 |
| History edits | R2-A Plan beside preview; autosquash; newest first; force-with-lease offered; instant single-commit edits | "A, as picked"; "Apply automatically"; "Newest first"; "Offer it right away"; "Apply at once, undo with Ctrl+Z" | 2026-11-21 | R2 |
| Many projects | R3-A Library + nested folders, one per repo; local-only status; reopen tabs | "A, as picked"; "One folder per repo"; "Nest freely"; "Local only, no auto-fetch"; "Reopen last session's tabs" | 2026-11-21 | R3 |
| Conflicts | R4-A Property table; Smart Merge offered at first merge; auto merges listed collapsed; verify blocks commit; three-column code merge | "A, as picked"; "Offer at first merge"; "Listed, collapsed"; "Block until confirmed"; "Three columns" | 2026-11-21 | R4 |
| Bitbucket | Repos are on Bitbucket Cloud: no LFS locks there | "Bitbucket Cloud" | 2026-11-21 | O1 |
| Phasing | v1.0, then v1.1 (Unity structural work + agents) | "v1.0 then v1.1" | 2026-11-21 | section 7 |
| Op log | Split: index copy before every action, working-tree snapshot only before destructive actions | "Split it" | 2026-11-21 | spike S1, section 3.3 |

Answers given as option picks are quoted as the option label Max chose.

## 10. Review

Adversarial review by a separate agent, 2026-11-21. Fixed in this revision:

- Labels:
  - tiers are picks, not decisions;
  - "came with" parts are separated from the explicit answers;
  - sources are corrected;
  - Max's words are added to section 9.
- Missing git operations added and tiered: merge, rebase onto, pull strategy, revert, reset, tags,
  file history / blame, open / create PR in the browser.
- The op log has a defined snapshot scope, retention and cost targets, plus spike S1.
- Agent rows: untracked files in `status`, and replaying a proposal when HEAD moved.
- Keyboard:
  - focus rule for text fields;
  - WebView accelerators turned off;
  - layout-safe conflict keys;
  - missing keys added: push / pull / fetch, pane focus, split / unified, shortcut sheet, redo,
    reject, stage all, discard file;
  - the multi-window key dropped.
- Performance targets restated with their preconditions; LFS, cold-cache, memory, start-up and
  image targets added.
- Lock and remote freshness defined for open tabs.
- Conflicts placed outside the inspector; screens list completed.
- Undo scope defined per repo and for the Library.
- Bundled git-lfs and the Unity detection rule added.

Raised with Max and answered the same day: O1 ("Bitbucket Cloud") and section 7 ("v1.0 then v1.1").

Rejected:

- "Q6 named no formats": Max's chosen Q6 option listed PNG/TGA/flat PSD. The row stays a pick
  anyway, since it adds JPG and Rust decoding.
- "Ask which keyboard layout Max uses": avoided instead. No shortcut needs brackets or AltGr.
