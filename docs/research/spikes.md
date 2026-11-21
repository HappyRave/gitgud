# Spikes (plan section 4)

Run 2026-11-21 on Max's PC (Windows 11, 28 threads, NVMe; Git 2.45.1, git-lfs 3.5.1, Rust 1.98.1,
Edge for WebView2-equivalent rendering). Throwaway code, as run, is in [`spikes/`](../../spikes) (paths in
the scripts point at the session scratchpad). All numbers are `fact` (measured here, warm cache unless
stated); verdicts are `Claude's pick`.

Test repos: **rust-lang/rust** (349,307 commits, 64,797 files, 1.2 GB `.git`) and **Unity-Technologies/
BoatAttack** (Unity, 1,376 files, 124 prefabs, 734 `.meta`, 185 LFS files = 903 MB).

## Summary

| Spike | Verdict | Effect on stack |
|---|---|---|
| 4.1 Old prototype | Keep ideas, no code | Confirms `CLAUDE.md` |
| 4.2 `gix` reads | Fast enough for log/graph, status, diff, blame. Two traps: rename detection is slow, and status doesn't refresh the index | Keep `gix` for reads, with two rules (below) |
| 4.3 Graph rendering | Webview copes with 349k commits at display refresh. Canvas is steadier than DOM. Lane layout must be lazy | Keep Tauri/WebView2; canvas graph column |
| 4.4 File watching | `notify` survives a 160k-event storm, no overflow; Unity `Library/` is 98% of reimport noise | Keep `notify`; watch per top-level dir, skip ignored |
| 4.5 `git` CLI | Progress parses cleanly; prompts fail fast when disabled; kill tree is clean except leftovers | Keep CLI for writes; cancel = kill tree + cleanup |
| 4.6 Unity + LFS | UnityYAMLMerge works as an automatic driver **only with `--force`**; merges 3 of 5 conflict cases; silently loses data in 1 | Ship the driver + a post-merge check |
| 4.7 Agent CLI/MCP | One core, two adapters works; proposal under hidden ref leaves Max's index untouched | Keep the design from agents.md §6 |
| 4.8 Op-log snapshot | Each `git` process costs ~23 ms on Windows: a CLI snapshot is ~200 ms clean, 0.5–1.4 s with changed LFS files, and every changed LFS version is stored in full | Split the op log: index copy for most actions, working-tree snapshot only before destructive ones (decided) |
| 4.9 Library refresh | 152 repos incl. rust refreshed in 0.5–1 s on 8 threads; one screen of rows in ~160 ms | Spec targets hold |

## 4.1 Old prototype (`5f1257d`, 10 commits, 2025-12-14)

**Question:** what worked, what to keep. **Method:** read the tree at `5f1257d`.

**Numbers:** `git_operations.rs` 708 lines, 17 commands in one file. `RepositoryDetail.svelte` 1,009 lines
(staging, diff, commit, push, credential form in one component). Two placeholder test specs, no Rust tests.

| Area | What it did | Verdict |
|---|---|---|
| Git access | `git2` 0.19 for everything | drop. Pull handled fast-forward only (`"Merge required (not implemented in MVP)"`); hooks never ran |
| Auth | HTTPS PAT typed into a form, saved by `tauri-plugin-store` (plain JSON on disk) | drop. The `git` CLI + Git Credential Manager removes this feature (4.5) |
| Remote / log / state | hard-coded `origin`; log limit 50, no graph; Svelte 4 stores, re-invoke everything after each action | drop |
| Diff view | side-by-side, two line-number columns, sticky hunk headers | keep the idea |

## 4.2 `gix` 0.90 reads on rust-lang/rust and BoatAttack

**Question:** do status, diff, blame and log/graph walks work in `gix`, and how fast vs the `git` CLI?
**Method:** [`spikes/gixbench`](../../spikes/gixbench), release build, vs `git` 2.45.1 doing the same work.

| Operation (rust-lang/rust) | `gix` | `git` CLI | Note |
|---|---|---|---|
| Open repo | 20 ms | — | |
| Status, clean tree, 65k files | 278 ms | 106 ms | First run after checkout: `gix` 6,957 ms (cold FS cache) |
| **Graph topology, all 349k commits** (ids + parents) | **106 ms** | 448 ms (`log --format=%H %P`) | with commit-graph file |
| Full walk incl. message + author, 349k | 1,855 ms (commit-graph) / 2,624 ms (none) | 2,367 ms | Only visible rows need metadata |
| First 500 commits with metadata | 2.3 ms | 44 ms | Opening a repo feels instant |
| Tree diff HEAD^..HEAD | 1.4 ms | — | |
| Tree diff HEAD~200..HEAD, **no renames** | 23 ms | 99 ms | 11,631 changes |
| Tree diff HEAD~200..HEAD, **with rename detection** | **3,588 ms** | 274 ms | `gix` 13× slower |
| Blame `README.md` (33,628 commits traversed) | 744 ms | 418 ms | |
| Write commit-graph | — | 2.4 s, once | Not present after clone |

