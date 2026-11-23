# Design decisions

Verdicts from the lab rounds (`docs/lab/index.html`), newest first. Each entry: date, Max's words in
quotes, what came with it, and what isn't applied yet. Feeds the decision log in `docs/spec.md`.

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
- **Not applied yet:** nothing is built. The decided combination is drawn as "R6 · As decided" on
  `docs/lab/viewer3d.html`.

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
