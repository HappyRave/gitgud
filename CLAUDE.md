# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

gitgud is a desktop Git GUI. The repo was wiped on 2026-11-21 for a fresh start; the earlier
prototype is still in git history (`5f1257d` and before). Built so far: locating the system git;
opening a repo (folder picker, or `GITGUD_OPEN=<path>` at launch); the commit graph in the centre
(topo order, lanes laid out in Rust, canvas lane column, rows fetched on scroll, WIP row on top);
the working-copy status as a bare list beside it. No writes, no live refresh yet; the layout is not
R1-A yet and lane colours are placeholders. Update this file as pieces land.

The approved product spec is `docs/spec.md` (2026-11-21); design verdicts are in `docs/lab/decisions.md`.

## Stack

- Tauri 2 shell, Rust backend, SvelteKit frontend (Svelte 5, `adapter-static`), pnpm.
- Git access is split by kind of operation:
  - **Reads** (status, log, commit graph, diff, blame): the `gix` crate (gitoxide), in-process.
  - **Writes and network** (commit, fetch, push, rebase, merge, stash): spawn the `git` CLI, so
    hooks, credential helpers, SSH, signing and user config behave exactly as in a terminal.
    The system `git` is used when it is ≥ 2.45.2; a bundled Git is the fallback (decided 2026-11-21).
  - Not `git2`/libgit2: no hook support, C build, credential friction.

## Layout

```
Cargo.toml              workspace root
crates/gitgud-core/     all git logic, plain Rust, no Tauri dependency
src-tauri/              thin Tauri layer: #[tauri::command]s that call gitgud-core
src/                    SvelteKit app; lib/api.ts wraps invoke() for every command
e2e/                    real-app tests: Playwright attached to the debug build over WebView2 CDP
spikes/                 throwaway research code, not in the workspace
package.json
```

Logic belongs in `gitgud-core`; `src-tauri` only adapts it to commands, and the Svelte side only
calls commands and renders. Core stays testable without compiling the webview.

Open repos live in `session::Session` (Tauri managed state), by id: `open_repo` walks the history
once, and later commands (`repo_status`, `graph_rows`) take the id. The graph keeps topology plus
lane checkpoints every 256 rows (`graph::Layout`), so any window of rows is laid out by replaying
at most 255 rows (spike 4.3).

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
```

Every feature is tested at each layer it touches:

- **Core** (`crates/gitgud-core`): unit tests next to the code, integration tests in `tests/`. They
  use the real `git` in temp dirs, never a mocked git. `testing::TestRepo` (feature `test-util`)
  builds throwaway repos through the git CLI, with a fake clock so commit dates (and so orders) are
  deterministic. Gix-based reads are checked against the CLI as an oracle: `cli_status()` parses
  `git status --porcelain=v2`, `cli_topo()` parses `git log --topo-order`. Pure algorithms get
  `proptest` property tests (the graph layout: windows equal the full layout, every edge reaches its
  parent); keep `proptest-regressions/` committed.
- **Commands** (`src-tauri`): tests go through Tauri's real IPC path (`tauri::test`, the `Ipc`
  helper in `lib.rs`) with JSON args as the webview sends them, on `TestRepo`s. Commands are `async`
  so they stay off the main thread. `build.rs` hands the Windows manifest to the linker for every
  target; without it, test binaries die with `STATUS_ENTRYPOINT_NOT_FOUND`.
- **Frontend** (vitest + jsdom + Testing Library): `$lib/test/ipc.ts`'s `mockCommands` answers
  commands from typed handlers. An unmocked command fails the test. Add every new command to
  `Commands` in `api.ts`, plugin commands (`plugin:dialog|open`) included. Logic sits in plain TS
  modules with their own tests (`$lib/graph/*`); components stay thin. jsdom has no canvas or
  layout: the setup installs a recording 2D context (`$lib/test/canvas.ts`), and tests stub
  `clientHeight`.
- **Real app** (`e2e/`): `launchApp(env)` starts `target/debug/gitgud.exe` with a throwaway WebView2
  profile, and Playwright drives it. `e2e/repo.ts` mirrors `TestRepo`; `generate(n)` makes large
  histories with `git fast-import`. Pass `GITGUD_OPEN` to open a repo, since native dialogs can't
  be driven. Performance targets from spec §6 are asserted here (10k commits: open + first rows
  ≤ 300 ms). Files run one at a time (one CDP port). Screenshots go to `e2e/out/` (ignored).
  Windows only.
