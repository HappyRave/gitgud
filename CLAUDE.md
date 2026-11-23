# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

gitgud is a desktop Git GUI. The repo was wiped on 2026-11-21 for a fresh start; the earlier
prototype is still in git history (`5f1257d` and before). Built so far: locating the system git;
multi-repo workspace with always-first Library tab (`library.rs`), nested folders, drag-and-drop organization, smart filters (`Uncommitted` / `Not pushed` / `Locks I hold` / `Unity projects` / `Missing on disk`), local-only visible-row status refresh + `Fetch visible`, folder scanner (`ScanFolderDialog`), `git clone --progress` with `core.longpaths` and `GIT_LFS_PROGRESS` (`clone.rs`, `CloneDialog`), Library undo/redo (`Ctrl+Z`/`Ctrl+Y`), open repo tabs (`TabBar`, `Ctrl+P` `QuickSwitch`, `Ctrl+Tab`/`Ctrl+Shift+Tab`, `Ctrl+W`, session tab restore) or `GITGUD_OPEN=<path>` at launch; the commit graph in the centre
(topo order, lanes laid out in Rust, canvas lane column, rows fetched on scroll, WIP row on top);
a side pane with the working copy (file and hunk stage / unstage, working-copy diffs, commit box with `⋯` options for amend / `--signoff` / `--no-verify`)
or, when a commit row or stash is selected, its message, files and text diff (`CommitDetail` / `StashDetail`); live refresh on
outside changes; the op log with Ctrl+Z / Ctrl+Y for every local write; discard of files and hunks,
undoable through work-tree snapshots; the op-log panel (Ctrl+3, from the R1 state 6 mock:
Undo / Restore per entry, "Restore these files" for discards); a sidebar from the R1-A mock
(branches in folders with ↑↓ against upstream + `+` / create / rename (`F2`) / delete (`Del`, undoable) / set upstream / checkout remote, remotes, stashes section, op-log entry; double-click checks a
branch out, undoable); fetch, pull (`pull.rebase` / `pull.ff` / divergence strategy dialog), and push (`--force-with-lease`, `--set-upstream`, Done-panel Force-push) with progress and cancel; open / create PR links in browser (GitHub and Bitbucket Cloud URL formats); detached HEAD banner with "Create branch…"; merge (right-click a branch → dialog with
conflict prediction; a stopped merge shows the R4-A banner and conflict view: list by kind,
three-column text merge with an arrow per hunk and "use both", pick-a-side for binaries and
one-sided deletes, summary with the prefilled message; every pick, abort and the commit undoable);
rebase onto a branch (same menu → dialog predicting each step; a stopped step shows the R1 state 1
banner with Continue / Skip commit / Abort over the same conflict view; every step undoable);
cherry-pick and revert commit with conflict prediction and stopped `SequencerBanner` + conflict resolution;
reset branch (soft / mixed / hard, undoable); stash save / apply / pop / drop with dashed graph rows;
the image diff (side by side / swipe / onion) for binary images in staging, commits and the
conflict side picker; the R2-A history plan (Shift+E or right-click a commit → plan in the
inspector, P/R/E/S/F/D, Alt+↑↓ or drag, fixup!/squash! applied on open, onto chip, "Update
dependent branches", per-step prediction with "Show the conflict" / "Keep the old base" / "Start
anyway", dashed after rows over faded commits in the graph, Done panel; an Edit stop keeps the
normal layout under the R1 banner) and the single-commit quick edits (R on the tip renames inline,
R on an older commit opens the plan, F / D apply at once, all undoable); Ctrl+F find in the history (subject, author, hash prefix, ref name); the
working-copy keys of spec §5 plus Ctrl+2 / F6 / Ctrl+C / Ctrl+Shift+L / Ctrl+Shift+P / Ctrl+B; the ultrawide inspector
split (≥ 2400 px); the strip's upstream / last-fetch line. Styling follows the lab
mocks (tokens in `src/app.css`, light and dark); the lane colours are the mock's, never given a
design round. Landed in M1–M6: command palette (`Ctrl+K`), shortcut sheet (`?`), line staging, syntax+intraline diff, plain-English error banner, settings modal, Unity Smart Merge / `.meta` guard / LFS locks / TGA+PSD previews, tags, file history & blame, op-log pruning + outside reflog recovery, commit-graph optimization, Unity YAML structural prefab/scene diff, R4-A property table resolver, post-merge integrity check, Unity repo setup, agent proposals (CLI / MCP / review inbox / ghost graph rows), stacked branches with `--update-refs` restack & per-level PR links, quick-edit force-push offer, LFS cache prune, AI commit message drafting, coding agent setup in Settings, faded "still on origin" commits after rewrites, first-open commit-graph prompt on large repos, manual index refresh, local latency diagnostics, linked agent worktrees -> proposals, R5 UI polish & `docs/lab/polish.html` preview, WebView2 browser accelerator disablement, custom level-dots app icon, bundled MinGit (`2.56.1`) + `git-lfs` (`3.8.1`) fallback (`git.rs` / `cli.rs`), Windows NSIS + portable packaging (`pnpm package` / `pnpm package:portable` / `scripts/package-portable.ps1` / `src-tauri/tauri.bundle.conf.json` / `src-tauri/windows/hooks.nsh` registering `$INSTDIR` + `gitgud-cli`), snapshot pruning in `OpLog::prune_now` / `Snapshots::sync_kept`, `O2`/`O6`/`O7` agent ergonomics (`stable_hunk_id_at`, `proposal_wait` / `--wait`, `agent hook` / `agent setup --with-hooks`), fast PNG checksums + TGA/PSD/150-repo/complex-Unity stress tests, M1–M9 E2E specs isolated via `GITGUD_LIBRARY`, `Graph.svelte` virtualized ARIA + reactive DPR/`ResizeObserver`, LFS-only op-log storage & reclaimable prune preview, Settings opt-in Claude Stop hook toggle (`agent_setup` `withHooks`), and strict hunk-ID + release-gated bundled-Git guards. Not yet: history edits over merge commits or the root commit (refused for now, Max 2026-11-22).
Update this file as pieces land.

