# Design decisions

Verdicts from the lab rounds (`docs/lab/index.html`), newest first. Each entry: date, Max's words in
quotes, what came with it, and what isn't applied yet. Feeds the decision log in `docs/spec.md`.

## Open in Explorer, terminal and editor (2026-11-26)

No lab round: menu entries, decided in chat. Brief (Max): "Open in Explorer & terminal next", from
the gap list against other clients.

- **Decided (2026-11-26):** the terminal is a setting, default Windows Terminal (its default
  profile), PowerShell when `wt` isn't installed (Max: "Setting, default WT"). "Open in editor" is in
  too, a setting with VS Code as default (Max: "Yes, setting, VS Code default"). Entries go on repos
  (Library row, repo tab, Ctrl+K) and on a new right-click menu for file rows (working copy, commit,
  compare) (Max: "Repo (Library, tab, Ctrl+K), File rows (new right-click)"). Not on sidebar
  branches.
- **Claude's picks while building (2026-11-27):** Git Bash and Command Prompt are also in the
  terminal list; editors are VS Code, Rider, Visual Studio or any program (given the file, or the repo
  folder). VS Code opens the repo folder at the file (`-g`); Visual Studio opens the file alone
  (`/Edit`). "Copy path" joined the file menus. The repo tab got a right-click menu (it had none)
  with "Close tab". A deleted file's editor entry is disabled.
- **Built (2026-11-27):** all of it; not yet tried by hand in the real app (the tests never start a
  window).

## R12 · Compare any two (2026-11-26)

