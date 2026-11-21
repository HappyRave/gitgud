# How developers use Git today (track C, section 2)

Research, 2026-11-21. All links checked 2026-11-21 unless marked otherwise. Labels: **fact** (sourced),
**Claude's pick**, **open**. Nothing here is decided. Screenshots: `shots/workflows.json` (prefix `wf-`).

Max's four pains, used as tags below: **P1** slow on big/LFS repos · **P2** Unity merge conflicts ·
**P3** scary history edits · **P4** clunky UI/workflow.

## 0. Ten-line summary

1. Trunk-based + short-lived branches + PR is the default; GitFlow is called "legacy" by Atlassian itself. fact
2. Stacked PRs went mainstream in 2026: GitHub shipped them natively (preview 2026-07-30, GA 2026-10-06); Cursor bought Graphite (2025-12). fact
3. Git itself is catching up with jj/GitButler-style history editing: `git history reword|split` (2.54), `fixup` (2.55), `drop` (2.56); all refuse to run into conflicts and rewrite descendant branches. fact
4. Worktrees went from niche to normal because of parallel AI agents; their pains are setup cost, disk, cleanup, LFS. fact
5. Agents now start >40% of GitHub PRs (Sept 2026, up from 17% in March). Review, not writing, is the bottleneck. fact
6. Big-repo Git keeps getting better server-side (geometric maintenance default, `backfill`, `last-modified`, path-walk packs, fsmonitor on Linux), but a GUI has to opt into it. fact
7. Unity game dev still needs three things Git doesn't give by default: LFS locking that actually works, `UnityYAMLMerge`, and binary previews. fact
8. jj ideas (op log undo, non-blocking conflicts, auto-rebase of descendants, change IDs) are the reference for "safe history edits". fact
9. gitgud's opening: make history edits and agent output **previewable and undoable**, and make big/LFS repos fast by default. Claude's pick
10. Least useful to chase: GitFlow tooling, submodule UX polish, conventional-commit linters. Claude's pick

## 1. Workflows as step sequences

⚠ = painful step today, with why and source. Tags say which of Max's pains it hits.

### W1. Trunk-based: short-lived branch + PR (the default)