The approved product spec is `docs/spec.md` (2026-11-21); design verdicts are in `docs/lab/decisions.md`.

## Stack

- Tauri 2 shell, Rust backend, SvelteKit frontend (Svelte 5, `adapter-static`), pnpm.
- Git access is split by kind of operation:
  - **Reads** (status, log, commit graph, diff, blame): the `gix` crate (gitoxide), in-process.
  - **Writes and network** (commit, fetch, push, rebase, merge, stash): spawn the `git` CLI, so
    hooks, credential helpers, SSH, signing and user config behave exactly as in a terminal.
    The system `git` is used when it is ≥ 2.45.2; bundled MinGit (`2.56.1`) + `git-lfs` (`3.8.1`) in
    `src-tauri/resources/git/` is the fallback via `git::locate()` and `GitCli::command()`.
  - Not `git2`/libgit2: no hook support, C build, credential friction.

## Layout

```
Cargo.toml              workspace root
crates/gitgud-core/     all git logic, plain Rust, no Tauri dependency
crates/gitgud-cli/      CLI + stdio MCP server binary (`gitgud`) for coding agents
src-tauri/              thin Tauri layer: #[tauri::command]s that call gitgud-core
src/                    SvelteKit app; lib/api.ts wraps invoke() for every command;
                        lib/dev/mock.ts is a fake backend for `pnpm dev` + `/?mock` (or `?mock=merge`)
e2e/                    real-app tests: Playwright attached to the debug build over WebView2 CDP
spikes/                 throwaway research code, not in the workspace
package.json
```

Logic belongs in `gitgud-core`; `src-tauri` only adapts it to commands, and the Svelte side only
calls commands and renders. Core stays testable without compiling the webview.

Open repos live in `session::Session` (Tauri managed state), by id: `open_repo` walks the history
once, and later commands (`repo_status`, `graph_rows`, ...) take the id. The graph keeps topology
plus lane checkpoints every 256 rows (`graph::Layout`), so any window of rows is laid out by
replaying at most 255 rows (spike 4.3).

