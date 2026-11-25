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
design round. Landed in M1–M6: command palette (`Ctrl+K`), shortcut sheet (`?`), line staging, syntax+intraline diff, plain-English error banner, settings modal, Unity Smart Merge / `.meta` guard / LFS locks / TGA+PSD previews, tags, file history & blame, op-log pruning + outside reflog recovery, commit-graph optimization, Unity YAML structural prefab/scene diff, R4-A property table resolver, post-merge integrity check, Unity repo setup, agent proposals (CLI / MCP / review inbox / ghost graph rows), stacked branches with `--update-refs` restack & per-level PR links, quick-edit force-push offer, LFS cache prune, AI commit message drafting, coding agent setup in Settings, faded "still on origin" commits after rewrites, first-open commit-graph prompt on large repos, manual index refresh, local latency diagnostics, linked agent worktrees -> proposals, R5 UI polish & `docs/lab/polish.html` preview, WebView2 browser accelerator disablement, custom level-dots app icon, bundled MinGit (`2.56.1`) + `git-lfs` (`3.8.1`) fallback (`git.rs` / `cli.rs`), Windows NSIS + portable packaging (`pnpm package` / `pnpm package:portable` / `scripts/package-portable.ps1` / `src-tauri/tauri.bundle.conf.json` / `src-tauri/windows/hooks.nsh` registering `$INSTDIR` + `gitgud-cli`), snapshot pruning in `OpLog::prune_now` / `Snapshots::sync_kept`, `O2`/`O6`/`O7` agent ergonomics (`stable_hunk_id_at`, `proposal_wait` / `--wait`, `agent hook` / `agent setup --with-hooks`), fast PNG checksums + TGA/PSD/150-repo/complex-Unity stress tests, M1–M9 E2E specs isolated via `GITGUD_LIBRARY`, `Graph.svelte` virtualized ARIA + reactive DPR/`ResizeObserver`, LFS-only op-log storage & reclaimable prune preview, Settings opt-in Claude Stop hook toggle (`agent_setup` `withHooks`), strict hunk-ID + release-gated bundled-Git guards, draggable/persisted panel splitters (`Splitter` / `prefs.panes`), `OpLogPanel` `Close` + Sidebar `Working copy` toggle, `Ctrl`/`Shift`-click multi-select staging (`WorkingCopy` / `StatusList`), expanded texture viewer (DDS/EXR/HDR/TIFF/16-bit PSD decode, channels/zoom/pixelated/diff heatmap, `<= 25 MiB` LFS auto-fetch + `lfs_fetch_object`), LFS pointer `BinaryMeta` cards (`LfsBinaryCard`), and embedded asset+LFS conflict inspector (`SidePick`), 3D model preview (R6: Studio look with a Textured / Changes tint toggle, side by side / swipe / onion, synced animation; FBX / OBJ / glTF / GLB, prefabs with nested prefabs and variants, `.asset` meshes, Animator / `.anim` clips; `ModelViewer` in working copy, commits and the conflict side picker, prefabs open in 3D first with a 3D | Structure switch), Settings › Caches (Unity references, 3D models, commit graph, LFS objects: where, size, Clear / Rebuild / Prune), R7 file previews so far: viewer routing with Visual | Structure | Source the font viewer (specimen, glyph grid, metrics), the `.mat` viewer (ball + changed properties, "Your mesh"), audio (stacked lanes + A/B, loudness), data tables (CSV / JSON / ScriptableObject grid with cell diff), PDF (pages + changed-page rail), video (synced players, frame step, frame-difference scan), sprite sheets (slices over the texture) and sprite atlases (packables table), animation curves (curve editor + 3D), Unity scenes in 3D (Merged mode, Changed only, filterable change list). Not yet: history edits over merge commits or the root commit (refused for now, Max 2026-11-22).
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
- **3D models** (`model/`, `$lib/model/*`, `ModelViewer.svelte`): the protocol's `/model` serves a model
  file at a version (LFS resolved; a Unity `Mesh` `.asset` converted to GLB in `mesh_asset.rs`) and
  `/ref?path=<model>&ref=<name>` the texture or buffer it names (`Files::find_near`). `model_info`
  returns Unity materials (`.mat` → base map / normal / colour / metallic / smoothness, custom shaders
  flagged `approximated`, their base map picked by name — colour-like first, normals / masks / noise /
  spec skipped — with `baseCandidates` for the per-shader "base map" choice in the 3D viewer's ⚠ panel,
  shared with the material viewer's), an FBX's `.meta` `externalObjects` remaps and scale, and for a prefab a
  `ModelScene` (`prefab.rs`: GameObjects, nested prefabs and variants with `m_Modifications`, stripped
  objects via Unity's `(instance ^ id) & i64::MAX` ids, built-in primitives, missing references,
  Animator clips from `anim.rs`), mirrored into three.js's right-handed space. GUIDs resolve through
  the working copy's index, falling back to the version's `.meta` files (`guids.rs`). Unity YAML is
  read per document with `yaml-rust2` (`model/yaml.rs`), not the diff parser. FBX goes through
  `$lib/model/vendor/FBXLoader.js`, three r170's loader with "gitgud:" changes (fixes for an empty
  normals layer and a geometry no model uses, still missing upstream in r187; a `geometryOnly` mode
  without images or textures; no `window` for camera aspect): re-copy and re-apply them when three is
  upgraded. Prefabs and scenes load FBX and Unity meshes `geometryOnly`: fetched and parsed on a pool
  of six workers (`fbxPool.ts` queues jobs in the order asked, at most eight per worker;
  `fbx.worker.ts`, `readGeometry.ts`) and handed back as transferable buffers (`packFbx.ts`, which
  keeps each mesh's material names so a whole model file placed in a scene still gets its import's
  remapped materials).
  `buildPrefab` shows the hierarchy at once with a dot per mesh and `load()` streams the meshes in,
  nearest the default camera first (`view.ts`); the viewer shows a scene before its meshes and
  `stage.refresh()` compares, fits floor / light and draws unchanged static meshes instanced
  (`instancing.ts`) once both sides are in. Shadow maps redraw only when models change or move. The frontend loads three.js lazily; `stage.ts` draws both versions in one WebGL context with a shared camera and clock;
  `compare.ts` matches objects by name path for the Changes tint and legend. FBX pivots are baked into
  prefab meshes as Unity's import does (`pivotOffset`).