| Operation (BoatAttack, LFS) | `gix` | `git` CLI |
|---|---|---|
| Status, clean | 26–31 ms | 41 ms |
| Status after touching all 185 LFS files (same content, new mtime, as a Unity reimport does) | **1,312–1,441 ms, every call** | 1,902 ms once, then 42 ms |

**Finding:** `gix` status is read-only, so it never writes refreshed stat data back to the index. Each call
re-hashes every stat-dirty LFS file through the filter. `git status` rewrites the index once, and later
calls are cheap again. `gix` still reported 0 false changes and caught a real 1-byte edit, so the LFS filter
is applied correctly.

**Verdict:**
- `gix` for log and graph, diff, blame and status.
- Turn rename detection off for big ranges, or cap it (`diff.renameLimit` style), and run it lazily per file.
- After a slow status, or when the watcher reports a mass touch, refresh the index once. Either run
  `git update-index -q --refresh` through the CLI path, or write the index with `gix`. Untested: whether
  `gix` 0.90 can write the refreshed index.
- Write the commit-graph on first open, with one confirm (it took 2.4 s here, once).

## 4.3 Commit graph rendering at 349k commits

**Question:** canvas vs virtualised DOM, scroll smoothness, first paint. **Method:**
[`spikes/graph/bench.html`](../../spikes/graph/bench.html) in a separate Edge window (the same Chromium engine
as WebView2), 1384×861, occlusion throttling off. Data: rust-lang/rust topo-order log (34 MB TSV). Each run
lasted 4 s: slow scroll at 2,000 px/s, fast scroll at 30,000 px/s, and a random scrollbar jump every frame.
The display refresh was 100 Hz (10 ms frames).

**Finding first:** rust's history is wide. Topo order holds about 100 concurrent lanes on average and 185 at
peak, which comes to **35M line segments**. Precomputing segments per row does not fit in a JS array
(RangeError). The working design stores a lane snapshot every 256 rows and replays at most 255 rows per
frame, then draws only the visible rows. Width is capped at 32 drawn lanes.

| | Canvas | Virtualised DOM (SVG per row) |
|---|---|---|
| Parse 349k rows (JS) | 368 ms | 355 ms |
| Lane layout, checkpoints only | 33 ms | 33 ms |
| First paint | 5 ms | 3 ms |
| JS heap | 123 MB | 122 MB |
| Slow scroll: fps / p95 frame / frames > 17.5 ms | 99.6 / 10.1 ms / 0% | 99.8 / 10.1 ms / 0% |
| Fast scroll | 99.7 / 10.1 / 0% | 99.9 / 10.1 / 0% |
| Scrollbar jumps | 99.7 / 10.1 / **0%** | 96.6 / 10.1 / **3.4%** |
| Render time p95 (script only) | 0.7–1.4 ms | 0.3–4.5 ms |

**Verdict:**
- WebView2 is not the bottleneck; Tauri stays.
- Draw the graph column on canvas. Text can be either, but canvas held 0% slow frames on jumps.
- Compute lanes in Rust (`gix` topology takes 106 ms) and send the webview checkpoints plus visible rows
  on demand, not the whole history.
- Cap the drawn width and fold far lanes into an edge marker.
- `open`: test on a 4K monitor at DPR 2 and on a weaker laptop.

## 4.4 File watching (`notify` 9.0)

**Question:** event latency, and behaviour under storms. **Method:** [`spikes/watchbench`](../../spikes/watchbench),
one recursive watch on the repo root (ReadDirectoryChangesW, 64 KiB buffer).

| Test | Result |
|---|---|
| Watch setup on rust (65k files) | 0.2 ms |
| Single-file write → event | p50 0.30 ms, p95 0.35 ms, max 0.46 ms |
| `git checkout HEAD~3000` on rust (46,926 files changed, 18 s) | 136,596 events, **0 rescans, 0 errors** |
| Checkout back (19 s) | 160,651 events, 0 rescans, 0 errors |
| Simulated Unity reimport on BoatAttack: 20,000 files written to `Library/` + every `Assets/` file touched (7.2 s) | 62,779 events: **Library 61,581 (98%)**, worktree 1,198, `.git` 0 |

