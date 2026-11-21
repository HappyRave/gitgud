# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

gitgud is a desktop Git GUI. The repo was wiped on 2026-11-21 for a fresh start; the earlier
prototype is still in git history (`5f1257d` and before). Scaffolded 2026-11-21: the shell runs and
locates the system git; no repo features yet. Update this file as pieces land.

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
  use the real `git` in temp dirs (throwaway repos, or fake `git` scripts on a custom search path),
  never a mocked git.
- **Commands** (`src-tauri`): unit tests that the serialized result matches the shape `api.ts`
  declares.
- **Frontend** (vitest + jsdom + Testing Library): `$lib/test/ipc.ts`'s `mockCommands` answers
  commands from typed handlers. An unmocked command fails the test. Add every new command to
  `Commands` in `api.ts`.
- **Real app** (`e2e/`): `launchApp()` starts `target/debug/gitgud.exe` with a throwaway WebView2
  profile, and Playwright drives it. Screenshots go to `e2e/out/` (ignored). Windows only.
