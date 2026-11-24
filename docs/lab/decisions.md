# Design decisions

Verdicts from the lab rounds (`docs/lab/index.html`), newest first. Each entry: date, Max's words in
quotes, what came with it, and what isn't applied yet. Feeds the decision log in `docs/spec.md`.

## R7 · Previews for more file types (2026-11-24)

Brief (Max, 2026-11-24): "Would be awesome if we can preview/compare: images pdfs sounds textures
mats and many more game dev or dev file formats". Images and textures already exist (image diff,
texture viewer); `.svg` already opens in the image diff.

- **Decided:** one batch with every format offered. Max picked "Unity .mat, Audio, PDF, Fonts + SVG
  + video" and "Sprite atlases/slices, Animation curves, Data tables, Unity scenes 3D".
- An adversarial review ran before the round was shown; its fixes are in the page (SVG marked as
  existing, `.spriteatlas` split from sprite sheets, no spinning ball or drifting scene camera per
  R6's "Never", `.mat` / `.anim` / ScriptableObject storage costs, AIFF and codec limits, counts
  corrected, new states: `.meta`-only change, binary Unity asset, conflict side picker).
- **Decided:** every Claude's pick on `docs/lab/viewers.html`. Max: "Go with your picks". That is:
  1. **Shared frame as a rule:** the header holds only how the two versions are arranged (Side by
     side / Swipe / Onion; Stacked / Overlay for sound); every other view switch lives in the pill
     over the view; Expand on the right. Frames that broke this get redrawn when built.
  2. **Defaults:** M1 Ball + changes, A1 Stacked lanes + A/B, P1 Pages + changed-page rail, T1
     Specimen + glyph grid, S1 as today (plus an attribute caption and Source), V1 Synced players,
     SP1 Slices over the texture, AC1 Curve editor, D1 Grid with cell diff, U1 R6 Studio +
     hierarchy. The other variants come later as modes, one at a time (A2 → Overlay, P2 → Text,
     T2 → Onion, AC2 → Dope sheet, D2 → nested cells, V2 → scrub preview, U2 → Map).
  3. **Unity text files** (.mat, .anim, .asset, .unity): R6's "Structure | 3D" grows to
     **Visual | Structure | Source**, opening in Visual, remembered per format.
  4. **Audio and video:** never autoplay, video starts muted, one app-wide volume.
  5. **Remembered choices** (A/B, Stacked/Overlay, PDF Text, Changed rows, Curves/Dope sheet…):
     per format.
  6. **New dependencies:** pdf.js (frontend); a Rust font parser (`ttf-parser` or `skrifa`, plus
     WOFF2 decompression); `symphonia` (Rust audio decoding: AIFF, true rate, loudness,
     spectrogram); a Rust MP4/MOV container parser for fps and codec. No ffmpeg / ffprobe.
  7. **A `.meta`-only change** opens the asset's viewer with an "import settings changed" note.
  8. **"Your mesh" (.mat) and "3D" (.anim):** a reverse lookup offers a list of users, first one
     preselected.
  9. **TextMeshPro font assets:** later; the .otf / .ttf viewer first.
  10. **Scenes:** a merged Changes view as an extra mode next to side by side; a scene opens in
      Structure first above 20k objects (a guess, to measure).
  11. **Auto-download:** the 25 MiB LFS rule holds for audio and video; "Open in default app"
      writes the old version to the OS temp folder, deleted when gitgud closes.
  12. **Build order:** fonts, .mat, audio, data tables, PDF, video, sprite sheets, curves, scenes.
      SVG already works.
- **Came with the picks, not discussed separately:** `.spriteatlas` changes show as a packables
  table (the packed page lives in `Library/`); a `.asset` picks its viewer by its first object
  (ScriptableObject → grid, Mesh → 3D, TMP font → later, else Structure); binary Unity assets get a
  "binary format" card pointing to the Force Text check; all viewers work in the conflict side
  picker with sides named Yours / Theirs (Parent / Commit in a commit).
