# Handoff: gitgud post-Milestone 9

Written 2026-11-22 for the next session picking up `gitgud`.

## Read first

1. `CLAUDE.md` — architecture, op-log invariants, module map, and commands.
2. `docs/spec.md` — approved v1.0 / v1.1 requirements, keyboard map (§5), performance targets (§6), and open questions (§8).
3. `docs/lab/decisions.md` — UI layout verdicts (R1–R4).

## Where things stand

- Branch `main` has **Milestones 1–9** implemented (`e00bdc3` through `3aa28c1`), covering all v1.0 and v1.1 functional rows in `docs/spec.md` plus `gitgud-cli` and its stdio MCP server.
- All M1–M9 work was verified headlessly (Svelte/TypeScript checks, Vitest, Vite production build, `cargo fmt`, `cargo clippy -- -D warnings`, and Rust unit/integration/IPC suites). **No GUI (`pnpm tauri dev`) or Playwright E2E (`pnpm e2e`) session ran during M1–M9** because desktop GUI launch was explicitly disabled while Max used the machine.

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

## Latest verified checks (at `3aa28c1`)

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

1. **Playwright E2E & live WebView2 GUI audit (high priority)**
   - Run existing `e2e/` suite (`pnpm e2e`) and launch `pnpm tauri dev` to inspect M1–M9 visually across Light/Dark themes, Standard/Compact row density (20/24 px), and standard vs ultrawide (`>= 2400 px`) layouts.
   - Expand `e2e/` tests to cover M1–M9 flows (Library/tabs/clone, Command Palette/Settings, Smart Merge verify/locks, Tags/Blame/Reflog, Unity YAML structural diff + R4-A table, Proposal Inbox + `ProposalView`, Stacks + Restack, AI draft, Linked worktrees).
   - Verify Windows WebView2 accelerator keys (`AreBrowserAcceleratorKeysEnabled = false` in native Tauri WebView configuration if `Ctrl+P` / `Ctrl+F` / `Ctrl+R` still trigger browser defaults on Windows — `docs/spec.md` §2).
2. **Real-world Unity + LFS testing & performance benchmarks**
   - Test structural diff and R4-A property table merge against real complex Unity scenes/prefabs (nested prefab instances, prefab variants, Stripped objects, YAML flow collections) and confirm `UnityYAMLMerge.exe` integration.
   - Smoke-test TGA and flat PSD decoding performance (target: 2048px texture `<= 300 ms`, PSD up to 100 MB `<= 1.5 s` in `docs/spec.md` §6).
   - Verify GitHub LFS lock/unlock/unlock-on-push and Bitbucket Cloud lock-surface hiding (`O1`) against live remotes.
   - Measure Sourcetree comparison baseline (`O9`) and 150-repo cold/warm Library load (`S2` / `§6`) using Settings' Local latency panel.
3. **UI/UX design polish (`O3` in `docs/spec.md`)**
   - Screens marked "no Sketch" in `docs/spec.md` §4 (Settings modal sections, `ProposalView`, `TagDialog`, `CloneDialog`, `ScanFolderDialog`, `CommandPalette`, `ErrorBanner`, Compact density, ultrawide 2-column inspector) were built directly from `app.css` tokens and need Max's visual review/polish in live rendering.
4. **Release & installer packaging**
   - Test `pnpm tauri build` Windows NSIS/MSI installer end-to-end.
   - Bundle or expose `gitgud-cli` (`crates/gitgud-cli`) alongside the desktop installer so `gitgud-cli mcp` works cleanly on `PATH` for Claude Code / Codex / Cursor.
   - Bundle fallback Git + `git-lfs` (`Q17`, `O8`) or document minimum version enforcement cleanly if distributing beyond Max's machine.
5. **Remaining open / follow-up engineering items**
   - History edits (`R2-A`) over merge commits or the root commit are intentionally refused for now (`CLAUDE.md`).
   - Align snapshot pruning in `snapshot.rs` with op-log 30d/2000-entry retention + `git gc` cleanup (`CLAUDE.md` notes snapshot pruning isn't wired to retention yet).
   - Open agent ergonomics questions in `docs/spec.md` §8: `O2` (line-shift hunk ID stability between `status` and `propose`), `O6` (MCP elicitation vs polling `proposal`), `O7` (hook-driven auto-proposals).
   - Verify Narrator / screen-reader accessibility (`O4`) and 4K DPR-2 canvas crispness (`O5`).
   - Deferred **later** items (`docs/spec.md` §3.6–§3.8, out of v1 scope unless requested): rendered prefab thumbnails (`Q6`), sparse checkout / `lfs.fetchinclude` UI (`Q7`), Unity Editor package (`Q5`), Artist mode (`Q3`), multi-window support.

## Things to do next (recommended first session order)

1. Run `pnpm e2e` against the debug build and fix any selectors/regressions in existing E2E specs.
2. Start `pnpm tauri dev`, open a real Unity + LFS repo plus a multi-repo Library folder, and do a structured walkthrough of M1–M9 UI + Settings' **Local latency** timings.
3. Address any layout polish, WebView2 accelerator (`Ctrl+P`/`Ctrl+F`), or Unity YAML parser edge cases uncovered on real `.unity`/`.prefab` assets.
4. Wire snapshot pruning into `OpLog::prune_now`, add M1–M9 `e2e/` coverage, and verify `pnpm tauri build` packaging with `gitgud-cli`.