**Verdict:**
- `notify` is reliable on Windows.
- Don't watch the root recursively. Watch each top-level directory that isn't ignored (skip `Library/`,
  `Temp/`, `Logs/`, `obj/`), plus `.git/HEAD`, `.git/refs` and `.git/index` for outside git changes.
- Debounce about 150 ms, then run one `gix` status.
- Pause the watcher during gitgud's own git operations and refresh once when they finish.

## 4.5 Driving the `git` CLI: progress, credentials, cancel

**Method:** [`spikes/gitproc`](../../spikes/gitproc) spawns `git` with stderr piped, splits on `\r`/`\n`,
parses `phase: NN% (a/b), size | rate`.

| Test | Result |
|---|---|
| `clone --progress` rust-lang/rust | 230 progress updates in 15 s. Phase, percent, counts and rate parse cleanly (e.g. `Receiving objects 16% (579112/3619449), 295 MiB, 30.8 MiB/s`) |
| Cancel: `Child::kill` (git.exe only) | Children exited too (`fatal: early EOF`); 0 orphan processes |
| Cancel: `taskkill /T /F` | 170–190 ms, 0 orphans |
| Leftovers after cancel | Cancelled clone left its directory (61 KB to **310 MB**); cancelled fetch left `.git/objects/pack/tmp_pack_*` (128 MB). No `.lock` files seen |
| HTTPS auth with no helper, `GIT_TERMINAL_PROMPT=0` | Fails in 407 ms: `terminal prompts disabled` |
| HTTPS with GCM and `GCM_INTERACTIVE=never` | Fails in 589 ms; no window appears |
| SSH with `BatchMode=yes` | Fails in 427 ms with a readable host-key error |
| `git lfs pull` progress | Nothing on piped stderr. **`GIT_LFS_PROGRESS=<file>`** writes machine-readable lines (`download 4/183 20086015/49690226 <path>`), 4,920 lines in 8 s |
| `git lfs pull` cancelled at 8 s, then rerun | 0 files hydrated (LFS downloads everything before checkout); `.git/lfs/incomplete/` remains; rerun took the full 23 s |

**Verdict:**
- Always set `GIT_TERMINAL_PROMPT=0`.
- Background fetches also set `GCM_INTERACTIVE=never` and `ssh -o BatchMode=yes`. User-started push and
  fetch let GCM show its own window.
- Cancel = kill the process tree, then clean up: delete the clone directory, or delete `tmp_pack_*` newer
  than when the operation started.
- Read LFS progress from `GIT_LFS_PROGRESS`.
- Use Windows Job Objects in production. `taskkill` worked here, but a Job Object also catches grandchildren
  (`open`).

## 4.6 Unity + LFS (BoatAttack) and UnityYAMLMerge

**Clone / LFS:** clone (no smudge) 5.2 s; `git lfs pull` of 185 files / 903 MB in 23.3 s (about 39 MB/s).
Sourcetree comparison not run: it needs GUI driving, which is still pending (see below).

