# Handoff: gitgud v0.1.2

Written 2026-11-22, updated 2026-11-23 (through `62c6239`, v0.1.2) for the next session picking up `gitgud`.

## Read first

1. `CLAUDE.md` — architecture, op-log invariants, module map, and commands.
2. `docs/spec.md` — approved v1.0 / v1.1 requirements, keyboard map (§5), performance targets (§6), and open questions (§8).
3. `docs/lab/decisions.md` — UI layout verdicts (R1–R4).

## Where things stand

- Branch `main` has **Milestones 1–9** implemented (`e00bdc3` through `3aa28c1`), covering all v1.0 and v1.1 functional rows in `docs/spec.md` plus `gitgud-cli` and its stdio MCP server.
- After M9: polish + NSIS packaging + full E2E pass (`4a739be`), portable zip/exe (`19fb83a`), v0.1.1 (`9512243`) and v0.1.2 (`62c6239`) — see "Since M9" below.
- M1–M9 were verified headlessly as they landed; the Playwright E2E suite (`pnpm e2e`) first ran in `4a739be` (33/33). **No interactive `pnpm tauri dev` walkthrough has happened yet.**

## What is built (Milestones 1–9)

| Milestone | Commit | Scope | Key files |
|---|---|---|---|
| M1 | `e00bdc3` | Pull, push, branch CRUD, reset, cherry-pick/revert sequencer, stashes, host PR links | `remote.rs`, `branches.rs`, `sequencer.rs`, `stash.rs`, `remote_url.rs` |
| M2 | `0fe173e` | Multi-repo Library, nested folders, smart filters, scan folder, clone + LFS progress, tab state + undo | `library.rs`, `clone.rs`, `LibraryView.svelte`, `TabBar.svelte`, `QuickSwitch.svelte`, `CloneDialog.svelte`, `ScanFolderDialog.svelte` |
| M3 | `88838a8` | Command palette (`Ctrl+K`), `?` shortcut sheet, syntax + intraline diff, line staging, error banner, settings | `CommandPalette.svelte`, `ShortcutSheet.svelte`, `SettingsModal.svelte`, `ErrorBanner.svelte`, `highlight.ts`, `intraline.ts`, `errors.ts` |
| M4 | `a9aa149` | Unity Smart Merge (`UnityYAMLMerge`) + verify gate, `.meta` guard/pairing, Git LFS locks + unlock-on-push, TGA/PSD decoding, background refresh | `unity.rs`, `lfs.rs`, `image.rs`, `images.rs`, `WorkingCopy.svelte`, `ConflictView.svelte` |
| M5 | `6855fc6` | Annotated/lightweight tags, `--follow` file history & blame, op-log 30d/2000 pruning + `git reflog` recovery, commit-graph write | `tags.rs`, `blame.rs`, `oplog.rs`, `FileHistoryBlame.svelte`, `TagDialog.svelte`, `OpLogPanel.svelte` |
| M6 | `cbb3075` | Pure-Rust multi-doc Unity YAML structural diff & GUID/`fileID` index, R4-A interactive Property Table merge, post-merge integrity check, Unity `.gitignore`/`.gitattributes` + Force Text check | `unity_yaml.rs`, `UnityDiffView.svelte`, `UnityMergeTable.svelte` |
| M7 | `0decc61` | Isolated agent proposals (`refs/gitgud/proposals/<id>` + JSON), untracked/hunk-ID `agent_status`, Unity/LFS risk flags, 3-way replay on moved `HEAD`, Sidebar Inbox, dashed ghost graph rows, `ProposalView`, `gitgud-cli` & stdio MCP server | `proposals.rs`, `crates/gitgud-cli/`, `ProposalView.svelte`, `topology.rs`, `Graph.svelte` |
| M8 | `bc045ba` | Stacked branches (`depth >= 2`), undoable `git rebase --update-refs` restack, stacked compare PR URLs, LFS cache stats/prune, AI CLI commit drafting (`claude`/`codex`/`gemini`), quick-edit Force-push offer, Coding Agents setup in Settings | `stacks.rs`, `ai.rs`, `lfs.rs`, `CommitBox.svelte`, `Sidebar.svelte`, `SettingsModal.svelte` |
| M9 | `3aa28c1` | Faded "still on origin" commits after rewriting pushed branches, linked agent worktrees -> proposals import, large-repo commit-graph prompt (`>= 500` rows), manual `update-index --refresh`, in-memory local latency panel, `F5`/`F7` browser key guard | `ops.rs`, `proposals.rs`, `session.rs`, `RepoView.svelte`, `Graph.svelte`, `SettingsModal.svelte`, `+page.svelte` |

