# UX principles for gitgud

Track D (ux), 2026-11-21. Status: **research proposal**. Nothing here is decided. All links checked
2026-11-21 unless marked otherwise. Every principle has a **test**: one sentence you can hold a mockup
or build against and answer pass/fail.

Pains: **P1** slow on big/LFS repos · **P2** Unity merge conflicts · **P3** scary history edits ·
**P4** clunky UI/workflow.

Labels: `fact` (sourced) · `Claude's pick` · `open`.

## Summary table

| # | Principle | Test (pass/fail) | Pains |
|---|---|---|---|
| 1 | Latency budget per action | Local action shows its result ≤100 ms; >1 s shows in-place progress; >10 s shows % and Cancel | P1 P4 |
| 2 | Never drop a frame | Graph and diff scroll at display refresh on a 100k-commit repo | P1 |
| 3 | Undo everything | Ctrl+Z reverses the last repo change, discard and rebase included, named in a history list | P3 |
| 4 | Preview before rewrite | Every history edit shows the resulting graph before it runs | P3 |
| 5 | Confirm only the irreversible | Dialogs appear only for non-undoable ops and name the target and outcome | P3 P4 |
| 6 | Keyboard-first, palette for all | Every action is in the palette with its shortcut; daily loop needs no mouse | P4 |
| 7 | Dense, with hierarchy | ≥ ~35 commit rows visible at 1080p; hierarchy from type/contrast, not boxes | P4 |
| 8 | Progressive disclosure, two levels | Daily loop on the main surface; anything else ≤1 step away and in the palette | P4 |
| 9 | Direct manipulation + keyboard twin | Drags preview their outcome on hover; each has a keyboard equivalent | P3 P4 |
| 10 | Live state, no refresh button | External changes appear ≤1 s without a click; local ops update optimistically | P1 P4 |
| 11 | Feedback where you look | Results/errors inline and persistent; toasts only for off-screen, non-error news | P4 |
| 12 | Make git's hidden state visible | HEAD, index, detached state and in-progress ops are always on screen | P2 P3 |
| 13 | Errors and empty states teach | Each error says what, why (in git terms), and offers the fix | P2 P4 |
| 14 | Readable diffs by default | Syntax + intra-line + moved-code highlighting; split/unified one key | P2 P4 |
| 15 | Color never alone, both themes first-class | Lanes/status readable in grayscale; WCAG AA in light and dark | P4 |
| 16 | Behave like a native desktop app | Standard OS shortcuts, menus, scaling, high-contrast all work | P4 |

---

## 1. Latency budget per action

- **Rule:** Local actions (select commit, stage hunk, switch file, open palette) paint their result in
  ≤100 ms, aim ≤50 ms. 0.1–1 s: no spinner needed but cursor/inline hint. >1 s: in-place progress.
  >10 s (fetch, LFS pull, clone): percent-done, background, **Cancel**.