Brief (Max, 2026-11-26): "Compare any two", picked from the gap list against Fork, Sourcetree,
GitKraken and Tower. Round: `docs/lab/compare.html`, built on pairs measured in the VEX repos
(35 to 73,162 files; escape1's Dev_Sam: 23 files as snapshots, 0 as "what it brings").

- **Decided (2026-11-26):** **B, a compare view of its own.** Max: "B Compare view". Two pickers
  (branch, tag, commit, working copy, stash), the mode, Swap; commits only on each side, files by
  folder with a kind summary, the existing viewers. A's Ctrl+click and "Compare with current" open it
  (Claude's pick, came with B).
- **Rejected for now:** A as the whole feature (kept as a way in), C (pin a base), D (branch card).
- **Decided (2026-11-26, Max: "Go" on Claude's picks):** two branches open in "What B brings"
  (question 2); "Take B's version of this file", undoable, is in, cherry-pick from the view later
  (question 3); the merge prediction shows when B is a branch (question 4).
- **Claude's picks while building (2026-11-26):** a tag's "Compare with current" puts the tag on the A
  side (so B shows what changed since that build); a branch's puts the current branch on A. A commit
  row offers "Compare with <current>" (the commit as A) and "Compare with working copy". Ctrl+K opens
  the current branch against its upstream (else the working copy). The picker finds branches, tags,
  stashes and the working copy by name, and takes a typed hash; searching commits by subject isn't in.
- **Built (2026-11-26):** core compare + file / Unity diffs + take version, the view, the pickers, the
  four ways in.
- **Not built:** commit search by subject in the picker; clicking a listed commit; cherry-pick from the
  view (question 3: later); an empty state for 0 files in snapshot mode beyond "0 files".

## R11 · Overlap radar (2026-11-26)

Brief (Max, 2026-11-26): "1 Overlap radar", picked from an analysis of the VEX repos (14 people on
one shared branch; 71 of 477 merges in 90 days joined two sides that changed the same file, 59 of
those 134 files in LFS). Round: `docs/lab/overlap.html`.

- **Decided (2026-11-26):** **B, heads-up when it starts.** Max: "B Heads-up". A's marks on working
  copy rows, the strip and the commit box, plus one interruption (a toast in gitgud, a Windows
  notification when it's in the background) when a file you're changing and origin's changes meet,
  only for files git can't merge as text.
- **Rejected for now:** C (team board) and D (soft claims, useless until the team runs gitgud). Not
  ruled out as later additions.
- **Decided (2026-11-26):** the active tab fetches every **2 minutes** (question 3, Max: "q3: 2
  minutes"); Windows notifications are **a setting, on by default** (question 5, Max: "it should be
  a setting, default ON"). Questions 2 (compare with the main branch too) and 4 (interrupt only for
  won't-merge files) taken as Claude's picks (Max: "GO").
- **Claude's picks while building (2026-11-26):** when a won't-merge file has uncommitted edits, the
  heads-up offers "Commit, then pull" instead of "Pull now": git refuses to pull over an uncommitted
  file the incoming change touches, and the conflict view needs a real merge (a stash pop's conflicts
  don't open it). The button focuses the commit box; the next commit runs the pull. The toast sits over
  the bottom of the history, clear of the commit box. Only the active tab fetches every 2 min; other
  tabs don't fetch in the background (as before), but their radar re-reads on every outside change.
- **Built (2026-11-26):** core `overlap` + command, row marks, strip count, commit-box line, heads-up
  toast, Windows notification with its Settings switch, 2-min fetch for the active tab.
- **Not built:** the mark on your unpushed commit's graph row (state 4 on the page); "Unity is open,
  save the scene first" hint in the toast; told heads-ups are forgotten on restart (one repeat per
  overlap after a relaunch).

## R10 · Partial LFS download (2026-11-26)

Brief (Max, 2026-11-26): "Let's start on partial lfs" (spec Q7, brought forward from "later").
Round: `docs/lab/lfs.html`, built on sizes measured in the VEX repos.

- **Decided (2026-11-26):** **B, what your scenes use.** Max: "B". Downloads what the chosen scenes
  reference through GUIDs (default: Build Settings' scenes) plus what Unity always loads; A's folder
  tree stays inside B as an override tab (Claude's pick, came with B).
- **Max's question (2026-11-26):** "what when I need to get one of the ommited files later?" Answered
  in the round's "Getting a skipped file later" section.
- **Decided (2026-11-26):** keep what you download (question 6). Max: "Yes". Every download joins
  the list under "Kept by you" until removed.
- **Claude's picks, applied while Max was away (2026-11-26, Max: "Don't ask anything, just work"):**
  the clone dialog's "LFS: download only what the build's scenes use" is on by default (question 2);
  applying turns no-longer-needed files back into pointers, only unmodified ones whose object stays in
  `.git/lfs`, refused while Unity has the project open (question 3); the dialog opens from Ctrl+K ›
  "LFS downloads" (question 4: not yet from the Library's right-click or the strip).
- **Built (2026-11-26):** the dialog (scenes, kept, always kept, skipped folders with Download,
  Download everything), the plan / apply / download backend, the clone option.
- **Not built / open:** reading Unity's `Editor.log` for skipped files Unity asked for; the Library's
  LFS column and the pull summary; ☁ marks on file rows. The Unity pointer-file test (question 5) has
  not been run: nobody has opened a partial project in Unity yet.

## R9 · Merge by property (2026-11-26)

Brief (Max, 2026-11-26): "if I push one field change in a scriptable object, and Miguel push another
change of another property in the same file, I'm gonna get a merge conflict because it's considered
binary data, instead of a clean merge because the 2 properties were independant" … "a big .unity file
is on LFS, but it's basically a yaml with properties we can edit?" … "the merge shouldn't be
automatic. You get a conflict as "normal", and an option to merge inteligently (with clear feedback
of the changes and merged result)". Round: `docs/lab/merge.html`.

- **Decided (2026-11-26):** M2 and every Claude's pick. Max: "go with your picks". That is:
  1. **M2:** R4-A's property table extended to every change: clashes on top (unpicked), one-sided
     changes below, filters All / Clashes / Yours / Theirs / Objects added-deleted, 1 / 2 / E per
     row, merged result in 3D on the right with the Changes tint by source.
  2. **One-sided changes come in applied**, each can be flipped; only clashes block saving.
  3. **Both sides added children under one parent: ask** for the order (a clash row).
  4. **Counts are computed when the conflict opens**, in the background (the E offer).
  5. **Also offered for Unity YAML not in LFS** (when Smart Merge isn't set up or gives up).
- **Came with it, not discussed separately:** never automatic, the file stops as a normal conflict
  and the offer sits in the pick-a-side view; objects matched by fileID, reference lists by ID, value
  lists by position only when one side changed them; deleted-vs-edited is an object-level clash; save
  writes the YAML and stages it (LFS clean filter), then the post-merge check; every pick and the save
  undoable; same view in rebase and cherry-pick stops; a missing LFS side offers Fetch; a
  binary-serialized side gets a reason instead of the offer.
- **Built (2026-11-26):** all of it except the 3D result pane.
  The R4-A table's "Pick all" buttons stay, for clashes only.
- **Decided (2026-11-26):** the 3D result pane is **A, on demand**. Max: "Go A". Built from the
  current picks when the pane opens or on Rebuild (R); later picks mark it out of date ("Out of date ·
  N picks since"), never rebuild by themselves. Not chosen: B Live, C No result pane.
- **Built (2026-11-26):** A, 3D only (Max: "Go, 3D only"; the mock's Structure | 3D switch left out).
  "Result in 3D" in the table's header opens the pane, for `.unity` and `.prefab` files. The result
  shows Changes-shaded by source; Textured is one click away. Claude's call: objects are matched to
  rows by their object path, so a repeated sibling name tints only the first.

## R8 · App stats in the strip (2026-11-26)

Brief (Max, 2026-11-25): "I'm not sure the little git version and path in the bottom right corner is
useful. What would be more useful is some app stats, like cpu, ram, and cache usage (in each repo)
don't make it boring, like not just CPU: 2%. but must remain 100% clear and readable. The cache stat
can be clicked to show a little context window with a clear button". Round: `docs/lab/stats.html`.

- **Decided (2026-11-26):** S1 Trace and C1 cache popover, with every Claude's pick. Max: "S1 Trace
  + my picks". That is:
  1. **S1 Trace:** CPU as a 40-second trace (amber above 50%, orange above 85%) then the exact %;
     RAM and cache as plain numbers with short labels; the cache stat is a button with a drum icon.
  2. **CPU scale:** a share of the whole machine, like Task Manager.
  3. **RAM:** the whole process tree, WebView2 included; hover splits app / view / git.
  4. **Library tab:** the cache stat shows every repo's caches together; the popover lists the
     biggest repos, each with its own Clear, plus Clear all.
  5. **Clear** also drops the viewers' in-memory model cache.
- **Came with it, not discussed separately:** the git version and path leave the strip (Settings ›
  Git has them); sampled every 2 s, paused while the window is hidden; Clear never touches LFS
  objects or git's commit-graph, and has no confirm since everything it removes rebuilds.
- **Built (2026-11-26):** as decided. The popover's "Settings › Caches" link opens Settings (at the
  top) and reads "More in Settings"; the Library tab's popover has no such link.

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
- **Decided (2026-11-24):** in the 3D Changes shading, everything under a modified object (a moved
  GameObject's mesh child) is tinted amber too; before, only the object itself counted and its mesh
  stayed grey. Max: "Yes, tint it".

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
