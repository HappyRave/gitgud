# Landscape: today's Git GUIs

Track A (gui). All links checked **2026-11-21** unless a row says otherwise. Labels: `fact` (sourced),
`Claude's pick`, `open`. Screenshots with pins: `docs/research/shots/gui.json` (17 shots, `gui-*`).
Cells come from docs, changelogs, issues and threads, except those marked `hands-on`: settled in the
2026-11-21 install round (Fork, Sublime Merge, GitButler; see `handson.md`).

Legend: ✓ yes · ✗ no · ~ partial · ? not verified this pass. Cell sources are the per-tool source keys
in section 3 (e.g. `GK2`).

## 1. Feature matrix

Columns: GK GitKraken · Fork · SM Sublime Merge · Tow Tower · GHD GitHub Desktop · SG SourceGit ·
GB GitButler · SmG SmartGit · ST Sourcetree · lg lazygit · Mg Magit · VSC VS Code · JB JetBrains ·
GC GitComet (2026 newcomer) · GG (jj GUI).

| Feature | GK | Fork | SM | Tow | GHD | SG | GB | SmG | ST | lg | Mg | VSC | JB | GC | GG |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Windows | ✓ GK3 | ✓ F1 | ✓ SM1 | ✓ T1 | ✓ D1 | ✓ SG1 | ✓ hands-on | ✓ SmG1 | ✓ ST1 | ✓ L1 | ✓ (Emacs) | ✓ | ✓ | ✓ GC1 | ✓ GG1 |
| Linux | ✓ GK3 | ✗ F1, F5 | ✓ SM1 | ✗ T1 | ✗ D3 | ✓ SG1 | ? | ✓ SmG1 | ✗ ST1 | ✓ | ✓ | ✓ | ✓ | ✓ GC1 | ✓ GG1 |
| Open source | ✗ | ✗ | ✗ | ✗ | ✓ MIT M1 | ✓ MIT M1 | ~ FSL→MIT GB4 | ✗ | ✗ | ✓ MIT M1 | ✓ GPL M1 | ~ (Code-OSS) | ✗ | ✓ AGPL + paid Pro GC1 | ✓ Apache GG1 |
| Commit graph | ✓ GK3 | ✓ F1 | ✓ SM1 | ✓ T4 | ✗ D3 (#9452) | ✓ SG1 | ~ "smartlog" GB1 | ✓ SmG2 | ✓ ST1 | ✓ L1 | ~ log | ✓ VSC1 | ✓ JB1 | ✓ GC1 | ✓ GG1 |
| WIP shown as graph node | ✓ GK3 shot | ✗ F5 (#308) | ✓ shot | ? | ✗ | ✗ SG3 (#1673) | ✓ GB1 | ? | ✓ ST shot | ✗ | ✗ | ? | ? | ✓ GC shot | ✓ (working copy is a commit) GG1 |
| Line-level staging | ? | ✓ F1 | ✓ SM1 | ? | ✓ D shot | ? | ~ hunks only GB5 (#2513) | ? | ✓ ST1 | ✓ L1 | ✓ | ✓ VSC1 | ? | ✓ GC1 | ~ file drag GG1 |
| Interactive rebase UI | ✓ GK2 | ✓ F1 | ~ commit edit SM1 | ✓ T2 | ~ reorder/squash D2 | ✓ SG1 | ✓ (no rebase needed) GB1 | ✓ SmG2 | ✓ ST1 | ✓ L1 | ✓ | ✗ (CLI) | ✓ JB2 | ✓ GC1 | ✓ GG1 |
| Drag-and-drop history ops | ? | ✓ F2, F3 | ✗ SM2 (#1194) | ✓ T3 | ? | ? | ✓ GB1 | ? | ? | n/a | n/a | ✗ | ? | ? | ✓ GG1 |
| Multi-step undo | ~ last action GK2 | ✗ (reflog restore) F1 | ✗ Undo is text-only, hands-on | ✓ Ctrl+Z/redo T2 | ~ undo last commit D2 | ? | ✓ op log GB3 | ? | ✗ | ✓ reflog-based L1 | ? | ~ undo last commit VSC1 | ~ Local History JB3 | ? | ✓ op log GG1 |
| Undo covers working tree | ✗ GK2 | ✗ | ✗ discard unrecoverable, hands-on | ✓ discard/stage T2 | ✗ | ? | ✓ snapshots files GB3 | ? | ✗ | ✗ L1 | ? | ✗ | ✓ Local History JB3 | ? | ✓ (jj snapshots) |
| Reflog UI | ? | ✓ F1 | ? | ✓ T4 | ✗ | ? | ✓ oplog | ? | ? | ✓ L1 | ✓ | ✗ | ? | ? | ✓ op log |
| Built-in 3-way merge tool | ✓ GK3 | ✓ F1 | ✓ SM1, SM3 | ✓ wizard T4 | ~ Copilot assist D1 | ~ no text edit SG3 (#2168) | ~ conflicted commits GB2 | ✓ SmG2 | ✗ external | ~ | ✓ (ediff) | ✓ VSC2 | ✓ JB4 | ✓ GC1 | ✗ external GG1 |
| Rebase continues past conflicts | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ GB2 | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ (jj) |
| AI commit message | ✓ GK1 | ✓ via local agent F2 | ? | ✓ T4 | ✓ Copilot D1 | ✓ BYO API SG1 | ✓ agent-driven GB6 | ? | ✗ | ✗ | ? | ✓ Copilot | ? | ~ Pro only GC1 | ✗ |
| AI conflict resolution | ✓ GK3 | ✗ | ✗ | ? | ✓ Copilot D1 | ✗ | ✗ | ? | ✗ | ✗ | ✗ | ~ experimental VSC2 | ? | ? | ✗ |
| Git LFS UI | ✓ track/pull GK4 | ✓ F1 | ~ needs system git-lfs SM4 | ✓ T4 | ~ bundled, CLI setup D4 | ✓ SG1 | ✗ GB5 (#2595, open since 2024) | ✓ SmG1 | ✓ ST1 (slow, ST3) | ~ transparent | ? | ? | ? | ✓ GC1 | ? |
| LFS file locking UI | ✗ not documented GK4 | ✓ F3 (source 2020) | ✗ only "LFS Track File", hands-on | ✗ T4 | ✗ D4 | ? | ✗ | ? | ? | ✗ | ✗ | ✗ | ? | ✓ GC1 | ✗ |
| Image diff (swipe/onion) | ? | ✓ F3 | ~ side-by-side + onion, no swipe, hands-on | ✗ T4 | ? | ✓ SG1 | ✗ LFS pointer text only, hands-on | ? | ~ before/after ST1 | ✗ | ✗ | ? | ✓ | ? | ✗ |
| Worktrees | ✓ GK1 | ✓ F2 | ✓ SM2 | ✓ T4 | ~ D1 | ✓ SG1 | ~ prefers one folder GB5 (#10677) | ? | ? | ✓ L1 | ✓ | ✓ VSC1 | ✓ JB1 | ✓ GC1 | ✓ (jj workspaces) |
| Stacked branches | ? | ~ `--update-refs` F2 | ? | ✓ Pro T4 | ✗ | ? | ✓ GB1 | ? | ✗ | ~ L2 (#2527) | ? | ✗ | ? | ? | ✓ |
| Command palette / keyboard-first | ? | ✓ Quick Launch F3 | ✓ SM1 | ✓ Quick Actions T4 | ? | ~ SG3 (#2655) | ✓ CLI/TUI GB1 | ? | ✗ | ✓ L1 | ✓ | ✓ | ✓ | ? | ~ |
| Shows the git commands it runs | ? | ✓ console | ✓ SM1 | ? | ✗ | ✓ command log SG1 | ✗ | ? | ✓ | ✓ command log | ✓ M2 | ✓ output | ✓ console | ? | ✗ |
| Agent workflow features | ✓ agent sessions in worktrees GK1, GK5 | ~ agent code review F2 | ✗ | ✗ | ✗ | ✗ | ✓ `but` CLI + skill GB6 | ~ SmG1 | ✗ | ✗ | ✗ | ~ (Copilot) | ? | ? | ✗ |

`open`: the remaining `?` cells, especially LFS locking in SmartGit/Sourcetree/SourceGit and
GitKraken's keyboard story. The hands-on round skipped Sourcetree (Max's work install) and could not
drive lazygit (terminals are click-only for computer use).

## 2. Usage and market data

| Metric | Value | Source |
|---|---|---|
| How pros interact with VCS (SO 2022, last survey with that question found) | CLI ~84.6 %, code editor ~54.4 %, other GUIs lower (multi-select) | [DevClass on SO 2022](https://devclass.com/2022/06/23/vs-code-react-git-dominant-technologies-increase-their-grip-says-stack-overflow-survey) (source 2022) |
| Git share among pros (SO 2022) | 96.65 %, SVN 5.96 % | same |
| GitButler waitlist survey (n=1,253) | 82 % CLI; ~30 % use a GUI, only 8 % GUI-only | [GitButler "Git Zeitgeist"](https://blog.gitbutler.com/git-zeitgeist) (source 2024, biased sample) |
| JetBrains DevEco 2023 | Git "de facto standard"; IDE use for VCS rising; 35 % of Perforce users are game devs | [JetBrains Team Tools 2023](https://www.jetbrains.com/lp/devecosystem-2023/team-tools/) (source 2023) |
| SO 2023–2025 | No git-client question found | [SO 2025](https://survey.stackoverflow.co/2025/) |
| GitHub stars (2026-11-21, `gh api`) | lazygit 84.0k · Zed 95.5k · jj 32.6k · gitui 22.7k · GitHub Desktop 22.1k · GitButler 22.0k · tig 13.4k · GitLens 10.0k · Git Extensions 8.6k · Magit 7.3k · SourceGit 6.6k · git-branchless 4.1k · Gitnuro 2.8k · Gittyup 2.3k · jjui 2.2k · lazyjj 1.2k · GitComet 1.0k · GG 0.9k | GitHub API |
| Release-asset downloads (last 100 releases) | lazygit 3.81M · SourceGit 485k · GitHub Desktop 307k (undercounts: main installer is served from GitHub's CDN) | GitHub API |
| Issue tracker pressure (top 👍) | GHD: Linux 4,853, GPG signing 340, history graph 295 · Fork: Linux 591 · SM: diff non-git files 107, diff between branches 81 · GB: line-level moves 51, **LFS 38** · lazygit: maintainer handover 191 | `gh api search/issues sort=reactions` |
| Funding signals | GitButler raised $17M Series A (Apr 2026) | [blog](https://blog.gitbutler.com/series-a), [HN 47712656](https://news.ycombinator.com/item?id=47712656) |
| Consolidation | Cursor acquired Graphite (stacked PRs), Dec 2025 | [TechCrunch](https://techcrunch.com/2025/12/19/cursor-continues-acquisition-spree-with-graphite-deal) |

`fact`: no current survey measures per-client share. Stars and downloads favour open-source/TUI tools;
paid closed clients (Fork, Tower, SM, GitKraken) publish no counts.

## 3. Per-tool sections

Shared keys: **M1** = GitHub API repo data (stars, licence, language), pulled 2026-11-21 · **GC1** =
[GitComet README](https://github.com/Auto-Explore/GitComet) · **GG1** = [GG README](https://github.com/gulbanana/gg) ·
**F4** = user-opinion threads listed under Fork.

### GitKraken Desktop
- **Model:** classic branches; graph-centric. 2026 repositioning: "launch and manage parallel coding
  agents", each in its own worktree with a WIP node in the graph `fact` [GK1].
- **Price/licence:** closed. Free "Community" = local repos + public remotes only; Pro (≤2 seats),
  Advanced, Business, Enterprise; AI credits per seat; Pro ≈ $4.95/user/mo annual per third-party
  listings (official page didn't render a number) [GK6].
- **Stack:** Electron/React (their own [React perf post](https://gitkraken.com/blog/performance-solutions-react-js-pt-1), older source). Win/macOS/Linux.
- **Perf:** vendor's 11.5 post admits large repos opened too slowly, claims up to 480× faster open
  [GK7]; feedback board: LFS repos 10–20× slower with UI hangs (source 2022), 1.5 GB repo 5–10× slower
  than CLI (2024) [GK7].
- **Undo:** Ctrl+Z for the **most recent** supported action only (checkout, commit, discard, delete
  branch, reset, rebase incl. interactive, multi-cherry-pick); unavailable mid-merge/rebase [GK2].
- **LFS:** init/track/pull from UI, auto LFS pull after clone; needs system Git ≥2.39.3; locking not
  documented [GK4].
- **AI:** commit/PR/stash messages, "Commit Composer" to restructure WIP or agent commits, AI conflict
  resolution with confidence levels [GK3]. Agent-review doc: diff per agent worktree, combined range
  diff, revert hunk, "remove worktree and delete branch" [GK5].
- **Praise / complaints:** graph clarity; subscription + account wall and big-repo slowness [GK7].
- Sources: [GK1 git-client](https://www.gitkraken.com/git-client) · [GK2 undo](https://help.gitkraken.com/gitkraken-desktop/undo-and-redo/) · [GK3 git-client features](https://www.gitkraken.com/git-client) · [GK4 LFS](https://help.gitkraken.com/gitkraken-desktop/git-lfs/) · [GK5 agent changes](https://help.gitkraken.com/gitkraken-desktop/review-and-undo-agent-changes/) · [GK6 pricing](https://www.gitkraken.com/pricing), [toolradar](https://toolradar.com/tools/gitkraken/pricing) · [GK7 perf help](https://help.gitkraken.com/gitkraken-desktop/performance-issues/), [11.5 post](https://www.gitkraken.com/blog/gitkraken-desktop-11-5-we-fixed-what-mattered-most), [LFS feedback](https://feedback.gitkraken.com/suggestions/196652/comment/396848), [sparse feedback](https://feedback.gitkraken.com/suggestions/212258/comment/664876)

### Fork
- **Model:** classic; tabs per repo, sidebar groups branches by folder prefix.
- **Price:** $59.99 one-time, free evaluation; Windows 7+ and macOS only [F1]. Linux is the #1 request
  (591 👍) [F5].
- **Stack:** native per OS (WPF on Windows, AppKit on Mac) — inferred from separate Win/Mac release
  trains, `open`.
- **Mechanics:** line staging, interactive rebase, merge-conflict helper, image diffs, reflog restore,
  LFS + git-flow [F1]; 2020: drag-and-drop merge/rebase, **LFS file locking**, Quick Launch command
  palette, image swipe/onion-skin [F3] (source 2020). 2025–26 (Mac notes): drag-drop reorder/squash +
  **rebase conflict preview** (2.55), `--update-refs` in rebase (2.61/2.63), worktree dialog,
  **AI commit messages and branch review via the user's installed Claude Code / Codex / Cursor** (2.59,
  2.60, 2.64, 2.70) [F2].
- **Praise:** HN commenters call it the best GUI; a Reddit-aggregate puts it near the top; common
  "Sourcetree → Fork, never went back" story [F4]. **Complaints:** no Linux, no shortcut customisation
  (#309), no uncommitted changes in history (#308) [F5].
- Sources: [F1 fork.dev](https://fork.dev) · [F2 release notes](https://fork.dev/releasenotes) · [F3 blog](https://fork.dev/blog) · [F4 HN 45878578](https://news.ycombinator.com/item?id=45878578), [GummySearch](https://gummysearch.com/tools/best-products/git-gui/) · [F5 tracker](https://github.com/fork-dev/Tracker/issues/153)

### Sublime Merge
- **Model:** classic; three tabs (Locations/Commits/Files), command palette, shows exact git commands.
- **Price:** perpetual licence with 3 years of updates; unlimited evaluation, no enforced limit; build
  2136 on 2026-11-09 [SM1], [SM5]. Price on store page not rendered (`open`; historically $99).
- **Stack:** custom C++ cross-platform toolkit + own high-perf git reading library; Win/Mac/Linux [SM1].
- **Mechanics:** line/hunk staging with hunk splitting, drag hunk edges for context, find-as-you-type
  search across message/author/path/content, blame, merge tool [SM1].
- **LFS:** smudge/clean support since 2020; bundled git lacks LFS, so system git-lfs is needed; 2024 dev
  build added "track via Files context menu" + LFS icons [SM4].
- **Praise:** speed; best-in-class 3-way merge (HN) [SM3]. **Complaints:** slow update cadence, no
  drag-and-drop rebase (#1194), no diff between branches (#255) [SM2], [SM3].
- Sources: [SM1 home](https://www.sublimemerge.com/), [download](https://www.sublimemerge.com/download) · [SM2 issues](https://github.com/sublimehq/sublime_merge/issues/1194) · [SM3 HN 40862144](https://news.ycombinator.com/item?id=40862144) (source 2024) · [SM4 LFS forum](https://forum.sublimetext.com/t/lfs-support/54016), [dev builds](https://www.sublimemerge.com/dev) · [SM5 licence thread](https://forum.sublimetext.com/t/cost-of-upgrade-text-merge-is-same-as-new/73046)

### Tower
- **Model:** classic + Pro "Stacked Branches" and Graphite workflow [T4].
- **Price:** subscription €59 Basic / €99 Pro / €125 Enterprise per user/year, 30-day trial; macOS +
  Windows only [T1]. Tower 15 Windows; Tower 18 Mac (Oct 2026) adds a `gittower` CLI to open views from
  the terminal [T5].
- **Undo (best in class among classic GUIs):** Ctrl+Z / Shift+Ctrl+Z for delete file, stage/unstage,
  discard hunk/file, commit (changes return to working copy), create/checkout/delete branch, merge,
  rebase incl. interactive, publish branch, stash save/delete/apply [T2].
- **Drag-and-drop:** branch→HEAD = merge (Alt = rebase); commits→HEAD = cherry-pick (Alt = revert);
  commit onto commit = squash (Alt = fixup); reorder in history; branch→remote = push; files→Stashes
  [T3]. Confirm dialog states the edit "can be undone using the Reflog" (shot).
- **LFS:** yes; **file locking: no**; image diff: no; conflict wizard, Quick Actions, reflog,
  worktrees, AI commit: yes [T4].
- Sources: [T1 pricing](https://www.git-tower.com/pricing) · [T2 undo](https://www.git-tower.com/features/undo) · [T3 drag-drop](https://www.git-tower.com/features/drag-and-drop) · [T4 all features](https://www.git-tower.com/features/all-features), [features](https://www.git-tower.com/features) · [T5 release notes](https://www.git-tower.com/release-notes/windows)

### GitHub Desktop
- **Model:** deliberately simple; no graph. Free, MIT, Electron/TypeScript; Win + Mac [D1], [M1].
- **2025–26:** Copilot commit messages and **Copilot conflict resolution**, multiple accounts side by
  side, push-after-commit, worktree warnings (3.6.x) [D1].
- **History ops:** amend, undo (unpushed), reset-to-commit, revert, cherry-pick, reorder, squash; no
  full interactive rebase [D2].
- **LFS:** bundled git-lfs, but setup is on the CLI; no locking UI [D4].
- **Complaints (👍):** Linux 4,853, GPG signing 340, history graph 295, multiple windows 228, stash
  specific files 205, multiple stashes 132 [D3]. HN users praise it for amend/cherry-pick-lines ease [F4].
- Sources: [D1 releases](https://github.com/desktop/desktop/releases) · [D2 docs](https://docs.github.com/en/desktop/managing-commits/options-for-managing-commits-in-github-desktop) · [D3 top issues](https://github.com/desktop/desktop/issues/1525) · [D4 LFS docs](https://docs.github.com/en/desktop/configuring-and-customizing-github-desktop/about-git-large-file-storage-and-github-desktop)

### SourceGit
- **Model:** classic, Fork-like layout. MIT, C#/.NET 10 + Avalonia; Win/Mac/Linux; 6.6k ★, release
  v2026.24 (2026-11-09) [SG1], [M1].
- **Mechanics:** graph, interactive rebase, worktrees, GitFlow, **Git LFS**, bisect, image diff
  (side-by-side/swipe/blend), git command log, custom actions, PR creation on GitHub/GitLab/Gitea/
  Bitbucket, AI commit messages via OpenAI-compatible API, conventional-commit helper [SG1].
- **Complaints:** maintainer asking for volunteers (#2722, 47 👍), no auto-updater, can't edit text in
  the merge window (#2168), no custom shortcuts (#2655), no uncommitted/stash nodes in graph (#1673);
  status checks of unopened repos saturate HDD on Linux (#2891) [SG3].
- Sources: [SG1 README](https://github.com/sourcegit-org/sourcegit) · [SG3 issues](https://github.com/sourcegit-org/sourcegit/issues/2722)

### GitButler
- **Model:** **parallel (virtual) branches** — all applied branches live in one working dir; Git sees
  a synthetic merge commit on `gitbutler/workspace`; you choose which branch each hunk goes to;
  `but unapply` hides a branch [GB1]. **Stacked branches** for dependent work [GB1], [GB6].
- **Licence/price:** FSL-1.1-MIT (fair source, converts to MIT later) [GB4]; free app; [$17M Series A, Apr
  2026](https://blog.gitbutler.com/series-a). Stack: Rust + Tauri + Svelte (repo languages) [M1]. Desktop + `but` CLI + TUI on one state [GB1].
- **Undo:** operations history — snapshot of branch state, uncommitted work and conflict state stored in
  the Git object DB **before every major action**; hover an entry → Revert; file snapshots viewable [GB3].
- **Conflicts:** rebase never stops: conflicted commits are stored as "conflicted", marked in UI,
  resolved later one at a time in an edit mode that checks out only that commit [GB2].
- **Agents:** positioned as "Version Control for your Agents"; `but agent setup` installs a Claude Code
  skill; `but commit <file/hunk ids>`, `but amend`, `but squash`, `but uncommit`, `but oplog`, `but pr`;
  each agent commits to its own branch, stacking when dependent; vendor claims 60 % faster / 80 % fewer
  tool calls than vanilla git (vcbench) [GB6].
- **Gaps (👍):** line-level moves 51, **Git LFS 38 (open since 2024-02)**, VS Code extension 37, Azure
  DevOps 32, revert commits 17, worktrees/bare repos 15 [GB5]. `fact`: no LFS = unusable for Max's
  Unity repos today.
- Sources: [GB1 docs](https://docs.gitbutler.com/), [parallel branches](https://docs.gitbutler.com/parallel-branches) · [GB2 conflicts](https://docs.gitbutler.com/features/branch-management/merging) · [GB3 timeline](https://docs.gitbutler.com/features/timeline) · [GB4 LICENSE](https://github.com/gitbutlerapp/gitbutler/blob/master/LICENSE.md) · [GB5 issues](https://github.com/gitbutlerapp/gitbutler/issues/2595) · [GB6 agents](https://docs.gitbutler.com/ai-agents/overview), [home](https://gitbutler.com/)

### SmartGit
- **Model:** classic; three window styles (Standard workflow-oriented, Log graph, Working Tree
  file-oriented) chosen at setup [SmG2].
- **Price:** commercial from $5/user/mo or perpetual; free Hobby / Education / Open-Source licences [SmG3].
- **Stack:** Java/SWT (long-standing; not stated on the new site, `open`). Win/Mac/Linux [SmG1].
- **Mechanics:** Conflict Solver (side-by-side 3-way, one-click take left/right), split/squash/reorder
  ("Clean commits"), guided Git Flow / GitHub Flow, Distributed Reviews add-on (offline review), LFS
  with faster refresh in 25.1 [SmG1], [SmG2]. An agent-branch screenshot names Claude Code; no AI
  feature text [SmG1].
- Sources: [SmG1 home](https://www.smartgit.dev/) · [SmG2 features](https://www.smartgit.dev/features/) · [SmG3 pricing](https://www.smartgit.dev/pricing/)

### Sourcetree (baseline Max uses)
- **Model:** classic. Free, Atlassian, Bitbucket-flavoured; Win 3.4.x / Mac 4.2.x [ST1]. Git + Hg.
- **Status:** maintenance mode in practice: 2025–26 Windows releases are mostly fixes (3.4.25 Git 2.51
  + LFS 3.7; 3.4.27 shortcuts for custom actions, branch filter; 3.4.28 expired signing cert; 3.4.29
  "stuck loading commits"; 3.4.30 history not refreshing) [ST2].
- **Perf (Max's pain 1):** long record of Windows slowness: per-file LFS operations during clone vs one
  batched `git lfs fetch` on Mac; staging hangs while embedded git-lfs.exe pegs CPU (SRCTREEWIN-6667);
  2025 clone stalls on 3.4.25; a Windows 11 update hang [ST3].
- **Praise:** still loved for "pleasant" everyday use (top HN comment in a 2025 lazygit thread) [F4].
- Sources: [ST1 home](https://www.sourcetreeapp.com/) · [ST2 3.4.30 notes](https://product-downloads.atlassian.com/software/sourcetree/windows/ga/ReleaseNotes_3.4.30.html), [3.4.27](https://product-downloads.atlassian.com/software/sourcetree/windows/ga/ReleaseNotes_3.4.27.html) · [ST3 SRCTREEWIN-6667](https://jira.atlassian.com/browse/SRCTREEWIN-6667), [LFS clone thread](https://community.atlassian.com/t5/Sourcetree-questions/Clone-for-large-LFS-project-very-slow-on-windows/qaq-p/681959) (older), [2025 clone thread](https://community.atlassian.com/forums/Sourcetree-questions/Cloning-a-fresh-repo-taking-a-long-time/qaq-p/3115353), [KB5064401 hang](https://jira.atlassian.com/browse/SRCTREEWIN-14595)

### lazygit
- **Model:** TUI, numbered panels (Status/Files/Branches/Commits/Stash), single-key actions, command log
  panel. Go, MIT, 84k ★, v0.66.3 [L1]. Founder handed over to co-maintainer Stefan Haller, June 2025 [L2].
- **Mechanics:** `space` stages hunk, `a` line mode, `v` range; `i` interactive rebase with
  s/f/d/e and ctrl+j/k moves, also on single commits without starting a rebase; **custom patches**
  (pick hunks out of an old commit → move to new commit); `z`/`Z` undo/redo via reflog (not working
  tree or stash); `/` filter everywhere; bisect; worktrees; custom command keybindings [L1].
- **Wants (👍):** full-screen diff 44, stacked PRs 43 (#2527), git-based log search 41, GPG flow 39 [L2].
  HN: praised for fast everyday git-fu; a tip to disable force-push because it is easy to fat-finger [F4].
- Sources: [L1 README](https://github.com/jesseduffield/lazygit) · [L2 issues](https://github.com/jesseduffield/lazygit/issues/4655), [#2527](https://github.com/jesseduffield/lazygit/issues/2527)

### Magit
- **Model:** Emacs text UI where "every bit of visible information is also actionable"; auto-refresh;
  shows the git commands it runs; transient (popup) menus with mnemonic keys [M2]. GPL, 7.3k ★.
- Benchmark for discoverable keyboard menus, not a competitor for Max.
- Sources: [M2 magit.vc](https://magit.vc/) · M1

### VS Code built-in (+ GitLens)
- Line staging, Source Control Graph (incoming/outgoing), 3-way merge editor, worktrees, stashes,
  timeline (git + local saves), blame [VSC1]. Merge editor: Incoming / Current / **Result** panes,
  per-conflict "Accept Incoming / Accept Combination", "N conflicts remaining", Complete Merge;
  experimental AI conflict resolution [VSC2]. No interactive-rebase UI (CLI).
- GitLens (GitKraken-owned, ~10k ★) adds graph/blame; free with Copilot for AI, GitKraken account for
  its own AI [GK6].
- HN: "fast enough for common workflows" sentiment; abandoned "Git Graph" extension missed [F4], [HN 45645120](https://news.ycombinator.com/item?id=45645120).
- Sources: [VSC1 overview](https://code.visualstudio.com/docs/sourcecontrol/overview) · [VSC2 merge conflicts](https://code.visualstudio.com/docs/sourcecontrol/merge-conflicts)

### JetBrains IDEs (built-in git)
- Changelists, Shelf, Git log graph, worktrees, GPG signing [JB1]. History edits from the log: reword,
  amend, fixup, squash-into, squash commits, drop, **extract selected changes to a separate commit** [JB2].
- **Local History:** IDE-side snapshots of every file edit (also external changes), labels on test
  runs/commits, 5 working days default, restores deleted files [JB3].
- **Conflicts:** dialog lists all conflicted files with **Accept Yours / Accept Theirs per file**,
  "Resolve All Simple Conflicts", then a 3-pane editor (yours | result from base | theirs) [JB4].
  Unity devs on Rider get this UI; whether it calls UnityYAMLMerge is track B's question.
- Sources: [JB1 git docs](https://www.jetbrains.com/help/idea/using-git-integration.html) · [JB2 edit history](https://www.jetbrains.com/help/idea/edit-project-history.html) · [JB3 Local History](https://www.jetbrains.com/help/idea/local-history.html) · [JB4 resolve conflicts](https://www.jetbrains.com/help/idea/resolve-conflicts.html)

### Newcomers 2025–26
| Tool | What | Why it matters to gitgud | Source |
|---|---|---|---|
| **GitComet** (Jan 2026) | Rust + GPUI, **uses `gix`**, AGPL free + paid Pro (PR review, forges, AI). Claims responsiveness on Chromium-size repos (no numbers). **LFS status, download, locks, tracking** + git-annex; 2/3-way merge usable as `git mergetool`; worktrees, embedded terminal | Closest open-source peer to gitgud's stack and game-dev wish list; watch for LFS locking UX | [GitHub](https://github.com/Auto-Explore/GitComet) |
| **GG** (jj GUI) | **Tauri + Rust + Svelte** (same stack as gitgud), Apache-2.0. Drag revisions to rebase, drag files to squash, drag bookmarks; op-log Undo button; `gg web` mode. "What if you were always in the middle of an interactive rebase" | Proof the stack works; mechanics map to pain 3 | [GitHub](https://github.com/gulbanana/gg) |
| jjui / lazyjj | TUIs for jj, 2.2k / 1.2k ★ | jj ideas (op log, conflicts as data) spreading | [HN 44093334](https://news.ycombinator.com/item?id=44093334) |
| Zed git panel | Hunk staging with "stage and next", Uncommit (soft reset), AI commit message with own model + instructions, per-conflict keep ours/theirs/both | Editor built-ins keep rising | [Zed docs](https://zed.dev/docs/git) |
| Graphite | Stacked-PR CLI/service, bought by Cursor Dec 2025 | Stacks are mainstream; gitgud stays host-agnostic | [TechCrunch](https://techcrunch.com/2025/12/19/cursor-continues-acquisition-spree-with-graphite-deal) |
| Tauri hobby clients (Gitnado/Leviathan, GitLite, Gitmun, l8git, ArezGit) | Many tiny 2025–26 Tauri git GUIs, <50 ★ each | Crowded "fast Tauri git GUI" niche; gitgud's edge must be game-dev + agents, not speed alone `Claude's pick` | [Gitnado](https://github.com/hegsie/Leviathan), [search](https://github.com/cst8t/gitmun) |
| Gitnuro, Gittyup, Git Extensions | Kotlin Compose / Qt / WinForms OSS clients, 2–9k ★ | Steady but not growing | GitHub API |

## 4. Steal this mechanic

Ranked by fit with Max's section-0 answers (game-dev, Windows, any host, opt-in AI + agent review).
Pains: **P1** slow big/LFS · **P2** Unity merge conflicts · **P3** scary history edits · **P4** clunky UI.
All rankings `Claude's pick`.

| # | Mechanic | From | Why | Pains |
|---|---|---|---|---|
| 1 | **Snapshot op log before every mutating action** (refs + index + working tree + conflict state into the git ODB), list with per-entry Revert | GitButler [GB3], GG/jj | Makes every action reversible, including discards; pure git, no extra store | P3 |
| 2 | **Global Ctrl+Z / Ctrl+Shift+Z** over that log, scoped to stage/discard/commit/branch/merge/rebase/stash | Tower [T2] (GitKraken only does the last action [GK2]) | Muscle memory beats a history panel for the common case | P3, P4 |
| 3 | **Per-file "take mine / take theirs" conflict list + "resolve all simple"** before any text merge | JetBrains [JB4] | For prefabs, scenes and binaries the honest answer is usually whole-file; Unity YAML smart-merge plugs in here (track B) | P2 |
| 4 | **Rebase that doesn't stop**: conflicted commits recorded and resolved later one by one | GitButler [GB2], jj | Removes the "stuck mid-rebase" fear; Fork's **conflict preview before rebasing** [F2] is the cheap first step | P2, P3 |
| 5 | **LFS as first-class**: lock/unlock from the file list, lock owner shown, LFS badge per file, batched background `git lfs` (never per-file) | GitComet [GC1], Fork [F3]; anti-pattern Sourcetree [ST3] | Locks prevent unmergeable asset conflicts; batching is exactly Max's Sourcetree slowness | P1, P2 |
| 6 | **Drag to rewrite** (commit onto commit = squash, Alt = fixup; drag to reorder; drag commit to HEAD = cherry-pick) with a confirm that says how to undo | Tower [T3], GG, Fork 2.55 [F2] | Direct manipulation for history edits, made safe by #1–2 | P3, P4 |
| 7 | **Agent work as reviewable units**: each agent's changes as its own branch/WIP node, commit by hunk IDs, range diff, revert hunk, drop session | GitButler `but` [GB6], GitKraken agents [GK5] | Direct template for gitgud's CLI/MCP + review screen (track C) | answer 5, P4 |
| 8 | **AI through the user's already-installed agent CLI** (Claude Code/Codex) instead of API keys | Fork 2.59–2.70 [F2] | Opt-in, no key management, no vendor lock | answer 5 |
| 9 | **Command palette + single-key verbs + visible command log** | Fork Quick Launch [F3], Sublime Merge [SM1], lazygit [L1], Magit [M2] | Speed plus trust: you see the exact git command run | P4, P3 |
| 10 | **Hunk/line staging with "stage and next"**, custom patch (lift hunks out of an old commit) | Zed, lazygit [L1], Sublime Merge [SM1] | Fast commit composition; lazygit's custom patch replaces most interactive rebases | P4, P3 |
| 11 | **Uncommitted changes and stashes as nodes in the graph** | GitKraken [GK1], GitComet; top request in Fork (#308) and SourceGit (#1673) | Where you are, at a glance | P4 |
| 12 | **Image diff modes** (side-by-side / swipe / onion-skin) extended to textures | SourceGit [SG1], Fork [F3] | Base for game-asset previews (answer 4) | P2, P4 |
| 13 | **Extract selected changes to a new commit** from the log | JetBrains [JB2] | Splitting commits without an interactive rebase | P3 |
| 14 | **Custom actions with shortcuts** | lazygit [L1], SourceGit [SG1], Sourcetree 3.4.27 [ST2] | Escape hatch for Unity-specific scripts | P4 |

Avoid (`Claude's pick`): GitKraken-style account wall and AI-credit tiers; GitButler's single-folder
virtual-branch model as the default (no LFS, confuses other tools; good ideas, wrong base for Unity
repos); Electron footprint for a tool that sits open all day.

## Game-dev tools

Summary of [gamedev.md](gamedev.md) (track B, links checked 2026-11-21).

- **Git is the game-dev default below AAA** (fact): GitHub used by 61% of mid/indie and 79% of hobbyist
  respondents; Perforce 45% at AAA but 10% mid/indie; Unity VCS 9% / 7%
  ([JetBrains 2025](https://lp.jetbrains.com/the-state-of-gamedev-2025/)).
- **Anchorpoint**: Git client for game teams: auto-lock, previews, Unity plugin, but its own lock server rather
  than LFS locks ([docs](https://docs.anchorpoint.app/git/file-locking/)); now rewriting as a **Lore** client and
  dropping Git for now ([docs](https://docs.anchorpoint.app/lore/quick-start/)) (fact).
- **Unity VCS (Plastic)**: smart locks (lock only on latest), Gluon partial workspaces for artists; cloud seat
  fees removed 2026-03-01 ([Unity](https://unity.com/en/products/pricing-updates)) (fact).
- **Perforce P4V**: exclusive checkout and streams; still the AAA norm (fact, see survey above).
- **Diversion**, **Lore** (Epic, open-sourced June 2026, Rust, MIT, advisory locks, no Git interop on roadmap,
  [repo](https://github.com/EpicGames/lore)) (fact).
- **LFS locks in Git GUIs** (fact): API complete (`--json`, `/locks/verify`)
  ([API](https://github.com/git-lfs/git-lfs/blob/main/docs/api/locking.md)); Sourcetree none, GitKraken a feature
  request, Fork a manual list ([#1862](https://github.com/fork-dev/TrackerWin/issues/1862)); "unlock after push"
  open since 2017 ([#2671](https://github.com/git-lfs/git-lfs/issues/2671)).
- **UnityYAMLMerge**: documented only as a manual `git mergetool`; merges `.unity`/`.prefab`/`.meta` only; known
  silent-merge bugs on prefab instances/variants
  ([Smart merge](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge)) (fact).
- **Unity 6.3 editor integration** covers Perforce + UVCS only; Git locking needs an editor plugin
  ([integrations](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)) (fact).
- **Claude's pick**: be the best *Git* client for Unity: native LFS lock panel + unlock-on-push checklist,
  `.meta` guard, UnityYAMLMerge as an auto-configured driver, structural prefab diff, lazy binary previews.
  Proposals T1–T14 in gamedev.md §6.

## Agent interfaces

Summary of [agents.md](agents.md) (track C, links checked 2026-11-21).

- **Git MCP servers mirror the CLI, none has a review gate** (fact). Official `mcp-server-git`: 14 tools +
  2 prompts, file-level `git_add` only, commits straight to the branch, disables hooks/signing/repo drivers
  ([README](https://github.com/modelcontextprotocol/servers/tree/main/src/git)). GitHub MCP is host-API only
  ([repo](https://github.com/github/github-mcp-server), 34.6k★). cyanheads/git-mcp-server mirrors ~30 git verbs (245★).
- **GitButler is the closest prior art** (fact): `but agent setup` installs a skill + policy; agents commit
  **hunks by ID**, amend/squash/uncommit, `but oplog` undo; parallel agents share one workdir
  ([docs](https://docs.gitbutler.com/ai-agents/overview)). Its MCP and Claude Code hooks doc pages now 404 (open).
- **Agents today** (fact): Claude Code, Codex, Cursor isolate in worktrees and review via a diff with line
  comments fed back to the agent; Codex's review pane has Unstaged/Staged/Commit/Branch/Last-turn scopes and chunk
  staging ([Codex](https://developers.openai.com/codex/app/review)); Aider auto-commits every edit
  ([Aider](https://aider.chat/docs/git.html)); Copilot cloud agent ends in a host PR whose workflows need human approval.
- **Orchestrators** (Conductor, Sculptor, Crystal→Nimbalyst, vibe-kanban, the latter now sunsetting) all follow
  worktree → diff review → PR. Nobody but GitButler helps split agent output into clean commits locally.
- **Scale** (fact): agents start >40% of GitHub PRs (Sept 2026)
  ([Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/));
  58.7% of developers don't plan to use AI for committing/reviewing ([SO 2025](https://survey.stackoverflow.co/2025/ai)).
- **Claude's pick for gitgud:** agents call `propose(hunks, message, why)`; gitgud builds draft commits under
  hidden refs (`refs/gitgud/proposals/<id>`) without touching Max's index or branches, adds risk flags (LFS,
  `.meta` pairs, unlocked lockable files, out-of-scope files), and Max approves/edits/rejects in an inbox. Approval
  commits via the `git` CLI so hooks and signing run. v1 tools: `status`, `diff`/`log`/`show`, `propose`,
  `proposal`, `withdraw`; no commit/push/merge tools. Alternatives (agent stages real index; agent branch/worktree
  review; virtual branches) compared in agents.md §6.