## Since M9 (2026-11-23)

| Commit | Scope | Key files |
|---|---|---|
| `4a739be` | R5 UI polish (`docs/lab/polish.html`), Windows NSIS installer, bundled MinGit `2.56.1` + `git-lfs` `3.8.1`, `gitgud-cli` sidecar, app icon, O2/O6/O7 agent ergonomics, snapshot pruning, Graph ARIA + DPR scaling, E2E suite 33/33 | `scripts/`, `src-tauri/tauri.bundle.conf.json`, `src-tauri/windows/hooks.nsh`, `git.rs`, `snapshot.rs` |
| `19fb83a` | Portable zip + standalone exe (`pnpm package:portable`) | `scripts/package-portable.ps1` |
| `9512243` v0.1.1 | Draggable, persisted panel splitters (`prefs.panes`), op-log `Close` + sidebar `Working copy` toggle, `Ctrl`/`Shift`-click multi-select staging | `Splitter.svelte`, `prefs.svelte.ts`, `StatusList.svelte`, `WorkingCopy.svelte`, `working.ts`, `OpLogPanel.svelte` |
| `62c6239` v0.1.2 | Texture viewer: DDS/EXR/HDR/TIFF/16-bit PSD decode, channels, zoom, pixelated, diff heatmap; LFS objects `<= 25 MiB` auto-fetched (`lfs_fetch_object`); LFS pointer cards; asset + LFS conflict inspector in `SidePick` | `image.rs`, `lfs.rs`, `ImageDiff.svelte`, `LfsBinaryCard.svelte`, `SidePick.svelte` |

The v0.1.1 / v0.1.2 commits don't record which checks ran; re-run the full list below before trusting them.

## Latest verified checks (at `3aa28c1`; E2E at `4a739be`)

- `pnpm check` (`svelte-check`): **0 errors, 0 warnings**
- `pnpm vitest run`: **43 test files / 339 tests passed**
- `pnpm build`: clean `@sveltejs/adapter-static` production bundle (`build/`)
- `cargo fmt --all --check`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed (0 warnings)
- `cargo test -p gitgud-core --test fetch -- --test-threads=1`: **6 passed**
- `cargo test --workspace -- --test-threads=2 --skip cancel_kills_git_and_everything_it_started`: passed (`gitgud_lib` 35/35, `gitgud_cli` `cli_and_mcp` 2/2, `gitgud_core` lib 56/56, `stacks` 1/1, `worktrees` 2/2, plus all existing core integration suites)

## Known caveats & conventions

- `.claude/` is untracked local config; never stage or commit it (`git add` explicit paths).
- `crates/gitgud-core/tests/fetch.rs::cancel_kills_git_and_everything_it_started` can flake under parallel child-process load; run `--test fetch -- --test-threads=1` separately before the workspace suite.
- `Sidebar.test.ts` (`[{ cmd: "branches" }, { cmd: "stashes" }]`) and `OpLogPanel.test.ts` (`[{ cmd: "op_log" }]`) assert exact IPC call sequences. Never add new `invoke` calls inside `Sidebar.svelte` or `OpLogPanel.svelte`; load data in `RepoView.svelte` and pass props down.

## Things to do (gaps, verification & packaging)

1. **Playwright E2E & live WebView2 GUI audit (`pnpm e2e` 33/33 passed; manual theme/density/key check pending)**
   - Done: isolated `LibraryStore` via `GITGUD_LIBRARY`, ran full Windows WebView2 CDP E2E suite (`pnpm e2e` -> 33/33 passed in 130.6s across M1–M9, including `10k`-commit graph open in `125.6 ms` and outside-edit refresh in `202 ms`), and fixed app bugs surfaced by E2E (`(bundled)` status-strip spacing in `LibraryView`/`RepoView`, redundant raw `<details>` suppression in `ErrorBanner`, `watch.rs` `100 ms` debounce).
   - Remaining: launch `pnpm tauri dev` for visual spot-checks across Light/Dark themes, Standard/Compact row density (`24`/`20` px), ultrawide (`>= 2400 px`) layout, and interactive WebView2 accelerator keys (`Ctrl+P` / `Ctrl+F` / `Ctrl+R`).