- **Viewers** (R7; `$lib/viewers/route.ts`, `FilePreview.svelte`, `ViewSwitch.svelte`, `preview.rs`):
  `viewerFor(path)` picks a file's viewer by extension; a `.meta` opens its asset's viewer with an
  "import settings changed" note; `.asset` is decided by its first object's class
  (`unity_asset_info`: 43 Mesh → 3D, 114 ScriptableObject → grid). `viewModes` / `viewModeFor` give Visual | Structure | Source,
  remembered per format in `prefs.views` (R6's prefab "Text" carries over as Structure). The
  protocol's `/file` serves any file at a version, LFS resolved (25 MiB auto-fetch). Binary Unity
  assets get a card (`PreviewCard.svelte`); "Open in default app" (`open_version`) writes a
  read-only copy under `%TEMP%/gitgud-open/<pid>/`, deleted on exit (stale folders after a day).
  Choices inside a viewer are remembered per format with `viewerChoice` / `prefs.viewer`.
  Fonts (`font.rs`, `FontViewer.svelte`, `$lib/viewers/font.ts`): `font_info` reads TTF / OTF / TTC /
  WOFF / WOFF2 with `ttf-parser` (+ `woff2-patched`, WOFF1 unwrapped by hand) into names, metrics,
  axes and a hash per glyph outline; the webview draws both versions as `FontFace`s from `/file`,
  Sample / All glyphs / Metrics in the pill, added / removed / redrawn glyphs tinted.
  Materials (`MaterialViewer.svelte`, `$lib/viewers/material.ts`, `$lib/model/primitive.ts`):
  `material_view` lists every `.mat` property (Inspector labels for built-in / URP / HDRP names,
  properties a custom shader's source no longer declares marked stale), keywords and render queue;
  the 3D stage is `ModelViewer` with an `opener` building a sphere / cube / plane and a `chrome`
  snippet for the shape pill; "Your mesh" opens a prefab or model from `material_users`
  (`git grep` for the material's GUID at that version, prefabs first).
  Audio (`audio.rs`, `AudioViewer.svelte`, `$lib/viewers/audio.ts`): `audio_info` decodes with
  `symphonia` (true rate, channels, length, min/max peaks per channel) and measures sample peak and
  integrated loudness with `ebur128`; the webview plays blob URLs of `/file` (AIFF through the
  protocol's `/audio`, decoded to WAV), stacked lanes on one time axis, A/B swaps the version heard
  at the same position, never autoplays, one app-wide volume (`prefs.volume`).
  Data tables (`table.rs`, `model/scriptable.rs`, `DataViewer.svelte`): `data_tables(before?, after)`
  reads CSV / TSV (`csv`, delimiter sniffed, UTF-16 / BOM tolerated), JSON (a list → one table, an
  object → a "fields" table plus one per list of records, nested fields flattened to `a.b[2].c`, key
  order kept via serde_json `preserve_order`) and ScriptableObject `.asset`s (class 114; lists of
  records → tables, references resolved to file names); rows are matched by `id` / `key` / `name` /
  first column, by position when that key repeats; cells carry `[old, new]`.
  PDF (`PdfViewer.svelte`, `$lib/viewers/pdf.ts`, `$lib/viewers/pdfdoc.ts`): pdf.js (`pdfjs-dist`,
  lazy, worker via `?url`) reads every page's text with progress; `matchPages` lines pages up by
  text (LCS, so an inserted page shifts nothing; textless scans pair by position), the rail rings
  changed / added / removed pages, text runs the other page lacks are highlighted; Side by side /
  Swipe / Onion per page.
  Video (`video.rs`, `VideoViewer.svelte`, `$lib/viewers/video.ts`): `video_info` reads MP4 / MOV
  boxes (codec, size, fps from `stts`, bitrate; WebM relies on the player); both versions play from
  blob URLs in lockstep (the earlier one re-synced when it drifts > 80 ms), muted at first, `,` / `.`
  step a frame, Side by side / Swipe / Onion; "Find where frames differ" samples ≤ 240 frames of
  both on hidden players and marks the scrub bar; a codec that won't play gets a card with
  "Open in default app".
  Sprite sheets (`model/sprites.rs`, `SpriteViewer.svelte`, `$lib/viewers/sprites.ts`): an image's
  `.meta` opens `sprite_sheet` — the texture's Single / Multiple slicing from `TextureImporter`,
  sprites matched by name (added / removed / moved / resized / pivot / border) — drawn over the
  texture, Slices / Pixels / Pivots in the pill, 9-slice borders on hover (always when changed).
  The PNG's own diff stays the image diff. `.spriteatlas` goes through `data_tables` as "Sprite
  Atlas": packables as a one-column table, settings as fields, packed data skipped.
  Animation curves (`model/curves.rs`, `CurveViewer.svelte`, `$lib/viewers/curves.ts`):
  `clip_curves` reads a `.anim`'s editor / float curves with slopes (infinite → step), splits
  vector curves per axis, shows quaternion rotations as unwrapped Euler keys joined by lines, and
  sprite swaps (`m_PPtrCurves`) as flipbook rows; tracks are matched by object path + property.
  The plot evaluates Unity's Hermite curves (HEAD dashed, working solid, keys as dots; changed tracks
  plotted first). "3D" plays the clip on a prefab from `clip_users` (controllers naming the clip,
  then prefabs naming those; `ModelViewer` `startClip`).
  Scenes (`.unity`): assembled by the prefab builder (`model_kind` → Prefab) and shown in
  `ModelViewer` with `merged` (the stage's "merged" mode: the working copy, plus the older version's
  changed / removed meshes as amber wire ghosts); the change list gets a filter past 8 entries and
  "Changed only" fades the unchanged clay. `unity_asset_info.objects` counts GameObjects: scenes and
  prefabs above Settings › Previews' limit (`prefs.auto3dLimit`, default 2,000) open on a card
  ("Open in 3D" / "Show Structure"). Textures in 3D come through `previewTexture()`: the protocol's
  `/image` and `/ref` take `max=<px>` and return the image shrunk (`image::shrink_image`, fast PNG,
  cached by a hash of the file as git stores it — content or LFS pointer — so a hit decodes nothing) per Settings › Previews' "Texture size in 3D" (`prefs.textureMax`: Native /
  4096 / 2048 / 1024, default 1024); model and material cache keys include it. With `gpu=bc` (sent
  when the GPU has S3TC + sRGB, `gpuFormats.ts`) they come GPU-compressed as DDS (BC1 / BC3 with
  mips, flipped, `gpu_texture.rs` via `image_dds` encode), uploaded as is by
  `BitmapTextureLoader`; other images still decode with `createImageBitmap`. Only LOD0 of a LOD
  Group is drawn. Meshes stored inside a scene or prefab (`MeshRef::Inline`, with a content `key`
  so both sides and reopenings share them) are served as GLB by `fileID` through the protocol's
  `/model?…&mesh=`, or several per answer with `&meshes=a,b` (`Repo::inline_meshes`,
  `meshBundle.ts`; `inlineBatch.ts` asks 32 at a time). A scene's or prefab's `model_info`
  carries its model files' infos (`models`), so the viewer doesn't ask for each. While a big
  scene's "Open in 3D" card shows, `warm.ts` starts both sides' `model_info` (`sceneInfo.ts`,
  taken once by the opening) and then `warm_textures` for their materials' maps; a texture being
  prepared sits in the cache as an in-flight entry, so concurrent requests share one result.
  Gitignored assets (asset packs kept out of git): `Repo::file_at` reads them from disk in every
  version when a version lacks them (`Repo::ignored_on_disk`, cached per path and per ignored folder,
  cleared on refresh); `Files::has` lets GUIDs resolve to them, `Files::key` gives them the same disk
  key in every version (so both sides share one load), and `ModelInfo.from_disk` counts them for the
  viewer's note. `Files::new` takes its listing from `Repo::listings` (per commit tree; the index's
  until refresh), since a scene builds one per model.
- **Caches** (`caches.rs`, Settings › Caches): the GUID index is updated from changed `.meta` files
  (size + mtime), saved to `.git/gitgud/guid-index`, built in the background when a Unity repo opens,
  and marked stale (not dropped) on refresh; `.meta` GUIDs are kept per blob and per commit tree;
  `model_info` results per (path, commit tree), the index's / work tree's until the next refresh
  (`model/cache.rs`, 2,048 entries); a prefab build assembles each nested prefab once and parses
  only the YAML classes it reads (`yaml::docs_reading`, in parallel past 1 MB); each level's nested
  prefabs are read and parsed together ahead of the walk, model `.meta`s once per build, and `.mat`
  files after the walk, read together (`Files::prefetch`, rayon). On disk (`disk_cache.rs`,
  `.git/gitgud/cache/`, keys include the app version, LRU-pruned to 1 GB in the background on
  open): scene / prefab `model_info` at commit trees, and shrunk textures by content
  (`Repo::shrink_texture`); Settings › Caches › 3D previews shows and clears it. Reopening a repo
  that is already open (a reloaded window) shares its `Repo` and caches (`Session::reuse_or_open`)
  unless its config changed. `Repo::inline_mesh` indexes each scene version's Mesh documents
  once (`InlineSource`) and keeps both sides of a compare. Every `model_info` carries a
  content `key` (blob id, or a work-tree file's size + time), and `$lib/model/cache.ts` / `shared.ts`
  share loaded model files and Unity materials by it across viewers (both compare sides, reopenings),
  marked `userData.shared` so `ModelStage.release` leaves them; 768 MB budget, LRU. `cache_stats` /
  `clear_unity_caches` feed the Caches section with the commit graph and LFS objects. Dev builds
  optimize dependencies (`[profile.dev.package."*"]`).
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
