# Handoff: gitgud research (sections 1–4)

For the agent running the research. Written 2026-11-21.

## Read first

1. `docs/research-plan.md`. It is the brief: section 0 holds Max's decided answers, and sections
   1–4 are your scope. Section 5 (lab page and spec) is **not** yours; stop before it.
2. `CLAUDE.md`, for the planned stack (Tauri 2 + Rust + SvelteKit; `gix` for reads, `git` CLI for
   writes). The research may challenge it; if so, say so with evidence, don't edit it.

## Where things stand

- The repo is `HappyRave/gitgud`, cloned here. The old prototype's 83 files are staged for deletion
  (`git rm`); nothing is committed. **Don't commit, push or unstage.** Max decides that.
- No app code exists. Don't write any. The spikes in section 4 are throwaway code in the scratchpad
  or `spikes/`, never in the app's layout.

## Product in one paragraph

Max's own daily Git GUI, to be open-sourced later. Windows first with portable code. Works with any
host; light PR links only. **Full game-dev focus:** Unity, LFS, file locking, previews of binary
assets, `.meta` awareness, big repos. Opt-in AI commit messages, and a CLI/MCP so AI agents can
prepare commits for Max to review. It replaces Sourcetree and IDE git, whose pain points are: slow
on big/LFS repos, Unity merge conflicts, scary history edits, clunky UI.

## Deliverables

All in `docs/research/`:

| File | Plan section | Must contain |
|---|---|---|
| `landscape.md` | 1 | Feature matrix (features × tools, ✓/✗/partial), one section per tool, a "steal this mechanic" list ranked by fit with section 0, and a game-dev and agent-interface subsection |
| `workflows.md` | 2 | Each workflow as steps, with painful steps marked; then which pains gitgud can solve, matched to Max's four |
| `ux-principles.md` | 3 | ~15 principles, each one you could judge a mockup against, with a source |
| `spikes.md` | 4 | One section per spike: question, method, numbers, verdict, effect on the stack |
| `open-questions.md` | all | Anything only Max can answer, one line each, with your recommendation |

## Rules for this work

- **Cite everything.** Each factual claim gets a link and the date you checked it. If a claim's
  source is older than 2025, say so: features move fast.
- **Label each conclusion:** fact (sourced), Claude's pick, or open. Nothing is "decided" except
  section 0.
- **Mechanics, not looks.** Note what a tool does and how the interaction works; never propose
  copying its appearance. Screenshots are for understanding, not as specs.
- **Downloads and installs need Max's OK in chat first.** That covers hands-on GUIs, test repos and
  crates for spikes. Ask once with the full list (name, source, size) instead of one at a time.
  Use free tiers or trials only; creating accounts also needs his OK.
- **Never start the Android emulator** (it froze the PC). Not relevant here, but it's a hard rule on
  this machine.
- **Be concise.** Max reads dense tables and short lists, not prose. No summary or changelog files
  beyond the deliverables above.
- **Push back.** If the evidence says one of Max's section-0 calls is costly (e.g. binary previews
  in the MVP), say so plainly with numbers in `open-questions.md`. He decides.

## Suggested execution

- Tracks 1, 2 and 3 are independent web research: one research subagent each, run in parallel.
  Give each the relevant plan section and the rules above. Merge their results yourself and
  spot-check sources; don't paste their summaries in unverified.
- Spikes (section 4) after that, because track 1 may change what is worth testing. The large
  Unity + LFS repo needs Max to name one (ask) or OK a public one.
- Hands-on GUI tests need a Windows session with Max's go-ahead; batch them into one request.

## Done means

All five files exist, every matrix cell is sourced or marked unknown, and you report to Max with:
the 3–5 biggest findings, any challenge to section 0 or the stack, and the open questions. Then
stop: the lab page and spec come next, with Max.