2. **Real-world Unity + LFS testing & performance benchmarks (headless targets passed; live remote checks pending)**
   - Done headlessly: `2048 px` TGA decode (`52.5 ms <= 300 ms`) & `100 MB` flat PSD decode (`358.7 ms <= 1500 ms`) after `adler32`/`crc32` optimization, `150`-repo Library refresh (`446.0 ms` in release), and complex nested `PrefabInstance` + `stripped` + flow/sequence collection YAML stress tests.
   - Remaining: test against live Unity project files (`UnityYAMLMerge.exe`), verify GitHub LFS lock/unlock/unlock-on-push & Bitbucket Cloud lock-surface hiding (`O1`) against live remotes, and measure Sourcetree comparison baseline (`O9`).
3. **UI/UX design polish (`O3` in `docs/spec.md` — CSS/layout pass + `docs/lab/polish.html` done; live visual review pending)**
   - Done offline: CSS token/density/dark-theme/ultrawide fixes (`app.css`, dialogs, inspectors, lists), Linear-style `?` shortcut filter, Sublime Merge-style CLI hints in `Ctrl+K`, 2-column Settings rail + Git/LFS badges, viewport-clamped `ContextMenu`, click-outside `TabBar` overflow, aligned level-dots app icon, and `docs/lab/polish.html`.
   - Remaining: Max's live review in `pnpm tauri dev`.
4. **Release & installer packaging (implemented via `pnpm package` / `pnpm package:portable`; interactive installer test pending)**
   - Done: bundled MinGit `2.56.1` + `git-lfs` `3.8.1` fallback (`scripts/fetch-bundled-git.ps1`, `git.rs`, `cli.rs`, `ToolDiagnostics` / Settings / status strip), `gitgud-cli` sidecar (`scripts/prepare-bundle.ps1`, `src-tauri/tauri.bundle.conf.json`), custom level-dots icon (`src-tauri/icons/`), current-user Windows NSIS installer (`src-tauri/tauri.conf.json`, `src-tauri/windows/hooks.nsh` adding/removing `$INSTDIR` on user `PATH`), and portable zip + standalone exe builder (`scripts/package-portable.ps1`).
   - Remaining: rebuild (`pnpm package`; no bundle is on disk as of 2026-11-23), then manual interactive run/uninstall test of `target/release/bundle/nsis/gitgud_0.1.2_x64-setup.exe` on clean/target Windows user profile and code-signing decision if distributing broadly.
5. **Remaining open / follow-up engineering items**
   - History edits (`R2-A`) over merge commits or the root commit are intentionally refused for now (`CLAUDE.md`).
   - Snapshot pruning (`snapshot.rs::Snapshots::sync_kept`) is now wired into `OpLog::prune_now` & retention + `git gc --auto`, and `OpLog::stats` / `OpLogPanel` include LFS-only snapshot object sizes and reclaimable prune previews.
   - `O2` (line-shift + strict missing-ID hunk validation), `O6` (`proposal_wait` / `--wait`), and `O7` (explicit `propose` default + opt-in `agent hook`, `agent setup --with-hooks`, and Settings UI Claude Stop hook checkbox) are implemented per Max's 2026-11-23 decisions.
   - `O4` (`Graph.svelte` virtualized `aria-activedescendant`/`aria-setsize`/`aria-posinset`) and `O5` (reactive DPR `matchMedia` + `ResizeObserver` canvas scaling) code and unit tests are landed; live Windows Narrator and physical 4K monitor confirmation remain pending.
   - Deferred **later** items (`docs/spec.md` §3.6–§3.8, out of v1 scope unless requested): rendered prefab thumbnails (`Q6`), sparse checkout / `lfs.fetchinclude` UI (`Q7`), Unity Editor package (`Q5`), Artist mode (`Q3`), multi-window support.

## Things to do next (recommended first session order)

1. Re-run the full check list on v0.1.2 (fmt, clippy, cargo tests, `pnpm check`, vitest, `pnpm e2e`) and update "Latest verified checks".
2. Build with `pnpm package` and test `target/release/bundle/nsis/gitgud_0.1.2_x64-setup.exe` interactively on a Windows user profile (install, CLI `PATH`, uninstall).
3. Start `pnpm tauri dev`, open a real Unity + LFS repo plus a multi-repo Library folder, and do a structured walkthrough of M1–M9 themes/density, WebView2 accelerator keys (`Ctrl+P`/`Ctrl+F`/`Ctrl+R`), Settings' **Local latency** timings, and the v0.1.1/v0.1.2 pieces (splitters, multi-select staging, texture viewer on real DDS/EXR/PSD, LFS auto-fetch, LFS conflict inspector).
4. Address any layout polish or Unity YAML parser edge cases uncovered on real `.unity`/`.prefab` assets, plus live GitHub/Bitbucket remote LFS lock checks (`O1`) and Sourcetree baseline (`O9`).