**Findings on this machine (`fact`):**
- **Git 2.45.1 breaks LFS clones.** The clone fails with "active `post-checkout` hook found during
  `git clone`". 2.45.2 reverted that change
  ([release notes](https://github.com/git/git/blob/master/Documentation/RelNotes/2.45.2.adoc)). Fix: upgrade
  Git (current is 2.56.1). gitgud should check `git --version` at startup and warn on known-bad versions.
- **rust-lang/rust fails to check out on Windows** without `core.longpaths=true` ("Filename too long").
  gitgud should set it, or offer it, on clone.
- **`GIT_CURL_VERBOSE=0` is set in your environment.** Git treats any value as "on", so every HTTPS
  operation prints curl traces. Harmless, but noisy, and probably slower.
- BoatAttack contains `AIcontroller.cs.meta` next to `AiController.cs`, a case mismatch. Windows hides it;
  case-sensitive filesystems break on it. The `.meta` guard should check case too.

**UnityYAMLMerge (Unity 6000.3.21f1) as a git merge driver.** Method:
[`spikes/scripts/unitymerge.py`](../../spikes/scripts/unitymerge.py). Each scenario uses two branches that edit
`ReplayCameras.prefab` (11 GameObjects) or `BaseButton_Finish.prefab` (a prefab instance with overrides).

- Without `--force` the driver **always fails**: git passes temp files with no extension
  (`.merge_file_XXXX`), and the tool answers "Don't know how to merge". Working config:
  `driver = "<Unity>/Editor/Data/Tools/UnityYAMLMerge.exe" merge -h -p --force --fallback none %O %B %A %A`
  (argument order: base, theirs, ours, dest).

| Scenario | Plain git | With driver |
|---|---|---|
| S1 two fields on one Transform (adjacent lines) | conflict | **clean, both kept** |
| S2 same field, different values | conflict | conflict, **0 markers** (file left as ours) |
| S3 both add a child GameObject to the same parent | conflict (3 hunks) | **clean, both kept, no duplicate fileIDs** |
| S4 prefab instance: both add different overrides | conflict | **clean, both kept** |
| S5 prefab instance: same override, different values | conflict | **clean, theirs silently lost** |

- On a real conflict, `-o <file> --describe` gives structured output, e.g.
  `571479311832868573.Transform.m_LocalPosition x change to 2.0` for one side and `1.0` for the other.
  gitgud can resolve the fileID to a GameObject name and show "BlimpCamera › Transform › Position.x:
  theirs 2.0 / yours 1.0" with pick buttons.

**Verdict:**
- gitgud registers the driver itself: it finds Unity in Hub installs and writes `.git/config` plus
  `.git/info/attributes`, so no tracked files change.
- After every clean driver merge, gitgud checks both sides' changes per propertyPath. When both sides
  changed the same one, it flags the file "merged, verify", which catches S5.
- When there are no markers, it shows the structured conflict list instead of a text editor.

## 4.7 Agent interface: one core, CLI + MCP

**Method:** [`spikes/agentcore`](../../spikes/agentcore). `core.rs` holds all the logic (git CLI only);
`main.rs` exposes it as a CLI and as an MCP stdio server (`rmcp` 3.11) with three tools: `status`, `propose`,
`proposals`. There is no commit or approve tool for the agent. The flow was driven by
[`spikes/scripts/agenttest.py`](../../spikes/scripts/agenttest.py), playing the agent.

1. The "agent" edits `Engine.cs` (2 hunks) and adds `Turbo.cs` without a `.meta`.
2. `status` returns hunk IDs: an FNV hash of the path and hunk body, without line numbers.
3. Over MCP: initialize, `tools/list`, then `propose` with 2 of the 3 hunks, a message and a "why". All
   round trips together took **177 ms**.
4. `propose` builds the commit in a **temporary index** (`GIT_INDEX_FILE`: `read-tree HEAD`, `apply --cached
   --recount`, `write-tree`, `commit-tree`) and stores it at `refs/gitgud/proposals/<id>`. Trailers:
   `Gitgud-Why`, `Gitgud-Base`.
5. Risk flag raised: `missing .meta for Assets/Scripts/Boat/Turbo.cs`.
6. Max's index stays **empty** and the worktree keeps all the edits.
7. `approve` (CLI, for the human) checks that HEAD hasn't moved and the index is clean, applies the proposal
   to the real index, then runs **`git commit`**, so hooks and signing run, and deletes the ref.
8. What's left unstaged is exactly hunk B. Its ID `5c7a403f` stayed the same even though its line numbers
   shifted (102 → 103).

**Limits found:**
- A new file only shows in `git diff` after `git add -N`, which touches the index. A real core needs its own
  untracked-file listing.
- When HEAD moves, the proposal has to be replayed (`git replay` / cherry-pick) rather than refused.
- Lock checks need the LFS lock API (not tested; BoatAttack has no lockable patterns).

**Verdict:** the design in agents.md §6 works as built. `rmcp` is easy to adopt (a derive-style
`#[tool_router]`). Keep the logic in `gitgud-core`, with thin CLI and MCP adapters.

## 4.8 Op-log snapshot cost (spec S1, 2026-11-21)

**Question:** what does a snapshot before each action cost on a Unity + LFS repo, and how fast does
`.git` grow? **Method:** [`spikes/scripts/snapbench.py`](../../spikes/scripts/snapbench.py) on a fresh
BoatAttack clone, Git 2.56.1, git-lfs 3.8.1. A snapshot is: copy `.git/index` to a temp index,
`GIT_INDEX_FILE=<tmp> git add -A` (runs the LFS clean filter on changed files), `write-tree` (temp and
real index), `commit-tree`, `update-ref refs/gitgud/oplog`. That's 6–7 `git` processes.

| Measurement | Median |
|---|---|
| Spawn `git --version` / `git rev-parse HEAD` | **23 / 25 ms per process** |
| Stage one hunk: `git apply --cached` | 27 ms |
| Snapshot, clean tree | **200 ms** (add -A 31, write-tree ×2 55, rev-parse + commit-tree + update-ref 114) |
| `git stash create` (one process, no untracked files), for reference | 56 ms |
| Snapshot, 3 dirty `.cs` files | 199 ms |
| Snapshot, 1 changed LFS file (54 MB) | 552 ms (clean filter ≈ 340 ms) |
| Snapshot, 20 changed LFS files (673 MB) | 1,416 ms |
| `.git` growth after those LFS snapshots | `.git/objects` +0 MB, **`.git/lfs/objects` +673 MB** |
| Snapshot right after a mass touch (all 185 LFS files, same content) | 1,868 ms, and 1,899 ms again on the next one (the temp index is re-copied from the stale real index) |
| Same, with the snapshot index **kept** between snapshots | **212 ms** |
| `update-index --refresh` on the real index after the touch | 1,689 ms, once |

**Findings (`fact`):**
- On Windows every `git` process costs ~23 ms before it does any work, so a CLI snapshot can't get near
  20 ms. Process count, not hashing, sets the floor on a clean tree.
- Every changed LFS file is stored in full in `.git/lfs/objects` on each snapshot that sees a new
  version. Snapshotting before every action would copy big binaries again and again.
- A temp index re-copied from a stale real index re-hashes every touched LFS file on every snapshot. A
  persistent snapshot index avoids it.

**Verdict (decided 2026-11-21, Max: "Split it"; the spec's section 3.3 is updated):**
- Split the op log in two:
  - Before actions that only move refs or the index (stage, unstage, commit, branch ops), save the
    refs plus a **copy of the `.git/index` file** (about 1 ms, no git process).
  - Before actions that can lose working-tree content (discard, checkout over changes, hard reset,
    stash, rebase / merge start), also snapshot the dirty files through a **persistent snapshot index**.
- Snapshot LFS files only when they're dirty and about to be overwritten, so most actions copy no
  binary.
- Count LFS objects that only the op log uses in the op log's size, and prune them with its retention.
- An in-process `gix` snapshot (S1b) could drop the ~150 ms of process spawns. Not pursued: dropped by
  Max on 2026-11-21 ("we won't do them").

## 4.9 Library refresh with 150+ repos (spec S2, 2026-11-21)

**Question:** can the Library refresh branch, ahead/behind and dirty state for 150+ repos inside the
spec's targets? **Method:** [`spikes/librarybench`](../../spikes/librarybench) (`gix` 0.90, std threads)
over 150 synthetic repos ([`spikes/scripts/make_library.py`](../../spikes/scripts/make_library.py): 300 files,
100 commits, upstream equal / ahead / behind, 15 dirty) plus BoatAttack and rust-lang/rust.

| Run (152 repos) | 8 threads | 16 threads |
|---|---|---|
| Refs only: branch + ahead/behind | 386 ms (p95 45 ms per repo) | 190 ms |
| Refs + `is_dirty`, first run | 953 ms (max 491 ms: rust) | 542 ms |
| Refs + `is_dirty`, second run | 459 ms (p95 22 ms; rust 126 ms) | 499 ms |
| 150 synthetic only, refs + dirty | 327–361 ms | — |
| First 25 repos (one screen of rows), one thread, + dirty | 155–160 ms | — |

**Findings (`fact`):**
- A full refresh of 152 repos, including rust, fits in about 0.5–1 s with 8 threads. One screen of rows
  takes ~160 ms on a single thread.
- More threads help the refs-only pass and give nothing for the dirty check (disk-bound).
- Not measured: a truly cold disk cache, which needs a standby-list flush and admin rights; and 150 real
  Unity repos. BoatAttack (1.4k files) stays inside p95. A Unity repo right after a reimport pays the LFS
  re-hash (~1.4 s, section 4.2) until its index is refreshed.

**Verdict (`Claude's pick`):** the spec's Library targets hold:
- Draw from the cache at once, then refresh the visible rows (~160 ms), then the rest in the background
  on 8 threads.
- Refresh the index of a repo whose dirty check was slow (`update-index --refresh`), as `git status`
  does, so a reimport costs once and not on every refresh.

## Not done

- **Hands-on GUI round:** done 2026-11-21 for Fork, Sublime Merge and GitButler; see
  [handson.md](handson.md). Not run: Sourcetree (Max's work install) and lazygit (terminals are
  click-only for computer use). The test-repo setup script was removed on 2026-11-21.
- 4K/DPR 2 graph run; `gix` index write-back; Job Object cancel; LFS lock API against a real host.