fact: Atlassian calls trunk-based "a required practice for CI" and Gitflow "a legacy Git workflow"
([trunk-based](https://www.atlassian.com/continuous-delivery/continuous-integration/trunk-based-development),
[Gitflow](https://www.atlassian.com/git/tutorials/comparing-workflows/gitflow-workflow)). Octoverse 2025: commits
are getting smaller, PRs shorter and single-purpose, feature flags ship unfinished work
([GitHub blog, 2025-11-07](https://github.blog/news-insights/octoverse/what-986-million-code-pushes-say-about-the-developer-workflow-in-2025/)).

1. `git switch -c feat/x` from fresh `main` (needs a fetch first).
2. Edit; stage hunks; commit small. ⚠ **P4** hunk staging in IDEs and Sourcetree is click-heavy; `git add -p` is terminal-only. Git 2.54 added hunk-decision status to `add -p` because reviewing your own staging choices was hard ([2.54](https://github.blog/open-source/git/highlights-from-git-2-54/)).
3. Push, open PR in browser.
4. Address review: more commits or fixups (see W3).
5. `main` moved: rebase or merge `main` in. ⚠ **P2/P3** conflicts mid-rebase leave a half-applied state; with Unity YAML they're unreadable (W6).
6. Squash-merge on host; delete local branch. ⚠ **P4** stale local branches pile up; Git 2.56 added `git branch --delete-merged … --dry-run` exactly for this ([2.56](https://github.blog/open-source/git/highlights-from-git-2-56/)).

### W2. GitFlow (legacy, still in release-based shops)

1. `develop` + `feature/*` → `release/*` → `main` + tag → `hotfix/*` back-merged twice.
2. ⚠ **P4** long-lived branches, larger commits, double back-merges ([Atlassian](https://www.atlassian.com/git/tutorials/comparing-workflows/gitflow-workflow)). Common in game studios with milestone builds (open: no 2025+ data found).
- Claude's pick: no GitFlow wizard. Branch-prefix folders in the sidebar are enough.

### W3. Stacked PRs

fact: GitHub stacked PRs: public preview 2026-07-30, GA 2026-10-06, created with the `gh stack` extension; merging a
lower layer auto-rebases and retargets the ones above; same-repo only; **not supported in GitHub Desktop**
([GA changelog](https://github.blog/changelog/2026-10-06-stacked-pull-requests-generally-available),
[docs](https://docs.github.com/en/pull-requests/get-started/about-stacked-prs)). 264k developers used them by
September ([Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/)).
Graphite (`gt create/modify/restack/sync/submit`) is the incumbent ([docs](https://graphite.com/docs/cli-quick-start)); Cursor agreed to buy it 2025-12-19
([Cursor](https://cursor.com/blog/graphite), [TechCrunch](https://techcrunch.com/2025/12/19/cursor-continues-acquisition-spree-with-graphite-deal)).
Other stackers: GitButler stacks, Sapling (meta/sapling, 7.3k★), ghstack (1.0k★), plain `git rebase --update-refs`
([git-rebase](https://git-scm.com/docs/git-rebase)). Stars via GitHub API, 2026-11-21.

1. Branch A off `main`, commit; branch B off A, commit; branch C off B.
2. Open PRs: A→main, B→A, C→B.
3. Reviewer asks for a change in A. Amend A. ⚠ **P3** B and C now sit on the old A. Plain Git needs `rebase --update-refs` (doesn't move branches checked out in other worktrees) or a per-branch rebase ([git-rebase](https://git-scm.com/docs/git-rebase)).
4. Force-push all three. ⚠ **P3** force-push fear; easy to push the wrong set.
5. A merges (often squash). ⚠ **P3** B must be rebased onto new `main` with the squashed commits dropped. HN users report `gh stack rebase` struggling after squash merges and whole-stack merge "broken in many cases" during preview ([HN, 783 pts, 2026-07-30](https://news.ycombinator.com/item?id=49112232)).
6. Repeat up the stack.
- HN sentiment (paraphrased): stacks are mostly welcomed as a way to cut giant (often AI-written) PRs into reviewable layers; a minority argues curated commits reviewed one by one do the same ([same thread](https://news.ycombinator.com/item?id=49112232)).

### W4. Cleaning up history before review (fixup / autosquash / absorb / `git history`)

1. Spot a bug in commit 3 of 5.
2. `git commit --fixup=<sha>` (needs the SHA) ⚠ **P4** finding the SHA. `git absorb` picks the target commit per hunk by commutation, port of `hg absorb`, 5.8k★ ([repo](https://github.com/tummychow/git-absorb)).
3. `git rebase -i --autosquash main` ⚠ **P3** todo-list editing in a text editor; a conflict leaves you mid-rebase ([git-rebase](https://git-scm.com/docs/git-rebase), [2.54 post](https://github.blog/open-source/git/highlights-from-git-2-54/)).
4. Reword an old message / split a commit. Git now has targeted commands: `git history reword` and `split` (2.54), `fixup` (2.55), `drop` (2.56). They don't touch the working tree, **refuse to run when a conflict would result**, don't handle merge commits, and rewrite every local branch descended from the commit ([2.54](https://github.blog/open-source/git/highlights-from-git-2-54/), [2.55](https://github.blog/open-source/git/highlights-from-git-2-55/), [2.56](https://github.blog/open-source/git/highlights-from-git-2-56/)). Experimental.
5. Force-push. ⚠ **P3** undo is the reflog, which few people read.
- HN (paraphrased, [457 pts, 2026-07-14](https://news.ycombinator.com/item?id=48901010)): people like the low-friction rewrites; one user notes `git history` can't sign the commits it rewrites (unverified, open); a self-described game dev says rewriting history "gives me the willies" and prefers new commits.
- Claude's pick: gitgud runs every history edit as a **preview first** (new graph shown, conflicts predicted via `git replay`/`merge-tree` style dry run), then one-click undo from its own op log.

### W5. Parallel work: worktrees (now: one agent per worktree)

fact: GitHub says worktrees gained popularity because AI pushed developers to work in parallel; drawbacks are
per-worktree dependency installs (disk), manual cleanup, and one branch per worktree; the GitHub Copilot app opens new
sessions in a worktree by default ([GitHub blog, 2026-06-16](https://github.blog/ai-and-ml/github-copilot/what-are-git-worktrees-and-why-should-i-use-them/)).
Claude Code (`--worktree`, `.claude/worktrees/<name>`, branch `worktree-<name>`), Cursor (`.cursor/worktrees.json`, auto-delete beyond 25),
Codex app (detached HEAD worktrees, deleted with the thread) all default to it
([Claude Code](https://code.claude.com/docs/en/worktrees), [Cursor](https://cursor.com/docs/configuration/worktrees), [Codex](https://developers.openai.com/codex/app/worktrees)).

1. `git worktree add ../repo-task -b task`.
2. Copy ignored setup files (`.env`) in. ⚠ **P4** every tool invented its own: `.worktreeinclude` (Claude Code, Codex), `worktrees.json` (Cursor).
3. Install deps / import project. ⚠ **P1** Trigger.dev: per-worktree Postgres/Redis/dev server, port clashes, ~10 GB `node_modules` per copy; setup cost exceeded the work ([2026-04-16](https://trigger.dev/blog/parallel-agents-gitbutler)). For Unity, a fresh worktree means a full `Library/` re-import (open: no measured source; Claude's estimate is minutes to hours on a big project).
4. LFS content. ⚠ **P1** Claude Code worktrees get LFS *pointer files* if LFS was installed with `--local`, because it skips repo-defined filter drivers for safety ([docs](https://code.claude.com/docs/en/worktrees)).
5. Work, commit, merge back. ⚠ **P4** each worktree is invisible to the GUI you have open on the main checkout.
6. Clean up. ⚠ **P4** locked/stale worktrees, sweeps with different rules per tool ([Claude Code](https://code.claude.com/docs/en/worktrees), [Cursor](https://cursor.com/docs/configuration/worktrees)).
- Alternative: GitButler parallel branches in one working directory (no copy, shared runtime; breaks when two agents touch the same file) ([GitButler](https://docs.gitbutler.com/ai-agents/parallel-agents)); jj workspaces ([jj](https://docs.jj-vcs.dev/latest/operation-log/)).

### W6. Unity project with LFS and locking (Max's daily case)

1. Clone; `git lfs pull`. ⚠ **P1** slow, and GUIs re-scan everything.
2. Before editing a scene/prefab: `git lfs lock Assets/X.prefab`. ⚠ **P4** only works if `.gitattributes` marks it `lockable`; few LFS servers implement locking; when locks can't be verified the push isn't halted ([git-lfs wiki, edited 2025-10-04](https://github.com/git-lfs/git-lfs/wiki/File-Locking)).
3. Edit in Unity; `.meta` files change alongside. ⚠ **P4** missing/orphan `.meta` is a classic Unity breakage (open: needs a 2025+ source; other track).
4. Pull teammate's changes → conflict in `.unity`/`.prefab` YAML. ⚠ **P2** Unity's `UnityYAMLMerge` is documented only as a `git mergetool`, not as a merge driver; `.asset`, `.mat`, `.controller` go to the fallback tool unless `--force` ([Unity 6 manual](https://docs.unity.com/en-us/engine/6000.0/manual/get-started/project-configuration/version-control/smart-merge)).
5. Commit, push, `git lfs unlock`. ⚠ **P4** forgotten locks block teammates; `--force` unlock rights vary by server ([wiki](https://github.com/git-lfs/git-lfs/wiki/File-Locking)).
- Note: the official MCP git server empties repo-defined merge drivers, so an agent merge that relies on a repo-level `UnityYAMLMerge` driver would just conflict ([README](https://github.com/modelcontextprotocol/servers/tree/main/src/git)).

### W7. Big repo / monorepo

fact: Scalar clones as partial clone + sparse checkout + background maintenance ([scalar](https://git-scm.com/docs/scalar)).
Recent Git: `git backfill` batches missing blobs in partial clones (2.49; ranges/pathspecs 2.54); `git last-modified`
gives per-entry last commit for a directory ~5.5× faster than `ls-tree`+`log` (2.52); geometric repacking default for
`git maintenance` (2.54); `core.fsmonitor` on Linux via inotify (2.55); `repack --filter=blob:limit --drop-filtered`
to evict local large blobs (2.56); reftable becomes the default in Git 3.0
([2.49](https://github.blog/open-source/git/highlights-from-git-2-49/), [2.51](https://github.blog/open-source/git/highlights-from-git-2-51/),
[2.52](https://github.blog/open-source/git/highlights-from-git-2-52/), [2.54](https://github.blog/open-source/git/highlights-from-git-2-54/),
[2.55](https://github.blog/open-source/git/highlights-from-git-2-55/), [2.56](https://github.blog/open-source/git/highlights-from-git-2-56/)).

1. Clone (full, or partial/sparse if you know the flags). ⚠ **P1** most GUIs don't offer partial/sparse at clone time (open: confirm in landscape track).
2. Daily `status` / `log` / file history. ⚠ **P1** without fsmonitor + commit-graph + maintenance these are slow; GUIs poll.
3. Blame or file browser in a partial clone. ⚠ **P1** one blob fetched at a time unless `backfill` runs first ([2.49](https://github.blog/open-source/git/highlights-from-git-2-49/)).
4. Housekeeping. ⚠ **P1** nobody runs `git maintenance start` by hand.
- Claude's pick: gitgud turns on `maintenance`, fsmonitor and commit-graph for a repo on first open (with one confirm), and uses `last-modified`-style data for the file tree.

### W8. Commit hygiene: signing and conventional commits

1. Configure SSH signing (`gpg.format=ssh`, `user.signingkey`). GitHub verifies GPG, SSH and S/MIME; vigilant mode marks unsigned commits "Unverified" ([GitHub docs](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification)). ⚠ **P4** setup is three config keys plus uploading the key; tools that rewrite history (rebase, `git history` per an HN user) can drop signatures.
2. Write a message in Conventional Commits form if the team uses it. ⚠ **P4** contested: an HN thread argues it focuses on the wrong things ([372 pts, 2026-06-05](https://news.ycombinator.com/item?id=48414027)). Aider defaults its AI messages to Conventional Commits ([aider](https://aider.chat/docs/git.html)).
- Claude's pick: show signature status per commit; respect `commit.gpgSign` in every write (CLI does it for free). No conventional-commit linter in MVP.

### W9. Submodules

1. Clone with `--recurse-submodules`, or forget and get empty folders. ⚠ **P4**
2. Update pointer, commit in two repos in the right order. ⚠ **P4/P3** detached HEAD inside the submodule.
- Little new in 2025–26 (Git 2.51: `cat-file` reports submodule entries ([2.51](https://github.blog/open-source/git/highlights-from-git-2-51/))). Claude's pick: show submodule state clearly, nothing more in MVP.

### W10. Undo / recovery

1. Something went wrong (bad rebase, reset, dropped stash).
2. `git reflog`, find the entry, `reset --hard`. ⚠ **P3** per-ref, cryptic, doesn't cover the index or the working tree.
- jj: the operation log records every repo-changing operation as a view (refs, heads, working-copy commit); `jj undo`, `jj op revert`, `jj op restore` ([jj docs](https://docs.jj-vcs.dev/latest/operation-log/)). GitButler has `but oplog` ([docs](https://docs.gitbutler.com/ai-agents/overview)). Git 2.51 added `stash export/import` to move stashes as refs ([2.51](https://github.blog/open-source/git/highlights-from-git-2-51/)).
- Claude's pick: gitgud keeps its own op log (snapshot of refs + index tree + working-tree stash commit before every write) so "Undo" works for every action, agent actions included.

### W11. Agent writes code, human reviews (the new loop)

Detail in [agents.md](agents.md). Short version:

1. Start agent (often in a worktree).
2. Agent edits; some tools auto-commit (Aider commits every edit ([docs](https://aider.chat/docs/git.html))), others leave changes uncommitted (Claude Code, Codex review pane).
3. Human reviews diff in the agent's own UI (Claude Code desktop, Codex review pane, Cursor) or on the host PR. ⚠ **P4** the review happens in the agent tool, not in the Git tool; ⚠ big, multi-file diffs: 87% of agent-started PRs touch multiple files vs 79% of others ([Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/)).
4. Split into commits / PRs. ⚠ **P3/P4** agents produce one blob of changes; splitting is manual.
5. Commit with co-author trailer; push; PR.

### jj ideas leaking into Git tooling (cross-cutting)

| jj idea | Where it shows up in Git land (fact) |
|---|---|
| Op log + undo | GitButler `but oplog`; Git has only reflog ([jj](https://docs.jj-vcs.dev/latest/operation-log/), [GitButler](https://docs.gitbutler.com/ai-agents/overview)) |
| Descendants auto-rebase | `git history` rewrites all descendant local branches; GitHub stacks auto-retarget ([2.54](https://github.blog/open-source/git/highlights-from-git-2-54/), [GitHub](https://docs.github.com/en/pull-requests/get-started/about-stacked-prs)) |
| Edit without touching worktree | `git replay`, `git history` operate without the working tree ([2.54](https://github.blog/open-source/git/highlights-from-git-2-54/), [2.56](https://github.blog/open-source/git/highlights-from-git-2-56/)) |
| First-class (non-blocking) conflicts | Not in Git. `git history` refuses instead; 2.55 `checkout -m` stashes conflicted changes so you can resolve later ([2.55](https://github.blog/open-source/git/highlights-from-git-2-55/)) |
| No staging area / working copy is a commit | GitButler's "uncommitted area" assigned to branches; agent advocates argue it removes forgotten-staging mistakes ([Amplify, 2026-05-20](https://amplifypartners.com/blog-posts/will-agents-like-git-any-more-than-we-do)) |

Interest: HN "jj – the CLI for Jujutsu" 549 pts (2026-04-14), jj repo 32.6k★; jj's creator joined ERSC, a company building a jj backend (HN 265 pts, 2026-09-01)
([HN](https://news.ycombinator.com/item?id=47763759), [HN](https://news.ycombinator.com/item?id=49525297), stars via GitHub API).

## 2. Data

| Source | Finding | Label |
|---|---|---|
| [SO Survey 2025, AI](https://survey.stackoverflow.co/2025/ai) (49k+ resp.) | 84% use or plan to use AI tools; 51% of pros daily. 46% distrust AI accuracy vs 33% trust. Top frustration (66%): "almost right, but not quite". | fact |
| same, `#3-ai-agents` | AI agents at work: 14.1% daily, 9% weekly; 37.9% no plans. | fact |
| same, dev workflow | "Committing and reviewing code": 10.2% mostly AI, 22.6% partially, **58.7% don't plan to** use AI for it. Humans want to keep the commit/review step. | fact |
| [SO Survey 2025, tech](https://survey.stackoverflow.co/2025/technology/) | GitHub used by 81.1%, GitLab 35.6%. No separate Git question this year. IDE list: Cursor 17.9%, Claude Code 9.7%. | fact |
| [JetBrains DevEco 2025](https://devecosystem-2025.jetbrains.com/) via [The Register](https://www.theregister.com/2025/10/21/massive_jetbrains_dev_survey) (24.5k resp.) | 85% use AI coding tools; 19% save 8+ h/week; Cursor raw responses 135 → 2,300+. No Git/VCS questions found. | fact |
| [Octoverse 2025](https://github.blog/news-insights/octoverse/octoverse-a-new-developer-joins-github-every-second-as-ai-leads-typescript-to-1/) | 43.2M merged PRs/month (+23%); smaller, more frequent commits ([pushes post](https://github.blog/news-insights/octoverse/what-986-million-code-pushes-say-about-the-developer-workflow-in-2025/)). | fact |
| [Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/) (2026-10-28) | Agents start >40% of PRs (Sept) vs 17% (March), 110M in Sept. Agent PRs merged within 2 weeks: 83% → 90%; others ~86%. 17.9B pushes, 34.3B commits, 1.3B merged PRs. AI code review usage +90% YoY. Stacked PRs: 264k users by Sept. | fact |
| HN signal | Stacked PRs launch 783 pts; `git history` 457 pts; jj CLI 549 pts; worktrees+tmux for LLMs 156 pts ([HN](https://news.ycombinator.com/item?id=44116872)). | fact |

Read: commit volume tripled in two years and most new PRs start with an agent, but most developers don't want AI to
own committing/reviewing. The human review step is the one to make fast. Claude's pick

### Git release features a GUI should use (2.45 → 2.56)

| Version | Feature | GUI use | Status |
|---|---|---|---|
| 2.44–2.56 | `git replay` (added 2.44, source 2024: [post](https://github.blog/open-source/git/highlights-from-git-2-44/), not re-checked; atomic refs + `--revert` 2.54, `--linearize` 2.56) | Rebase/cherry-pick preview without touching worktree | experimental |
| 2.45 / 3.0 | reftable; default in Git 3.0 | `gix` must read it | fact ([2.51](https://github.blog/open-source/git/highlights-from-git-2-51/)) |
| 2.49 | `git backfill`, `clone --revision`, name-hash v2 | Fast blame/history in partial clones | fact |
| 2.51 | `stash export/import`, `switch`/`restore` stable | Stash sync, safe op-log | fact |
| 2.52 | `git last-modified`, `git repo info/structure`, `sparse-checkout clean` | File tree "last commit" column; repo health panel | fact |
| 2.54 | `git history reword/split`, config-based hooks + `git hook list`, `rebase --trailer`, geometric maintenance default, `status.compareBranches` | Hunk-split UI, hooks panel, trailer stamping | experimental / fact |
| 2.55 | `git history fixup`, fsmonitor on Linux, `checkout -m` autostash, `log --graph-lane-limit` | Fixup-to-commit drag, portable file watching | experimental / fact |
| 2.56 | `git history drop`, `git add --resolved`, `branch --delete-merged --dry-run`, `bisect run --reset-when-found`, faster merge-base | Safe conflict staging, branch cleanup preview | experimental / fact |

All from GitHub's "Highlights from Git" posts linked above. `git history` name verified; "`git last-modified`" verified (2.52).

## 3. Pains gitgud can solve

| Max's pain | Painful step (from §1) | Evidence | What gitgud could do | Label |
|---|---|---|---|---|
| P1 slow big/LFS | W7.2 status/log polling; W6.1 LFS pull; W7.3 blob-at-a-time blame | [2.49](https://github.blog/open-source/git/highlights-from-git-2-49/), [2.55](https://github.blog/open-source/git/highlights-from-git-2-55/) | `gix` reads + file watcher (fsmonitor where present), never poll; enable maintenance/commit-graph on open; `backfill` before blame in partial clones; LFS progress with per-file status | Claude's pick |
| P1 | W5.3–4 worktree setup, LFS pointers | [Trigger.dev](https://trigger.dev/blog/parallel-agents-gitbutler), [Claude Code](https://code.claude.com/docs/en/worktrees) | One worktree panel across all tools' worktrees; "hydrate" action (LFS pull + copy ignored files) | Claude's pick |
| P2 Unity conflicts | W6.4 YAML conflict, mergetool-only | [Unity manual](https://docs.unity.com/en-us/engine/6000.0/manual/get-started/project-configuration/version-control/smart-merge) | Register `UnityYAMLMerge` as merge driver/mergetool from a settings toggle; conflict view that names GameObjects, not YAML lines; `add --resolved` to stage only resolved files ([2.56](https://github.blog/open-source/git/highlights-from-git-2-56/)) | Claude's pick; driver feasibility open (spike) |
| P2 | W6.2/W6.5 locks | [LFS wiki](https://github.com/git-lfs/git-lfs/wiki/File-Locking) | Lock-before-edit prompts on lockable files, lock owner badges in tree, unlock reminder on push | Claude's pick |
| P3 scary history | W3.3–5 restack; W4.3 rebase -i; W10 reflog | [git-rebase](https://git-scm.com/docs/git-rebase), [2.54](https://github.blog/open-source/git/highlights-from-git-2-54/), [jj op log](https://docs.jj-vcs.dev/latest/operation-log/) | Every rewrite: preview graph + predicted conflicts → apply → undo from gitgud op log. Drag hunk onto commit = `history fixup`/absorb. Restack descendants automatically (`--update-refs` semantics) | Claude's pick |
| P3 | W4.5 force-push | HN [thread](https://news.ycombinator.com/item?id=48901010) | Always `--force-with-lease`; show exactly which remote branches change | Claude's pick |
| P4 clunky UI | W1.2 hunk staging; W1.6 stale branches; W5.5 invisible worktrees | [2.54](https://github.blog/open-source/git/highlights-from-git-2-54/), [2.56](https://github.blog/open-source/git/highlights-from-git-2-56/) | Keyboard hunk/line staging; branch cleanup with dry-run list; worktrees as first-class sidebar items | Claude's pick |
| P4 | W11.3–4 agent review in the wrong tool | [Octoverse 2026](https://github.blog/news-insights/octoverse/octoverse-commits-pushes-reach-new-highs-in-2026-as-developers-build-with-agents/), [SO 2025](https://survey.stackoverflow.co/2025/ai) | Agent proposals land in gitgud as reviewable draft commits (see agents.md) | Claude's pick |
| P4 | W3 stacks | [GitHub](https://docs.github.com/en/pull-requests/get-started/about-stacked-prs) | Show stacks in the graph; "open next PR in browser" with the right base. No host API (decided scope) | Claude's pick |

## 4. Open

- Does `UnityYAMLMerge` work as a `merge.<driver>.driver` (non-interactive)? → spike.
- Unity `Library/` re-import cost per worktree: no measured source found.
- Whether `git history` drops signatures: one HN user's claim, not in Git docs.
- JetBrains 2025 AI-delegation figures didn't render in fetch; numbers via The Register only.