- **Why (fact):** Nielsen's limits: 0.1 s feels instantaneous, 1 s keeps flow, 10 s holds attention;
  long ops need percent-done and a way to interrupt ([NN/g, source 1993](https://www.nngroup.com/articles/response-times-3-important-limits/)).
  Doherty threshold: productivity rises under 400 ms response ([Laws of UX](https://lawsofux.com/doherty-threshold/), source 1982).
  Superhuman treats 100 ms as a ceiling, targets <50 ms, and buckets events <50/<100/<1000 ms
  ([Superhuman, source 2022](https://blog.superhuman.com/superhuman-is-built-for-speed/)).
  RAIL: handle input in 50 ms, feedback for anything >50 ms ([web.dev](https://web.dev/articles/rail)).
  Even 100 ms is not "instant" for simple tasks; people perceive far less ([Dan Luu, source 2017](https://danluu.com/input-lag/)).
- **gitgud (Claude's pick):** Telemetry-free local histogram of action latency in a debug panel with
  the Superhuman buckets; a perf regression test fails CI if p95 select-commit >50 ms on a fixture repo.
  LFS pull runs as a cancellable background job with bytes/files done.

## 2. Never drop a frame

- **Rule:** Commit graph, file list and diff scroll at the display's refresh rate (16.7 ms at 60 Hz,
  8.3 ms at 120 Hz) on a 100k-commit, 50k-file repo. No layout work proportional to repo size on scroll.
- **Why (fact):** Zed's frame budget at 120 Hz is 8.33 ms; they left Atom/Electron because GC pauses and
  DOM relayout missed frames ([Zed, source 2023](https://zed.dev/blog/videogame)). RAIL budgets ~10 ms of
  app work per frame in a browser engine ([web.dev](https://web.dev/articles/rail)) — relevant because
  Tauri renders in WebView2.
- **gitgud (Claude's pick):** Virtualized rows, graph lanes precomputed in Rust and streamed in pages;
  canvas for the graph (spike in plan §4 decides). Retcon's author reports a rewrite was needed to handle
  hundreds of thousands of commits without freezing ([HN, 2025](https://news.ycombinator.com/item?id=44953580)) —
  budget for it from day one.

## 3. Undo everything

- **Rule:** Ctrl+Z reverses the last repo-changing operation — commit, amend, discard, stage/unstage,
  checkout, branch create/delete, merge, rebase, stash ops — and Ctrl+Shift+Z / Ctrl+Y redoes. An
  operations list names every entry ("Undo Squash 3 commits") and can restore any earlier point. Works
  after restart and sees ops done from the CLI.
- **Why (fact):**
  - Tower: Ctrl+Z undoes file deletion, (un)staging, discarding chunks, commits, branch ops, merges,
    interactive rebases, publishing, stashes ([Tower](https://www.git-tower.com/features/undo)).
  - GitButler oplog snapshots branch state, uncommitted work and conflict state into the git object DB
    before each major action; "Revert" restores it ([GitButler docs](https://docs.gitbutler.com/features/timeline)).
  - jj records every operation as a "view" (refs, heads, working-copy commit); `undo`, `op revert`,
    `op restore` ([jj docs](https://docs.jj-vcs.dev/latest/operation-log/)).
  - lazygit undo is reflog-based: it can't undo working-tree changes, stashes, branch creation or
    pushes, nor work mid-rebase ([lazygit docs](https://github.com/jesseduffield/lazygit/blob/master/docs/Undoing.md)).
    **Reflog alone is not enough.**
  - Apple HIG: label undo with its result, bring the restored thing into view, unlimited levels
    ([HIG](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo)).
  - NN/g heuristic 3, user control and freedom ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/)).
- **gitgud (Claude's pick):** jj/GitButler-style op log in `gitgud-core`: before each write, snapshot
  refs + index + dirty working-tree files (as blobs/a tree under a private ref, e.g.
  `refs/gitgud/oplog`). Discard becomes recoverable. Pushes are marked "can't undo remotely" with a
  revert-commit offer. After undo, scroll the graph to the restored commit (HIG "show the results").

## 4. Preview before rewrite

- **Rule:** Any history edit (rebase, squash, fixup, reorder, drop, reset, cherry-pick range) shows the
  resulting graph and the commits that will change hash *before* it runs, plus predicted conflicts if
  computable.
- **Why (fact):** NN/g heuristic 5, error prevention ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/));
  HIG "predict the results" — describe the outcome before people act ([HIG](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo)).
  Max's pain P3. Retcon markets history rewriting without stashing and with editing while in conflict
  ([HN, 2025](https://news.ycombinator.com/item?id=44953580)).
- **gitgud (Claude's pick):** Interactive rebase as an editable list beside a ghost graph: rewritten
  commits drawn hollow, original tips dimmed. Conflict prediction via an in-memory merge (`gix`
  merge-tree) per step, flagged on the step. Enter runs; Esc cancels; Ctrl+Z after.

## 5. Confirm only the irreversible

- **Rule:** No "Are you sure?" for undoable actions. A dialog appears only for ops the op log can't
  reverse (force-push, delete remote branch, LFS prune, gc). It names the target ("Force-push
  `feature/x` to `origin`, overwriting 3 remote commits"), buttons say the outcome ("Force-push" /
  "Keep remote"), and there is no default button for the dangerous one.
- **Why (fact):** Confirmations only for serious, irreversible consequences; overuse trains
  click-through; restate the request with identifying details; outcome-labelled buttons; avoid a default
  "Yes"; for the rarest dangerous ops require an unusual step ([NN/g, source 2018, reviewed 2026](https://www.nngroup.com/articles/confirmation-dialog/)).
  lazygit: confirm hard-to-reverse actions, Escape exits transient modes ([lazygit VISION](https://github.com/jesseduffield/lazygit/blob/master/VISION.md)).
- **gitgud (Claude's pick):** Default to `--force-with-lease` and say so in the dialog; typing the branch
  name only for deleting a protected/remote default branch.

## 6. Keyboard-first, palette for everything

- **Rule:** Every command exists in a fuzzy command palette (Ctrl+K and Ctrl+Shift+P), each row shows its
  shortcut; `?` opens a searchable shortcut sheet; the daily loop (pick file → stage hunk → write message →
  commit → push) runs with no mouse; focus is always visible and moves between panes with one key.
- **Why (fact):** Linear's Cmd/Ctrl+K command menu runs any action or jumps to any page
  ([Linear guide](https://linear.app/enablement/guides/navigating-linear)); `?` opens a searchable
  shortcut help, built to convert users to shortcuts ([Linear changelog, source 2021](https://linear.app/changelog/2021-03-25-keyboard-shortcuts-help)).
  Superhuman: every action has a shortcut, Cmd+K command library, hints in the UI teach the faster path
  ([Superhuman, source 2022](https://blog.superhuman.com/superhuman-is-built-for-speed/)). NN/g heuristics
  6–7: recognition over recall, accelerators for experts ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/)).
  Sublime Merge ships a command palette ([sublimemerge.com](https://www.sublimemerge.com/)).
- **gitgud (Claude's pick):** Palette is context-aware (commit selected → commit actions first) and
  takes arguments inline ("checkout ▸ fuzzy branch list"). Shortcut hints appear in tooltips and context
  menus. Keymap is data (one file) so the spec's keyboard map is the source of truth.

## 7. Dense, with hierarchy

- **Rule:** One line per commit and per file; at 1080p ≥ ~35 commit rows visible (Claude's pick:
  22–24 px rows). Hierarchy comes from type weight, contrast and alignment, not borders and cards. No
  element without a job.
- **Why (fact):** Linear's 2024 redesign reduced visual noise and increased hierarchy and density of
  navigation; layered surfaces by elevation ([Linear, source 2024](https://linear.app/now/how-we-redesigned-the-linear-ui)).
  WinUI offers a Compact density for information-rich, pointer-driven UI
  ([Microsoft Learn spacing](https://learn.microsoft.com/en-us/windows/apps/design/style/spacing)); Windows
  spacing ramp 8/12/16 epx, Body Strong for headers in confined space ([Microsoft Learn content basics](https://learn.microsoft.com/en-us/windows/apps/design/basics/content-basics)).
  NN/g heuristic 8, minimalist design ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/)).
  Max's rule B7: clean and dense, minimal chrome.
- **gitgud (Claude's pick):** 4 px grid, Compact by default, a single density toggle (Compact/Standard)
  rather than per-panel settings. Secondary metadata (author, date) at lower contrast, right-aligned.

## 8. Progressive disclosure, max two levels

- **Rule:** The main surface shows only the daily loop (status, graph, diff, commit box, branch
  switcher). Advanced options (rebase strategies, signing, merge drivers, LFS settings) are one step
  away, never more than two levels deep, and always reachable from the palette. Disabled items stay in
  place with a reason, not hidden.
- **Why (fact):** Show the few most important options first, the rest on request; obvious path to the
  second level; ≤2 levels ([NN/g, source 2006](https://www.nngroup.com/articles/progressive-disclosure/)).
  Hick's law: more choices, slower decisions ([Laws of UX](https://lawsofux.com/hicks-law/)). lazygit
  disables rather than hides menu items to protect muscle memory and explains why a binding is disabled
  ([lazygit VISION](https://github.com/jesseduffield/lazygit/blob/master/VISION.md)).
- **gitgud (Claude's pick):** Commit box has message + Commit; amend/sign-off/no-verify behind one
  "⋯" and as palette commands. Unity-specific surfaces (lock panel, `.meta` warnings) appear only when
  the repo is detected as Unity.

## 9. Direct manipulation with a keyboard twin

- **Rule:** Drag a commit onto a commit = squash/fixup; drag along the list = reorder; drag a commit to
  another branch = cherry-pick; drag a file/hunk onto a commit = amend into it. While hovering, the drop
  target previews the outcome (text + ghost graph); Esc cancels. Every drag has a keyboard equivalent.
- **Why (fact):** GitButler does squash, reorder, amend-by-dropping-files and split via drag-and-drop,
  replacing `rebase -i` ([GitButler docs](https://docs.gitbutler.com/features/branch-management/commits)).
  lazygit copies commits with one key and pastes them elsewhere — a keyboard form of the same move
  ([lazygit.dev](https://lazygit.dev/)). Fitts's law: bigger, nearer targets are faster
  ([Laws of UX](https://lawsofux.com/fittss-law/)). Nielsen: 0.1 s feedback is what makes it feel like
  direct manipulation ([NN/g](https://www.nngroup.com/articles/response-times-3-important-limits/)).
- **gitgud (Claude's pick):** Ctrl+X / Ctrl+V on commits (cut = move, copy = cherry-pick), Alt+↑/↓ to
  reorder, `S` to squash into parent. Drop zones are whole rows, not tiny icons.

## 10. Live state, no refresh button

- **Rule:** Changes made outside gitgud (editor saves, Unity reimports, CLI commits, agents) appear
  within ~1 s without user action. Local ops update the UI optimistically and roll back visibly if git
  fails. There is no Refresh button in the main UI.
- **Why (fact):** Git's built-in fsmonitor daemon watches the worktree so `status` asks for changed paths
  instead of scanning ([git-fsmonitor--daemon](https://git-scm.com/docs/git-fsmonitor--daemon)).
  Superhuman keeps a local DB and prerenders likely-next threads for speed
  ([Superhuman, source 2022](https://blog.superhuman.com/superhuman-is-built-for-speed/)). NN/g heuristic 1,
  visibility of system status ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/)).
- **gitgud (Claude's pick):** `notify` watcher + debounce (~100 ms), coalesce Unity's burst writes to
  `Library/` by honoring `.gitignore` before rescanning. Optionally enable `core.fsmonitor` per repo
  (opt-in, it writes config). A subtle "updated" indicator on rows that changed externally.

## 11. Feedback where you look

- **Rule:** The result of an action appears where the user was looking (the row changes, the graph
  moves). Errors stay inline until dismissed, with git's raw output one click away. Toasts only for
  passive, non-error news about something off-screen (fetch finished, agent staged a commit).
- **Why (fact):** Toasts suit passive notifications but are poor for errors — they vanish before being
  noticed; pick indicator/validation/notification by cause and urgency ([NN/g, 2024](https://www.nngroup.com/articles/indicators-validations-notifications/)).
  lazygit shows a toast only when the affected entity is off-screen ([lazygit VISION](https://github.com/jesseduffield/lazygit/blob/master/VISION.md)).
- **gitgud (Claude's pick):** One status strip per pane for running jobs; failed push shows an inline
  banner on the branch with "Pull and retry" / "Show output".

## 12. Make git's hidden state visible

- **Rule:** Always on screen: current branch or a loud **Detached HEAD at abc123** state with "create
  branch here"; staged vs unstaged vs untracked as distinct groups; ahead/behind; any in-progress
  operation (rebase 3/7, merge, cherry-pick, bisect) as a persistent banner with Continue / Skip / Abort.
- **Why (fact):** Perez De Rosso & Jackson identify git's concepts (staging area, tracked file, stash,
  branch) as hard to grasp and misfitting user purposes; ending up in detached HEAD when going back to an
  old commit is a listed misfit ([Onward! 2013 paper, MIT](https://dspace.mit.edu/bitstream/handle/1721.1/105233/onward009-perez_de_rosso.pdf);
  [summary of the 2016 OOPSLA follow-up](https://blog.acolyer.org/2016/10/25/purposes-concepts-misfits-and-a-redesign-of-git/), sources 2013/2016).
  Gitless removes the staging area and keeps working changes per branch so switching needs no stash
  ([gitless.com](https://gitless.com/)). lazygit marks "YOU ARE HERE" during rebase and surfaces the
  rebase-options key prominently ([lazygit VISION](https://github.com/jesseduffield/lazygit/blob/master/VISION.md)).
- **gitgud (Claude's pick):** Keep the index (Max stages hunks) but make it spatial: two stacked lists
  with drag between them. Conflict state per file (code vs Unity YAML vs binary/LFS) shown as distinct
  types so P2 conflicts route to the right resolver.

## 13. Errors and empty states teach git

- **Rule:** Every error message states (a) what happened in plain words, (b) the git concept behind it,
  (c) one primary fix button, (d) raw stderr collapsed. Empty states show the next action (empty repo →
  "Make first commit"; no conflicts → nothing; locked file → who holds the lock and "Request unlock").
- **Why (fact):** NN/g heuristic 9: plain-language errors that explain and suggest a fix; heuristic 10:
  task-focused help ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/)). lazygit aims to
  document how to solve git problems inside the tool ([lazygit VISION](https://github.com/jesseduffield/lazygit/blob/master/VISION.md)).
- **gitgud (Claude's pick):** Map the top ~20 git failure strings (non-fast-forward, dirty tree on
  checkout, LFS smudge failure, lock held by someone else, merge conflict in `.unity`/`.prefab`) to
  explained fixes. Unknown errors fall back loudly to raw output (Max's B8, fail loud).

## 14. Readable diffs by default

- **Rule:** Diffs ship with syntax highlighting, intra-line (word/char) change highlighting,
  moved-code coloring, a readable algorithm (histogram), whitespace-ignore toggle, and a one-key
  split/unified toggle. Hunks are stageable by line from either mode.
- **Why (fact):** git supports `--word-diff`, `--color-moved=zebra|dimmed-zebra` (moved blocks with
  borders highlighted) and `patience`/`histogram` algorithms for readability ([git-diff docs](https://git-scm.com/docs/git-diff)).
  Sublime Merge advertises syntax highlighting, character diffs and side-by-side ([sublimemerge.com](https://www.sublimemerge.com/)).
  difftastic diffs by syntax tree with tree-sitter and ignores formatting-only changes
  ([difftastic](https://difftastic.wilfred.me.uk/)). Fork's syntax-highlighting request drew 61 reactions
  ([Fork #56](https://github.com/fork-dev/TrackerWin/issues/56), source 2018).
- **gitgud (Claude's pick):** Unified by default under ~1400 px pane width, split above. Unity YAML gets
  a semantic view (object/component names instead of `fileID` noise) as a toggle — `open` whether this
  is MVP (feeds P2). Binary/LFS assets: image before/after/onion-skin instead of "binary file changed".

## 15. Color never alone; both themes first-class

- **Rule:** Graph lanes, file status and diff +/- are distinguishable in grayscale (shape, glyph,
  position or text as well as color). Text ≥4.5:1, UI glyphs/lane lines ≥3:1, in light and dark.
  Lane palette is color-blind safe. Themes come from a few tokens, and the app follows the OS theme by
  default.
- **Why (fact):** WCAG 1.4.1: color must not be the only means of conveying information
  ([W3C](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)); 1.4.3 text contrast 4.5:1
  ([W3C](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)); 1.4.11 non-text contrast
  3:1 ([W3C](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)). Okabe-Ito is a
  categorical palette that survives common color-vision deficiencies, but color-only coding is never
  fully safe ([data.europa.eu guide](https://data.europa.eu/apps/data-visualisation-guide/accessible-colour-palettes)).
  Linear generates all themes in LCH from base, accent and contrast, with a contrast knob for
  accessibility ([Linear, source 2024](https://linear.app/now/how-we-redesigned-the-linear-ui)). GitHub
  Desktop's dark-theme request got 159 reactions ([desktop #1515](https://github.com/desktop/desktop/issues/1515), source 2017).
- **gitgud (Claude's pick):** Status letters (M/A/D/R/U/C) next to colors; lanes keep a stable color per
  branch and the selected branch's lane is thicker; diff uses +/- gutter glyphs. Theme tokens in OKLCH.

## 16. Behave like a native desktop app

- **Rule:** Standard OS shortcuts work (Ctrl+Z/Y, Ctrl+C on a commit copies its hash, F2 rename branch,
  Del delete with undo, Ctrl+F find, Ctrl+, settings); right-click menus everywhere; follows OS
  theme, accent, text scaling and Windows high-contrast mode; screen reader announces rows (role,
  name, status). macOS swaps to Cmd later with no other change.
- **Why (fact):** NN/g heuristic 4, consistency and standards ([NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/));
  macOS undo/redo belong at the top of Edit with Cmd-Z / Shift-Cmd-Z ([HIG](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo));
  Windows content/spacing guidance ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/design/basics/content-basics)).
  Raycast renders extensions as native UI so they don't feel like a webview mini-app
  ([Raycast blog](https://www.raycast.com/blog/how-raycast-api-extensions-work)).
- **gitgud (Claude's pick):** Keymap file has a `ctrl`/`cmd` "mod" abstraction from day one; virtualized
  lists still expose ARIA `grid`/`treegrid` rows (`open`: verify Narrator + WebView2 with virtualization).

---

## Anti-patterns seen in today's git GUIs

| Anti-pattern | Evidence | Principle |
|---|---|---|
| Freezes on big/Unity repos; UI blocked by git calls | Sourcetree "Hangs (not responding) … Unity Large Project" thread; per search snippet the repo was 90 GB+ and a moderator said that scale isn't routinely tested ([Atlassian community, 2013–2019 era, thread body not loadable](https://community.atlassian.com/forums/Sourcetree-questions/SourceTree-Hangs-not-responding-on-Windows-10-Unity-Large/qaq-p/619773)); "Source Tree slow on big repositories" ticket ([SRCTREEWIN-2602](https://jira.atlassian.com/browse/SRCTREEWIN-2602)) | 1, 2, 10 |
| Missed frames from a web stack doing DOM layout per frame | Zed on Atom: GC pauses and relayouts made frame rate inconsistent ([Zed, 2023](https://zed.dev/blog/videogame)) | 2 |
| Undo that can't undo a discard | Reflog-only undo skips working tree, stash, branch creation ([lazygit docs](https://github.com/jesseduffield/lazygit/blob/master/docs/Undoing.md)); "Undo discard files" open request ([Fork #657](https://github.com/fork-dev/TrackerWin/issues/657), source 2020) | 3 |
| Keyboard as an afterthought | Fork's "Keyboard shortcuts" issue open since 2019, 161 reactions — its top request ([Fork #333](https://github.com/fork-dev/TrackerWin/issues/333)) | 6 |
| Clutter: everything on screen at once | HN user gave up on Sourcetree, GitKraken and others over too many UI elements in random places ([HN, 2022](https://news.ycombinator.com/item?id=33740619)) | 7, 8 |
| Hiding the graph | GitHub Desktop "History graph" request, 295 reactions, open since 2020 ([desktop #9452](https://github.com/desktop/desktop/issues/9452)) | 12 |
| Generic "Are you sure?" with Yes/No | NN/g's multi-item delete example: no target details, Yes/No labels ([NN/g](https://www.nngroup.com/articles/confirmation-dialog/)) | 5 |
| Errors in disappearing toasts | Toast errors missed for minutes in testing ([NN/g, 2024](https://www.nngroup.com/articles/indicators-validations-notifications/)) | 11 |
| Rebase/conflict mode with no visible "where am I" | Detached HEAD / staging confusion as concept misfits ([Perez De Rosso & Jackson](https://blog.acolyer.org/2016/10/25/purposes-concepts-misfits-and-a-redesign-of-git/)) | 12 |

## Open / not verified

- `open` — **Name clash:** a "GitGud" open-source Git GUI with undo for any action was posted on HN on
  2026-10-17 ([HN](https://news.ycombinator.com/item?id=50132140), repo `zeuros/gitgud`); a commenter
  lists other existing gitgud projects. Max should decide whether to rename before going public.
- `open` — Fluent 2 has no published Compact token set for web; WinUI Compact numbers were not
  extracted. Arc, Figma and Raycast's keyboard docs were not fetched in this pass.
- Apple HIG content came from its JSON data endpoint; Atlassian Sourcetree threads are 2013–2019 and
  were read via search snippets, not opened.
- Screenshots and annotations: [`shots/ux.json`](shots/ux.json).