- **Writes** go through `Session::write(id, action, op)`: one writer per repo, watcher paused, the
  op recorded in the op log as `action`, then `Session::refresh`. Ops use `cli::GitCli` (paths via
  `--pathspec-from-file` on stdin under `--literal-pathspecs`; git's own message is the error).
  Write commands return the refreshed `Opened`. User-facing refusals are `Error::Refused`.
- **Op log** (`oplog.rs`, spec §3.3): before and after each write it keeps HEAD, the branch tip and
  a file copy of `.git/index` in `.git/gitgud/oplog/` (no git process). Undo / redo refuse when
  HEAD or the staged entries changed outside gitgud (stat-only index rewrites from `git status`
  don't count) and respect git's `index.lock`. The spec says "under a private ref"; entries are
  files instead so recording stays ≤ 10 ms on big indexes (open for Max). Entries also keep git's
  merge files (`MERGE_HEAD`, `MERGE_MSG`, `MERGE_MODE`, `gitgud/merge.json`), so undo moves a
  merge into or out of progress exactly.
- **Merge** (`merge.rs`): `git merge --no-edit` (a clean merge commits at once, hooks run);
  prediction with `merge-tree --write-tree --name-only`; conflicts classified Unity YAML / code /
  binary (LFS, binary, or deleted on one side) and remembered in `.git/gitgud/merge.json`; text
  conflicts chunked by `git merge-file --diff3` over the index stages, with 31-character markers so
  marker-like lines in files can't confuse the parser. Resolving writes through the to-worktree
  filters (autocrlf) and stages; start, picks and abort are destructive writes over the files the
  merge can touch. The frontend keeps per-hunk picks locally until "Save and mark resolved".
- **Rebase** (`rebase.rs`): prediction replays the commits `rebase` would pick (`rev-list
  --cherry-pick --right-only --no-merges`) with `merge-tree --merge-base=<parent>` over trees, so
  it names the first conflicting step without writing anything. `git rebase` runs with
  `GIT_EDITOR=true`; the conflicts of the step it stopped on are kept in `.git/gitgud/rebase.json`.
  The op log keeps the files of `.git/rebase-merge` and the rebased branch's tip, so undo / redo
  move through a stopped rebase exactly (HEAD is detached meanwhile).
- **History edits** (`history.rs`): `history_plan(from)` lists `from^..HEAD` newest first with
  `fixup!` / `squash!` moved under their target (git's autosquash rules). Prediction replays the
  steps onto `onto` with `merge-tree --merge-base=<commit>^`, keeping the conflicted tree so "Show
  the conflict" reads the file with markers. A run is `git rebase -i --onto` with the todo written
  through `GIT_SEQUENCE_EDITOR` (our own `update-ref` lines with `--update-refs`); every rebase's
  `GIT_EDITOR` is a plain-`sh` script that hands git `gitgud/messages/<id>` for the step it asks
  about (a reword, or the last step of a squash chain), else keeps git's message, so commit-msg
  hooks still run. The preview is `Graph::preview`: ghost rows spliced into the topology, edge
  strokes (`Stroke::Dashed` / `Faded`) carried down the layout's lanes; `preview_rows` serves it.
  The op log records every local branch tip, so undo also moves branches `--update-refs` moved.
- **Destructive actions** (discard) go through `Session::write_destructive(id, action, paths, op)`:
  `snapshot::Snapshots` records the whole work tree (tracked + untracked, not ignored) before and
  after, via a persistent snapshot index and `add -A` / `write-tree`, chained under the private ref
  `refs/gitgud/snapshots` so `git gc` keeps them. Undo restores only the action's `paths`, and
  refuses if those files changed since. Snapshot pruning with the op log's retention isn't built.
- **Working copy** (`worktree.rs`): diffs per side (staged: HEAD → index; unstaged: index → work
  tree after git's to-git filters, so autocrlf files diff like `git diff`). Both hunks and individual
  clicked/Shift+clicked lines stage, unstage, and undoably discard through `git apply --cached` /
  worktree patches rebuilt from exact bytes; stale hunks are refused (`Error::Stale`).
- **Config & diagnostics** (`config.rs`): reads/writes `user.name` and `user.email` at global or
  repo-local scope (`Session::set_repo_identity` triggers config reload), and reports installed git
  and git-lfs paths/versions.
- **Unity & LFS** (`unity.rs`, `unity_yaml.rs`, `lfs.rs`): detects `ProjectVersion.txt`, `Temp/UnityLockfile` (guards
  checkout/merge/rebase touching `Assets/`), Hub `UnityYAMLMerge.exe`, Smart Merge setup in local
  `.git/config` + `.git/info/attributes` (never `.gitattributes`), v1.0 per-property verification in
  `merge.json` gating `merge_commit`, `.meta` auto-pairing (`Session::{stage,unstage,discard}`) +
  non-blocking `.meta` warnings, Git LFS locks (disabled on Bitbucket Cloud per O1) + unlock-on-push
  + inline unlock suggestion (`WorkingCopy.svelte`), quiet non-interactive 10-min `background_refresh`
  (`cli::GitCli::run_background`), pure-Rust multi-document Unity YAML structural diff (`UnityDiffView.svelte`)
  and GUID/`fileID` resolution (`GuidIndex`), R4-A interactive Property Table merge (`UnityMergeTable.svelte`
  with `1`/`2`/`E` + Raw YAML fallback), post-merge duplicate-`fileID` / broken-reference check, and
  one-click Unity `.gitignore`/`.gitattributes` + `Force Text` check (`SettingsModal.svelte`).
- **Network** (`task.rs`, `job.rs`): long git runs parse `--progress` and cancel by killing a
  Windows Job Object (git starts suspended, joins the job, resumes), which also ends grandchildren
  `taskkill /T` misses. A cancelled fetch deletes its `tmp_pack_*` files.
- **Refresh** rebuilds the graph only when refs / HEAD moved or the tree turned clean / dirty
  (`Graph::is_current`), and bumps `graphVersion`; the frontend remounts the graph on a new version.
- **Live refresh**: `watch::RepoWatcher` (notify) watches the root, each non-ignored top-level
  folder, and `.git` HEAD / index / refs, debounced 150 ms (max 1 s in a storm), and the app emits
  `repo-changed` with the repo id (spike 4.4).
- **Images** (`image.rs`, `src-tauri/src/images.rs`): the webview loads image versions from the
  custom protocol `gitgud` (`http://gitgud.localhost/image?repo=&path=&at=` on WebView2; `at` is
  head / index / worktree / yours / theirs / commit / parent), not over IPC. LFS pointers are read
  from `.git/lfs/objects` (and `.tga` / flat `.psd` decode purely in Rust to PNG); a version that
  isn't there answers 404 with the reason as text.
- **Diffs** (`diff.rs`, `$lib/diff/*`) are gix histogram + slider heuristics, 3 lines of context, against
  the first parent, no renames, with optional whitespace normalization (`ignoreWhitespace`) keeping
  original line text. The frontend adds syntax tokenizing (`highlight.ts`), word/token intra-line spans
  (`intraline.ts`), responsive auto-split at ≥ 1400 px, line selection, `Ctrl+K` command palette
  (`CommandPalette.svelte`), `?` shortcut sheet (`keymap.ts`, `ShortcutSheet.svelte`), `Ctrl+,`
  settings (`SettingsModal.svelte` for identity/theme/density/diff/commit-graph cache), and
  plain-English git error explanations (`errors.ts`, `ErrorBanner.svelte`).
- **Stacks, LFS Cache, AI Drafting & Agent Setup** (`stacks.rs`, `lfs.rs`, `ai.rs`, `remote_url.rs`):
  detects stacked branch chains rooted on trunk (`depth >= 2`), builds per-level GitHub/Bitbucket
  compare PR URLs (`pr_url_with_base`), and restacks chains undoably via `git rebase --update-refs`;
  inspects `.git/lfs/objects` size and runs `git lfs prune`; detects installed coding CLIs
  (`claude`, `codex`, `gemini`) to draft commit messages from `--cached` diffs without reading API
  key values; writes agent setup instructions (`SKILL.md` / `gitgud.mdc` / `GITGUD_AGENT.md`) from
  both CLI and Settings; refreshes index stats (`update-index --refresh`), highlights commits
  still on origin (`stale_origin_commits`) as faded graph rows after rewrites, prompts to build
  `commit-graph` on repos >= 500 rows, records capped local latency samples (`recent_timings`), and
  imports dirty linked worktrees (`worktrees_list` / `propose_from_worktree`) into proposals.
- **Tags, File History & Blame, Reflog & Commit-Graph** (`tags.rs`, `blame.rs`, `oplog.rs`,
  `ops.rs`): lightweight/annotated tags with create/delete/push (`TagDialog.svelte`, `Sidebar.svelte`),
  `--follow` file commit history and porcelain blame annotations (`FileHistoryBlame.svelte`), op log
  storage stats & 30d/2000-entry pruning plus `git reflog` recovery (`OpLogPanel.svelte`), and
  optional `git commit-graph write --reachable --changed-paths` optimization.
- **Agent Proposals & CLI/MCP** (`proposals.rs`, `crates/gitgud-cli`, `ProposalView.svelte`): agents
  inspect `status` (incl. untracked files without `git add -N` and stable hunk IDs), `diff`, `log`,
  `show`, `locks`, and submit isolated proposals built via temporary `GIT_INDEX_FILE` under
  `refs/gitgud/proposals/<id>` + `.git/gitgud/proposals/<id>.json` without touching the user's index
  or branches. Never exposes commit/checkout/merge/rebase/push over CLI/MCP. Non-blocking Unity risk
  flags (`.meta` pairs, unlocked LFS files, deleted files), sidebar Inbox (`Sidebar.svelte`), dashed
  ghost rows (`Graph.svelte` / `topology.rs`), 3-way replay onto moved `HEAD`, and undoable `approve` /
  `request_changes` / `reject` in `ProposalView.svelte`.

## Commands

```bash
pnpm install
pnpm tauri dev                      # run the app
pnpm tauri build                    # release build
cargo test --workspace
cargo test -p gitgud-core <name>    # single Rust test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm check                          # svelte-check
pnpm vitest run
pnpm vitest run <file> -t "<name>"  # single frontend test
pnpm e2e                            # debug build, then e2e/*.test.ts against the real window
pnpm dev                            # then /?mock or /?mock=merge: the UI on a fake backend, in a browser
                                    # (/?mock: Shift+E on a commit opens a history plan)
```

The first `pnpm dev` takes about a minute to pre-bundle dependencies before it listens.

Every feature is tested at each layer it touches:

- **Core** (`crates/gitgud-core`): unit tests next to the code, integration tests in `tests/`. They
  use the real `git` in temp dirs, never a mocked git. `testing::TestRepo` (feature `test-util`)
  builds throwaway repos through the git CLI, with a fake clock so commit dates (and so orders) are
  deterministic. Gix-based reads are checked against the CLI as an oracle: `cli_status()` parses
  `git status --porcelain=v2`, `cli_topo()` parses `git log --topo-order`, diffs are compared with
  `git diff`. Writes run with real hooks (`.git/hooks` scripts) in tests. Pure algorithms get
  `proptest` property tests (graph layout invariants; diffs that must rebuild both sides); keep
  `proptest-regressions/` and `tests/*.proptest-regressions` committed. Watcher tests are
  timing-based: wait generously for an event, use a quiet window to show none came. Remotes in
  tests are bare clones in temp dirs; a hanging remote is an `ext::sh <script>` transport
  (`protocol.ext.allow=always`) whose script records its processes' `/proc/$$/winpid`, so cancel
  tests can check with `tasklist` that nothing survived.
- **Commands** (`src-tauri`): tests go through Tauri's real IPC path (`tauri::test`, the `Ipc`
  helper in `lib.rs`) with JSON args as the webview sends them, on `TestRepo`s. Commands are `async`
  so they stay off the main thread. `build.rs` hands the Windows manifest to the linker for every
  target; without it, test binaries die with `STATUS_ENTRYPOINT_NOT_FOUND`.
- **Frontend** (vitest + jsdom + Testing Library): `$lib/test/ipc.ts`'s `mockCommands` answers
  commands from typed handlers. An unmocked command fails the test. Add every new command to
  `Commands` in `api.ts`, plugin commands (`plugin:dialog|open`) included. Logic sits in plain TS
  modules with their own tests (`$lib/graph/*`, `$lib/merge/*`, `keys.ts`, `working.ts`);
  components stay thin. Global keys are tested by firing on `window`. jsdom has no canvas or
  layout: the setup installs a recording 2D context (`$lib/test/canvas.ts`), and tests stub
  `clientHeight`. `mockCommands` mocks events too: `emit("repo-changed", id)` reaches the page.
- **Real app** (`e2e/`): `launchApp(env)` starts `target/debug/gitgud.exe` with a throwaway WebView2
  profile, and Playwright drives it. `e2e/repo.ts` mirrors `TestRepo`; `generate(n)` makes large
  histories with `git fast-import`. Pass `GITGUD_OPEN` to open a repo, since native dialogs can't
  be driven. Performance targets from spec §6 are asserted here (10k commits: open + first rows
  ≤ 300 ms; an outside edit visible ≤ 300 ms). Files run one at a time (one CDP port).
  Screenshots go to `e2e/out/` (ignored). Windows only.
