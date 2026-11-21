# gitgud — research and preparation plan

Goal: a spec Max approves (`docs/spec.md`) before any app code. Status: section 0 decided, the rest a **proposal**,
2026-11-21.

## 0. Max's answers (decided 2026-11-21)

1. **Audience:** Max's daily tool first, open source later. Public repo; no onboarding or support
   work until it's good.
2. **Platforms:** Windows now, with portable code (no Windows-only APIs) so macOS and Linux are
   only a packaging job later.
3. **Hosts:** works with any host through plain git, plus light host awareness ("open/create PR in
   browser", PR badges later). No in-app PR APIs.
4. **Game-dev:** full focus. LFS done properly, file locking, previews of binary assets (textures,
   prefabs), tuning for large repos, Unity `.meta` awareness.
5. **AI:** opt-in commit-message drafting. Plus "a MCP or CLI for agent to easily interact and
   prepare the commits for the user": AI agents stage and draft commits, and Max reviews them in
   gitgud.
6. **Today:** Sourcetree and IDE git. Pain points: slow on big/LFS repos, Unity merge conflicts,
   scary history edits, clunky UI/workflow.

## 1. Landscape: how today's Git GUIs work

- **Tools:** GitKraken, Fork, Sublime Merge, Tower, GitHub Desktop, SourceGit, GitButler, SmartGit,
  SourceTree (as a legacy baseline). Also lazygit and Magit (keyboard-driven power-user benchmarks)
  and the IDE built-ins (VS Code, JetBrains), which are the real competition for most developers.
  Research will add any 2025–26 newcomers.
- **Game-dev tools** (from answer 4): Anchorpoint (a Git client for game teams), Unity Version
  Control (Plastic SCM), Perforce P4V, Diversion. Also how Unity's own Smart Merge
  (`UnityYAMLMerge`) and LFS locking are surfaced, or not, in today's GUIs.
- **Agent interfaces** (from answer 5): existing git MCP servers, GitButler's CLI/MCP, and how
  Claude Code and other agents commit today (worktrees, staged hunks, draft messages). What
  should an agent hand to a human for review?
- **Per tool:** features, workflow model (classic branches vs GitButler's virtual/stacked branches),
  pricing and license, platforms, tech stack, big-repo performance, what users praise, what they
  complain about.
- **Sources:** official docs and changelogs, screenshots and demo videos, the most-upvoted open
  feature requests and issues (the gaps), Reddit/HN threads on "why I switched".
- **Hands-on:** install 4–5 (likely Fork, Sublime Merge, GitButler, Anchorpoint, lazygit) and run
  the same scripted task list as Sourcetree (the baseline Max uses today) on a test repo. It
  includes a Unity project with LFS: commit hunks, interactive rebase, resolve a conflict (code and
  a prefab), stash, cherry-pick, undo a mistake, lock a file. Each download needs Max's OK first.
- **Output:** `docs/research/landscape.md` with a feature matrix (features × tools), one section per
  tool, and a "steal this mechanic" list. Mechanics only, never looks.

## 2. How developers use Git today

- **Topics:** trunk-based development vs GitFlow; short-lived branches + PRs; stacked PRs (Graphite,
  `rebase --update-refs`, GitButler stacks); interactive rebase, fixup, autosquash; worktrees (now
  common because AI agents work in parallel worktrees); monorepos (sparse checkout, partial clone,
  Scalar); SSH commit signing; conventional commits; LFS and file locking; submodules; jj (Jujutsu)
  ideas leaking back into Git tooling; AI in the commit/review loop.
- **Sources:** Stack Overflow Developer Survey, JetBrains State of Developer Ecosystem, GitHub
  Octoverse, Git release notes, plus Max's answers to section 0.
- **Output:** `docs/research/workflows.md`: the main workflows, each as a step sequence, with the
  steps that are painful today marked. Those are gitgud's openings.

## 3. UI/UX principles for a dense pro tool

- **Topics:** density with hierarchy; keyboard-first design and a command palette; progressive
  disclosure; safety (preview before destructive actions, undo for everything via reflog);
  direct manipulation (drag a commit to rebase or cherry-pick); live state (file watching,
  optimistic updates); latency budgets (<100 ms per action, 60 fps scrolling a 100k-commit graph);
  diff readability; light/dark themes; accessibility; desktop conventions on each OS.
- **Sources:** Nielsen's heuristics, Laws of UX, Fluent and Apple HIG, design write-ups from tools
  known for feel (Linear, Raycast, Zed, Arc).
- **Output:** `docs/research/ux-principles.md`, about 15 principles. Each is specific enough to
  judge a mockup against, with the source it comes from.

## 4. Technical spikes (alongside the research)

- Review the old prototype in git history (`5f1257d`): what worked, what to keep.
- `gix`: confirm it handles status, diff, blame and log/graph walks, and time them on a large repo
  (e.g. rust-lang/rust).
- Rendering the commit graph: canvas vs virtualized DOM, at 100k+ commits.
- File watching (`notify` crate) and refreshing the UI without polling.
- Calling the `git` CLI: progress output, credential prompts, cancelling a running command.
- A large Unity repo with LFS: time status, log and LFS pull against Sourcetree, and check whether
  `UnityYAMLMerge` can run as a merge driver from gitgud.
- Agent interface: one `gitgud-core` exposed both as a CLI and as an MCP server. How an agent's
  prepared commit shows up in the GUI for Max to approve.
- **Output:** a short note per spike with numbers. They feed the spec's performance targets and
  confirm or change the stack in `CLAUDE.md`.

## 5. Synthesis into a spec

- **Lab page** (`docs/lab/index.html`): layout and interaction variants built from the research,
  labelled A/B/C with "Claude's pick" (the `design-round` skill). Max decides.
- **`docs/spec.md`:**
  - positioning: who it's for and the 2–3 things it does better than the tools in section 1
  - MVP feature list (must / should / later) and non-goals
  - screen list
  - keyboard map
  - performance targets
  - a decision log in which every item is labelled decided (date + Max's words), Claude's pick,
    proposal or open
- **Adversarial review** of the spec draft (`adversarial-review` skill) before it goes to Max.

## How it runs

Order: 0 → 1, 2 and 3 in parallel → 4 → 5. Sections 1–3 are wide web research: one research
agent per track, and Claude merges the results and checks the sources. Sections 4–5 are Claude's,
with Max approving at the end of section 0, after the lab page, and on the spec.
