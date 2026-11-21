# Hands-on GUI round

Run **2026-11-21** with computer use, on Windows 11 (3440×1440). Labels: `fact` (seen in this round),
`Claude's pick`, `open`. Screenshots with pins: `docs/research/shots/handson.json` (19 shots, `ho-*`).

Test repo: BoatAttack (Unity, ~185 LFS files), local bare `remote.git` as origin, reset with
a setup script before each tool (`handson_setup.py`, removed 2026-11-21 since the round won't be re-run). Same 10 tasks for every tool; mechanics only.

**Not run:**

- **Sourcetree 3.4.30 (baseline):** skipped. It is Max's work install; Max asked for it not to be
  touched. Its matrix column stays empty.
- **lazygit 0.66.3:** not driven. Computer use grants terminals at "click" tier (no keys), and lazygit
  is keyboard-only. `open`: run it by hand, or through a non-terminal host.

Legend: ✓ done smoothly · ~ done with friction or partly · ✗ not possible in the UI.

## 1. Results matrix

| Task | Fork 2.24.0 | Sublime Merge 2136 | GitButler 0.22.3 |
|---|---|---|---|
| 1. Open, graph usable | ✓ < 3 s | ✓ < 1.5 s | ~ ~4 s, no graph (lanes + branch list) |
| 2. Stage hunk A, commit | ~ hover pill unreachable when maximised on ultrawide | ✓ 1 click "Stage Hunk" | ✓ hunk/line checkboxes; lands on a virtual branch |
| 3. LFS texture diff | ✓ side-by-side / swipe / onion skin | ~ side-by-side / onion skin | ✗ pointer text only |
| 4. Rebase: fixup + reword | ✓ todo-list dialog, autosquash pre-applied | ~ per-commit edits; fixup line kept | ~ drag-to-squash; fixup line kept; "diverged" nag |
| 5. Merge with conflicts | ✓ predicted up front; 3-pane merge | ✓ inline markers; 3-way tool | ✗ no branch merge; apply refused |
| — external / UnityYAMLMerge | VS, VS Code only | none offered | n/a |
| 6. Stash and pop | ✓ stash node in graph; pop checkbox | ✓ Ctrl+S / Ctrl+Shift+S | ~ no stash; "Stash into branch" / unapply |
| 7. Cherry-pick | ✓ predicted "no conflicts" | ✓ flag picker | ✗ none |
| 8a. Discard, then recover | ✗ no undo | ✗ Undo is text-only | ✓ op-log Revert |
| 8b. Undo last commit | ✓ Reset dialog, soft/mixed/hard | ✓ Reset submenu, 4 modes | ✓ "Uncommit" |
| 9. LFS lock UI | ~ LFS Status (Locks) window, lock/unlock | ✗ "LFS Track File" only | ✗ and warns LFS filters are skipped |
| 10. Palette / keyboard | ~ Quick Launch, short command list | ✓ palette with git equivalents | ✗ none; Ctrl+Shift+P opens print dialog |

## 2. Fork 2.24.0

- **Setup:** first-run "User information" modal, then ~10 s "preparing for first run". Opened the
  repo via `Fork.exe <path>` (the folder picker blocks typing for computer use).
- **T1:** graph, folder-grouped branches and the stash as a graph node were all there in the first
  screenshot (`ho-fork-graph`).
- **T2:** the per-hunk "Stage | Discard…" pill sits at the far right of the hunk. Maximised on a
  3440 px screen its hover zone ends ~200 px before the pill, so it vanishes as the pointer reaches it;
  the line context menu has no stage. Worked only after un-maximising. Then Ctrl+Enter commits.
- **T3:** selecting the PNG opens an image diff straight away with LFS badges, size and dimensions;
  Side-by-Side / Swipe / Onion Skin (`ho-fork-imagediff`, `ho-fork-swipe`).
- **T4:** right-click base → Interactive Rebase → "…to here" (also quick Reword / Squash into parent /
  Fixup / Drop). The dialog had already marked `fixup!` as Fixup and moved it. Single-key actions
  (P/E/R/S/F/D), Ctrl+↑/↓ reorder, reword edited inline before running. "Update dependent branches"
  and "Backup current state with a temporary branch" checkboxes. ~6 clicks (`ho-fork-rebase`).
- **T5:** the merge dialog warned **"Merge will cause conflicts"** before running
  (`ho-fork-merge-predict`). After: raw git output modal, banner "Fix 2 conflicts" + Abort. Per-file
  view: tick theirs/ours, or "Merge" into a 3-pane editor (n/2 counter, "Select Left/Right" per chunk,
  both allowed, editable result). The prefab merged as plain YAML; external tools offered: Visual
  Studio, VS Code (`ho-fork-merge`).
- **T6:** toolbar Stash (message, "stage new files"); apply dialog has "Delete stash after applying"
  (= pop), kept on conflict.
- **T7:** right-click → Cherry-pick; dialog says "can be done without conflicts", options commit /
  append origin.
- **T8:** discard has a confirm modal and no undo; no reflog view in the menus. Reset dialog explains
  modes in plain words ("Keep all changes, unstage differences").
- **T9:** Repository → Git LFS → Status (Locks): a file list with lock / unlock buttons; the local
  remote returned the expected raw git-lfs error. Also Track pattern / Fetch / Pull / Prune.
- **T10:** Quick Launch (Ctrl+P) mixes commands, repos and branches, but the command list is short;
  rebase, merge, cherry-pick and hunk staging are mouse-only. Quick Launch lists all recent repos,
  so it was not screenshotted.

## 3. Sublime Merge build 2136 (unregistered)

- **Setup:** `smerge.exe <path>`, no first-run screen. A "Purchase / Cancel" nag modal appeared after
  a few actions; evaluation is unlimited.
- **T1/T3:** opens on the working directory with every changed file expanded in one scroll, the LFS
  image as side-by-side or onion skin with a slider (`ho-sm-workdir`).
- **T2:** hover a hunk → "Stage Hunk / Discard Hunk" on the hunk's own header row; one click.
- **T4:** no todo list. Right-click → Edit → Edit Message (inline, Ctrl+Enter) / Edit Contents /
  Squash with Parent / Move Up / Down / Drop; fixup-style squash needs a multi-selection
  (`ho-sm-edit-menu`). Squash with Parent kept "fixup! …" in the body; each edit is its own rewrite.
  Checkout with a dirty tree failed with a raw git toast and no offer to stash.
- **T5:** merge dialog lists flags and **"Runs git command git merge conflict/theirs"**; no conflict
  prediction (`ho-sm-merge-dialog`). Conflict markers appear inline; per file: Resolve / Using Ours /
  Using Theirs (`ho-sm-conflicts`). The 3-way tool is ours | merged | theirs with an arrow per hunk
  (Ctrl+Shift+]), Prev/Next Conflict, "all conflicts resolved", Save and Stage (`ho-sm-3way`).
  A one-click "Stage all unmerged files to continue" sits next to Abort.
