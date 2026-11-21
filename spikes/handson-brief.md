# Brief: hands-on GUI round

Run with computer use (desktop control) on. Load the `anthropic-skills:computer-use` skill first. Request access
once for Sourcetree, Fork, Sublime Merge, GitButler and the terminal that hosts lazygit.

## Setup

- Installed: Sourcetree 3.4.30 (baseline), Fork (lets itself update), Sublime Merge 2136, GitButler 0.22.3,
  lazygit 0.66.3 (`lazygit` on PATH; open a fresh terminal).
- No accounts. Skip anything that needs sign-in or a licence and note it. Decline telemetry.
- Test repo: `C:\Users\Happy\Documents\Projects\gitgud-handson\boat` (Unity + LFS BoatAttack). Its origin is
  `..\remote.git`.
- Reset before each tool: delete `C:\Users\Happy\Documents\Projects\gitgud-handson`, then run
  `python spikes/scripts/handson_setup.py` (script removed 2026-11-21; the round won't be re-run).

## Task list (same for every tool)

1. Open the repo; rough time until the graph is usable.
2. Stage only hunk A of `Assets/Scripts/Boat/Engine.cs` and commit.
3. View the modified texture `Assets/Objects/environment/Trees/Maps/TreeBillBoards_albedo.png`: is there an
   image diff, and what kind?
4. Interactive rebase of `feature/stack`: squash the `fixup!` commit, reword the "rgiht" typo.
5. Merge `conflict/theirs` into `conflict/ours`: what does the tool offer for `Engine.cs` and
   `ReplayCameras.prefab` (external tool, UnityYAMLMerge, mine/theirs)? Resolve both.
6. Stash and pop.
7. Cherry-pick `hotfix/camera` onto `master`.
8. Undo: discard a change and try to recover it; undo the last commit.
9. LFS lock: is there any lock UI? Record what's offered; the local remote can't actually lock.
10. Keyboard: is there a command palette, and how far does the keyboard get you?

For each tool × task, record: ✓ / ~ / ✗, clicks or keys, friction, surprises. Mechanics only, not looks.

## Deliverables

- `docs/research/handson.md`: results matrix, one short section per tool, and a "steal / avoid" list labelled
  Claude's pick.
- 10–20 screenshots, `docs/research/img/ho-*.webp` (PIL, quality 78, max width 1440), with annotations in
  `docs/research/shots/handson.json`. Same schema as `shots/gui.json`; source "hands-on 2026-11-21".
- Fill in the `?` cells of the feature matrix in `docs/research/landscape.md` that the round settles, cited
  "hands-on".
- Hub: add the `handson` track and doc to the hub build script. Rebuild `docs/research/hub.html` and
  republish it to https://claude.ai/artifact/Ueq7AGKLWtFs2nKDKrW93Z (read it first, then publish with its
  url, including the `img/ho-*` files).
- Don't commit. Close the apps and delete `gitgud-handson` when done.