- **Not applied yet:** nothing is built. The frames on the page that break rule 1 (A1, AC1, D1, P1,
  M1 / SP1 pill position, U1) aren't redrawn.

## R6 · 3D preview (2026-11-23)

Brief (Max, 2026-11-23): "a 3D preview of prefabs, fbx, meshes, with texture et al if applicable".
Before the round: three.js renders it ("A. Three J's"); animations are in scope ("B. Yes, and
animations"); three compare modes, side by side, onion skin and swipe ("C. Yes, plus de slider (so
side, onionskin and slider)").

- **Decided:** **R6-A Studio**, plus D's change tint as a toggle. Max: "Studio! But with the added
  option to toggle the textures in the side by side with the change tint". The shading toggle is
  Textured / Changes (clay tinted green added, red removed, amber modified).
- **Decided:** the shading an asset opens in **remembers the last choice**. Max picked "Remember last
  choice" (Claude's pick was Changes for changed files).
- **Decided:** in Changes mode **D's legend shows** (each change with why, click flies to it); hidden
  in Textured. Max picked "Yes, in Changes mode".
- **Decided:** animation on open **remembers the last choice** (playing or paused). Max picked
  "Remember last choice" (Claude's pick was paused). Both versions always run the same clip at the
  same time.
- **Decided:** the backdrop **follows the app theme**, not A's fixed studio grey. Max picked "Follow
  app theme".
- **Decided:** **no turntable**. Max picked "Never".
- **Decided:** camera: left-drag orbit, right-drag pan, wheel zoom, F frames, double-click frames what
  you hit. Max picked "Simple + F".
- **Decided:** **inline where the image diff is, plus Expand** to fill the window. Max picked "Inline +
  Expand".
- **Not chosen:** R6-B Inspector panel, R6-C Light table, R6-D Change tint as the whole look (its tint
  and legend live on inside A). No reason given.
- **Came with A, not discussed separately:** controls in a floating pill top-right (shading, Wire);
  video-player transport along the bottom (play, clip picker, scrub, time); stats chip above it
  (tris, verts, meshes, materials, textures, with deltas); real key-light shadow on an invisible
  floor; faint grid; the four states drawn on D (new file, missing references in magenta, LFS not
  downloaded, approximated shader note) carry over to A.
- **Applied 2026-11-23:** built as `ModelViewer` (see CLAUDE.md "3D models"); the decided combination
  is drawn as "R6 · As decided" on `docs/lab/viewer3d.html`. Not built: viewing a `.asset` Mesh file on
  its own (it still opens as Unity YAML; prefabs that use one draw it), trimming FBX clips to a
  `clipAnimations` frame range (the whole take plays), and OBJ `.mtl` materials.
- **Decided 2026-11-23** (build plan):
  - A prefab change opens **in 3D first**, with a Structure | 3D switch in its diff header that then
    remembers the last choice. Max picked "Switch, 3D first".
  - GUIDs in old versions resolve through the **working copy's index, falling back to the `.meta`
    files at that commit**. Max picked "Current + fallback".
  - **All three phases ship together**: standalone models (FBX / OBJ / glTF / GLB), Unity prefabs
    (materials, nested prefabs, variants), and `.asset` meshes, Animator / `.anim` clips, the
    big-mesh warning. Max picked "All three phases".
  - A standalone FBX **honours its `.meta` material remapping** (`externalObjects`), falling back to
    embedded / relative textures. Max picked "Honour .meta remaps".

## R4 · Merge conflicts, Unity first (2026-11-21)

- **Decided:** **R4-A Property table**. Max: "A, as picked". A Unity YAML conflict opens as one row
  per changed property (object › component › property; base / yours / theirs / result; 1 / 2 / E to
  pick); fileIDs resolved to names; raw YAML one tab away.
- **Decided:** Smart Merge (UnityYAMLMerge driver) is **offered at the first merge** that hits Unity
  files, not set up on open. Max picked "Offer at first merge".
- **Decided:** what Smart Merge merged on its own is **listed, collapsed** under the real conflicts.
  Max picked "Listed, collapsed".
- **Decided:** "merged, verify" flags **block Commit merge until each is confirmed**. Max picked "Block
  until confirmed".
- **Decided:** code conflicts use **three columns**, yours | result | theirs, arrow per hunk. Max picked
  "Three columns".
- **Not chosen:** R4-B Annotated text (for Unity files), R4-C Two hierarchies, R4-D One at a time. No
  reason given.
- **Came with A, not discussed separately:**
  - the conflict list on the left, grouped by kind (Unity YAML, code, LFS binary), files staged as
    they're resolved, Ctrl+Z undoes a pick;
  - the merge banner keeps Commit merge disabled until every file is resolved, Abort always there;
  - after every automatic Smart Merge, gitgud compares both sides per property and raises "merged,
    verify" where both changed the same one (the spike's S5);
  - LFS binaries: pick a side using the image diff, with lock info;
  - setup writes only `.git/config` and `.git/info/attributes`, driver with `--force`; "Resolve as text
    this time" and "Other Unity version…" offered;
  - after resolving: a summary (picked / verified / auto-merged), a post-merge check (prefabs re-read,
    no missing references, no duplicate fileIDs), prefilled merge message, Ctrl+Z undoes the merge.
- **Not applied yet:** the YAML tab and "Open in Unity" aren't drawn in detail; no spec.

## R3 · Many projects (2026-11-21)

Brief (Max, 2026-11-21): "a lot of them", "ideally folders or categories"; 150+ repos; several open at
once as tabs.

- **Decided:** **R3-A Library tab + folders**. Max: "A, as picked". The first tab is always the Library:
  every repo with branch, changes, ahead/behind, locks held, last activity, host; folders on the left,
  smart filters below them; type to filter, Enter opens in a new tab.
- **Decided:** **one folder per repo**, no tags. Max picked "One folder per repo".
- **Decided:** folders **nest freely**, drag to rearrange. Max picked "Nest freely".
- **Decided:** Library status is **local only, no background fetch**; "behind" shows the last known
  value with its age, plus a manual "Fetch all visible". Max picked "Local only, no auto-fetch".
- **Decided:** **reopen last session's tabs** on launch, Library first. Max picked "Reopen last
  session's tabs".
- **Not chosen:** R3-B Project rail, R3-C Switcher + tags, R3-D Smart groups only. No reason given.
- **Came with A, not discussed separately:**
  - open repos are tabs (Ctrl+Tab, Ctrl+W), dot for uncommitted work, lock icon for locks held,
    overflow menu when they don't fit;
  - Ctrl+P "go to repo" from anywhere; an already-open repo jumps to its tab;
  - status cached and refreshed only for visible rows (no 150× `git status` at startup);
  - smart filters: Uncommitted, Not pushed, Locks I hold, Unity projects, Missing on disk;
  - inside a folder, each subfolder lists its most recent repos first with "show all";
  - "Scan a folder" adds many repos at once and suggests folders from the disk layout; known repos skipped;
  - a repo missing on disk stays in place, greyed, with Locate… / Remove from gitgud.
- **Not applied yet:** the "Fetch all visible" button and "behind (2 h ago)" age aren't drawn; no spec.

## R2 · History edits (2026-11-21)

- **Decided:** **R2-A Plan beside preview**. Max: "A, as picked". "Edit history" turns the inspector
  into the rebase plan (action per commit with single-key P/R/E/S/F/D, drag or Alt+↑↓ to move,
  conflict prediction per step); the graph draws the result as dashed "after" rows over the faded
  current commits; nothing runs until Start.
- **Decided:** `fixup!` / `squash!` commits are **applied automatically** when the plan opens. Max
  picked "Apply automatically".
- **Decided:** plan order is **newest first**, matching the graph. Max picked "Newest first".
- **Decided:** after rewriting a pushed branch, the Done panel **offers "Force-push with lease" right
  away**. Max picked "Offer it right away".
- **Decided:** **single-commit edits apply at once** (reword the tip inline with R, F/D on one commit),
  undone with Ctrl+Z; anything touching 2+ commits or older history opens the plan. Max picked "Apply
  at once, undo with Ctrl+Z".
- **Not chosen:** R2-B Edit in place (as the whole model; its instant edits survive only for the
  single-commit case above), R2-C Plan dialog, R2-D Now | After. No reason given.
- **Came with A, not discussed separately:**
  - a neutral "Planning … nothing changes until Start" banner while the plan is open;
  - the plan footer states the result ("3 commits → 2", "final files identical to now", conflicts
    predicted) and warns when the branch is on origin;
  - a predicted conflict names the step, commit and file, and offers "Show the conflict" / "Keep the
    old base" before "Start anyway";
  - Squash opens the combined message inline in the plan; Fixup drops the folded commit's message;
  - "Update dependent branches" checkbox (`--update-refs`) in the plan;
  - after Done, the old remote commits stay drawn faded under `origin/<branch>`; no "update from
    remote" button.
- **Not applied yet:** the instant single-commit edits aren't drawn; the 3-way / Smart Merge resolver
  is round 3; no spec.
- **Decided 2026-11-22** (while building, screens the mocks don't draw; Max picked Claude's pick each time):
  - "Edit history" opens from a commit row: Shift+E or right-click → "Edit history from here"; the plan
    covers that commit up to HEAD, onto its parent; the Onto chip lists local branches. Max picked
    "Commit row: Shift+E + menu".
  - An Edit (E) stop keeps the graph and working copy; the R1 state 1 banner reads "Stopped at step n
    of m to edit …" with Continue (amends what is staged) / Abort. Max picked "Banner + normal layout".
  - "Show the conflict" shows the predicted file with git's conflict markers in the plan's diff area,
    yours / theirs tinted, read-only. Max picked "File in the plan's diff area".
  - History with merge commits in the range, and the root commit, are refused in v1 with a plain
    message. Max picked "Refuse for now".

## R1 · Main window layout (2026-11-21)

- **Decided:** layout **R1-A Graph centre**. Max: "A, as picked". Sidebar | graph | inspector; the
  top "uncommitted" row turns the inspector into staging, a commit row into that commit's diff.
- **Decided:** graph rows **24 px by default, with one app-wide Compact/Standard toggle (20 px in
  Compact)**. Max picked "24 px default + compact toggle".
- **Decided:** on an ultrawide (above ~2400 px) **the inspector splits in two**: file lists and commit
  box | diff. Sidebar and graph unchanged. Max picked "Inspector splits in two".
- **Not chosen:** R1-B Stage desk, R1-C One timeline, R1-D Tiles. No reason given; don't re-propose as
  whole layouts. Parts may come back only if Max asks.
- **Came with A, not discussed separately** (treat as decided-by-implication; raise if they cause
  trouble):
  - the op log is its own panel (sidebar entry, Ctrl+3), not woven into history (R1 Q2);
  - locks and agent proposals are permanent sidebar sections (R1 Q3);
  - the toolbar's Undo button names the last action; Ctrl+K palette field in the toolbar;
  - in-progress operations (rebase, merge) take a banner above all panes with Continue / Skip / Abort;
  - conflicts listed by kind (code, Unity YAML, LFS binary) with a resolver each;
  - the six states drawn on A (round 1 page) are the reference for those screens.
- **Not applied yet:** `docs/spec.md` doesn't exist; the ultrawide split and the Compact density aren't
  drawn on the lab page.