- **T6/T7:** Ctrl+S stashes without asking, Ctrl+Shift+S pops. Palette "Stash" and context
  Cherry Pick open flag pickers (include untracked / keep index; `--no-commit` / `-x`).
- **T8:** discard is an in-place two-click confirm (button turns red "Discard!", Shift+Backspace),
  no modal. Ctrl+Z / Edit → Undo only undoes text edits; the discard was gone. Reset submenu lists
  Soft / Mixed / Keep / Hard with one-line explanations, no confirm.
- **T9:** no lock UI; the file menu offers "LFS Track File" only.
- **T10:** Ctrl+Shift+P palette shows each command's git equivalent and shortcut, and chains into
  argument pickers (`ho-sm-palette`). Enter inside palette prompts didn't register via computer use;
  that's a tool quirk, not counted against the app.

## 4. GitButler 0.22.3

- **Setup:** "Before we begin": error reporting and usage metrics both default **on** (turned off).
  Update to 0.23.5 offered twice (declined). Sign-in is only for AI features (skipped). Configure
  workspace: GitButler moves HEAD to `gitbutler/workspace`.
- **LFS:** a PSA toast: worktree filter `lfs` detected, filters "will silently not be applied during
  workspace operations", lists the 185 LFS files and suggests `git lfs pull --include`. It came back
  later (`ho-gb-lfs-psa`).
