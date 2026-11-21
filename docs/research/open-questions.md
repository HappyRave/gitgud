# Open questions for Max

Collected 2026-11-21 from all tracks. **All answered by Max on 2026-11-21** (one at a time, with
Claude's pick as the recommended option). "Answer" is Max's choice; where it differs from Claude's
pick, the pick is kept for the record.

## Product and scope

| # | Question | Answer (decided 2026-11-21) | Source |
|---|---|---|---|
| Q1 | The name: `zeuros/gitgud` (Git GUI, 2024, 18★) launched on HN 2026-10-17 with "undo any action". Rename? | **Keep "gitgud" for good**, accepting the clash. (Claude's pick was a working name until going public.) | [HN](https://news.ycombinator.com/item?id=50132140), [repo](https://github.com/zeuros/gitgud) |
| Q2 | Which host do your Unity repos use? | **Bitbucket and GitHub.** Lock features target both. `open`: confirm Bitbucket's LFS lock API support before the spec relies on it. | gamedev.md §2.9 |
| Q3 | Solo, or with artists/designers? | **Solo first, artists later** (artist mode after v1). | gamedev.md §6 |
| Q4 | Lore (Epic, 2026): ignore, or keep `gitgud-core` VCS-agnostic? | **Ignore, Git-only core.** | gamedev.md §2.5 |
| Q5 | Unity editor package in v1? | **After v1.** v1 = locks inside gitgud only. | gamedev.md T13 |
| Q6 | Binary previews in the MVP? | **Textures + prefab object list**: image diff (side-by-side/swipe/onion) plus a structural prefab diff of changed objects and fields; rendered prefab thumbnails later. | gamedev.md T7–T8 |
| Q7 | Sparse checkout / `lfs.fetchinclude` UI in v1? | **Later.** LFS-aware status speed first. | gamedev.md T9 |
| Q8 | Stacked branches: show, or manage? | **Show + restack** via `rebase --update-refs`; PR links per level via browser URLs. | workflows.md W3 |

## Safety and history

| # | Question | Answer (decided 2026-11-21) | Source |
|---|---|---|---|
| Q9 | Undo model? | **Own op log behind Ctrl+Z**: snapshot refs, index and working tree under a private ref before every action. | ux-principles.md, handson.md |
| Q10 | Use Git's experimental `git history`, or own the rewrite logic? | **Own it in core** (gix + `git replay`/rebase), with conflict prediction and a result-graph preview. | workflows.md W4 |
| Q11 | Record conflicts and continue, or classic stop? | **Classic stop + preview**: predict conflicts before starting, persistent Continue/Abort banner. | landscape.md, handson.md |

## Agents and AI

| # | Question | Answer (decided 2026-11-21) | Source |
|---|---|---|---|
| Q12 | Agent flow? | **Proposals under `refs/gitgud/proposals/<id>` + review inbox**; the index is untouched until approval; approve commits through the git CLI. | agents.md §6 |
| Q13 | AI commit messages: installed CLI or API key? | **Installed CLI first, API key optional.** | landscape.md |
| Q14 | Which Unity checks block an agent proposal? | **Flag all three, block none**; Max decides on review. | agents.md |

## From the spikes and the hands-on round

| # | Question | Answer (decided 2026-11-21) | Source |
|---|---|---|---|
| Q15 | Git 2.45.1 breaks LFS clones. Upgrade? | **Claude upgrades it.** Done: Git for Windows 2.56.1, git-lfs 3.8.1. | spikes.md 4.6 |
| Q16 | Remove `GIT_CURL_VERBOSE=0` from the environment? | **Claude removes it.** Done: user variable deleted (new terminals pick it up). | spikes.md 4.6 |
| Q17 | Minimum Git version? | **Bundle our own Git; use the system Git by default when it meets the floor, bundled Git as fallback.** Floor: 2.45.2. Claude's pick was "≥ 2.45.2, recommend ≥ 2.54, no bundle"; the system-first default keeps hooks, credential helpers and signing identical to the terminal. | spikes.md, workflows.md |
| Q18 | lazygit hands-on column? | **Skip it.** | handson.md |
| Q19 | Keep the test repos (rust 1.2 GB, BoatAttack 1.1 GB) in the old session scratchpad? | **Keep until the spec is done.** They live in Temp, so a cleanup could still wipe them. | — |

Downloads approved by Max on 2026-11-21 ("ok all downloads"). Used for the Unity repo: Unity-Technologies/BoatAttack (public, Claude's pick, since none was named).

## Challenges to section 0 or the stack

- **Binary previews (answer 4)**: textures are cheap; prefab previews mean rendering or a structural diff
  of Unity YAML with GUID resolution: weeks, not days. Settled by Q6: textures + structural diff in the
  MVP, rendered thumbnails later.
- **Stack**: no challenge, now measured (spikes.md). WebView2 held 100 fps scrolling 349k commits; `gix`
  walks the full graph topology in 106 ms. Two `gix` rules: rename detection off or capped for big ranges,
  and refresh the index after a mass touch (status doesn't write it back).
- **"No in-app PR APIs" (answer 3)**: GitHub stacked PRs went GA 2026-10-06; opening the right base for each
  stack level works through browser URLs, so no change needed.
- **Bundled Git (Q17)**: CLAUDE.md says writes go through the `git` CLI so hooks and credentials behave as in
  a terminal. Running the system Git by default keeps that; the bundled copy only covers machines with no
  Git or one below the floor.
