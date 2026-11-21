# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

gitgud is a desktop Git GUI. The repo was wiped on 2026-11-21 for a fresh start; the earlier
prototype is still in git history (`5f1257d` and before). **Everything below is the plan — not
scaffolded yet.** Update this file as pieces land.

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
```

Core tests build throwaway repos in temp dirs with the real `git` binary rather than mocking git.