- **T1:** workspace = lanes of applied stacks; the Branches view is a list with ahead/behind counts
  plus a linear commit list. No view shows how branches relate.
- **T2:** "Start a commit" puts checkboxes on files, hunks and lines in the diff (`ho-gb-hunks`); the
  commit lands on an auto-named virtual branch (`ht-branch-1`), and HEAD becomes a synthetic
  "GitButler Workspace Commit". (landscape.md says hunks only, GB5; line ticks were visible here.)
- **T3:** LFS pointer text diff only (`ho-gb-lfs-diff`).
- **T4:** applying `feature/stack` was refused: "conflicts with stack ht-branch-1. Unapply it first"
  (`ho-gb-apply-refused`). After unapplying: Reword inline; squash = drag a commit onto another, the
  target labelled "Squash" (`ho-gb-squash-drag`). The fixup line stayed in the body. Then a banner
  "This branch and its remote have diverged" with an orange **Update local branch…** button that
  would pull the pre-rewrite commits back; Force push is the small one (`ho-gb-diverged`).
- **T5:** no merging one branch into another. Applying `conflict/theirs` was refused the same way.
- **T6/T7:** no stash or cherry-pick; file menu has "Stash into branch…" and "Absorb changes".
- **T8:** discard has a modal listing files. **Operations history** (Ctrl+3) logs every op with the
  files it touched; hover → Revert brought the discarded change back, and the revert is itself
  logged (`ho-gb-oplog`). Undo commit: "Uncommit" puts the changes back into the stack's staged area.
- **T9:** no lock UI.
- **T10:** no palette. Ctrl+1/2/3 switch views, Ctrl+B new branch, Ctrl+Enter commits;
  Ctrl+Shift+P falls through to WebView2's print dialog.

## 5. Steal / avoid (Claude's pick)

**Steal**

1. Pre-flight conflict prediction on merge, rebase and cherry-pick dialogs (Fork).
2. Rebase as a todo list edited up front: autosquash pre-applied, single-key actions, inline reword,
   "backup to a temporary branch" checkbox (Fork).
3. An op log that covers the working tree: every op, discards included, revertible in one click, and
   reverts logged too (GitButler). Neither Fork nor Sublime could recover a discard.
4. LFS image diff with side-by-side / swipe / onion skin, size and dimensions (Fork).
5. Palette entries that show the git command they run, chaining into flag pickers (Sublime); the
   same "Runs git command …" line in dialogs.
6. Hunk actions on the hunk's own row, always reachable (Sublime); checkbox selection of hunks/lines
   as a second way to build a commit (GitButler).
7. Ours | merged | theirs merge with per-hunk arrows and a resolved counter (Sublime, Fork).
8. Stash drawn as a graph node; branches grouped by prefix folder (Fork).
9. In-place two-click confirm for discard instead of a modal (Sublime).
10. Labelled drop targets for drag-and-drop history edits (GitButler).

**Avoid**

1. Any workspace model that bypasses git's smudge/clean filters; LFS repos are the target
   (GitButler).
2. Hover controls pinned to the far edge of wide panes (Fork on ultrawide).
3. Raw git output as the error UI: checkout-with-dirty-tree toast with no "stash and switch" offer
   (Sublime), git error modals (Fork).
4. After a deliberate rewrite, a loud "update from remote" button that undoes it (GitButler).
5. "Squash" that keeps `fixup!` lines; fixup should drop the message (Sublime, GitButler).
6. One-click "stage all unmerged files" next to Abort (Sublime).
7. Telemetry on by default.
8. No UnityYAMLMerge integration: none of the three offers it for `.prefab` / `.unity`. gitgud should
   detect Unity and offer it first.

## 6. Open

- Sourcetree and lazygit columns (see top).
- Fork's lock window against a real LFS server (the local remote can't lock).
- Whether Sublime Merge's Enter-in-palette issue is computer-use specific (likely).
