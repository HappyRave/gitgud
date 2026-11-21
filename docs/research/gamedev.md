# Game-dev version control: tools, Unity specifics, workflows, and where gitgud fits

Track B ("gamedev") of the [research plan](../research-plan.md). Every link was checked **2026-11-21** unless a
row says otherwise. Labels: **fact** (sourced), **Claude's pick**, **open**. Nothing here is decided.
Screenshots with pinned notes: [`shots/gamedev.json`](shots/gamedev.json) (`gd-*` images in `img/`).

## 0. Takeaways (the five that matter)

1. **Git + LFS is what game devs actually use, except at AAA.** In the JetBrains 2025 game-dev survey, GitHub is
   used by 61% of mid-size/indie and 79% of hobbyist respondents; Perforce P4 by 45% of AAA but 10% of
   mid/indie; Unity Version Control by 9% / 7% ([JetBrains](https://lp.jetbrains.com/the-state-of-gamedev-2025/),
   few AAA respondents). **fact.** Max's segment is the Git majority.
2. **Locking is the gap no Git GUI closes.** LFS has a full locking API (`git lfs lock/locks/unlock --json`,
   `/locks/verify` with ours/theirs) ([API](https://github.com/git-lfs/git-lfs/blob/main/docs/api/locking.md)), but
   "unlock after push" has been an open git-lfs issue since 2017
   ([#2671](https://github.com/git-lfs/git-lfs/issues/2671)), Sourcetree never shipped LFS locks
   ([SRCTREE-4663](https://jira.atlassian.com/browse/SRCTREE-4663)), GitKraken has an open request
   ([feedback](https://feedback.gitkraken.com/suggestions/220717/support-git-lfs-large-file-storage-locking-feature)),
   Fork has a manual locks list only ([Fork #1862](https://github.com/fork-dev/TrackerWin/issues/1862)). Anchorpoint
   solved it by **abandoning LFS locks** for its own server ([docs](https://docs.anchorpoint.app/git/file-locking/)).
   **fact.**
3. **Unity merges are fixable only half-way.** UnityYAMLMerge merges `.unity`, `.prefab`, `.meta` semantically but not
   `.asset`/`.mat`/`.controller` without `--force`, and Unity documents only the interactive `git mergetool` route,
   not a merge driver ([Smart merge](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge)).
   Known bugs on prefab instances/variants/nested prefabs (Unity issue tracker, §3.3). **fact.**
4. **Unity's own editor VCS integration is Perforce + UVCS only.** For Git, Unity offers nothing beyond "Visible Meta
   Files" ([integrations](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)).
   Every Git-for-Unity locking story (Anchorpoint, Diversion) needs an editor plugin because Unity ignores read-only
   files ([Anchorpoint](https://docs.anchorpoint.app/git/file-locking/)). **fact.**
5. **The market is moving fast in 2026.** Epic open-sourced **Lore** (Rust, MIT, centralized, chunked, pre-1.0, 9.2k
   stars) in June 2026 ([repo](https://github.com/EpicGames/lore)); Anchorpoint is rewriting its app as a Lore client
   ([docs](https://docs.anchorpoint.app/lore/quick-start/)); Unity dropped UVCS cloud seat fees on 2026-03-01
   ([Unity](https://unity.com/en/products/pricing-updates)). **fact.** Git stays the default for Unity indies; gitgud
   should be the best *Git* client for them, not a new VCS. **Claude's pick.**

## 1. Tool comparison matrix

Legend: ✓ yes · ~ partial · ✗ no · ? not verified. Each cell links to its source or the per-tool section.

| Feature | Anchorpoint | Unity VCS (Plastic) | Perforce P4V | Diversion | Lore (Epic) | UE Git plugin (PB) | Fork / GitHub Desktop / GitKraken / Sourcetree |
|---|---|---|---|---|---|---|---|
| Underlying VCS | Git (+ own metadata server); Lore client in rewrite [[1]](https://docs.anchorpoint.app/lore/quick-start/) | Plastic (own) | P4 (own, centralized) | Own, cloud [[2]](https://www.diversion.dev/pricing) | Own, centralized, content-addressed [[3]](https://github.com/EpicGames/lore) | Git + LFS | Git + LFS |
| Hard file lock | ✓ own server, not LFS [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ Smart Locks [[5]](https://docs.unity.com/en-us/unity-version-control/smart-locks) | ✓ `+l` typemap [[6]](https://perforce.com/manuals/p4sag/Content/P4SAG/superuser.basic.typemap_locking.html) | ✓ Hard Locks (Studio tier) [[2]](https://www.diversion.dev/pricing) | ~ advisory only, redesign 2026 [[7]](https://epicgames.github.io/lore/faq/) | ✓ LFS locks [[8]](https://github.com/ProjectBorealis/UEGitPlugin) | Fork ~ manual list; others ✗ (§2.8) |
| Auto-lock on edit | ✓ binaries on modify [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ checkout matching lock rule [[5]](https://docs.unity.com/en-us/unity-version-control/smart-locks) | ✓ checkout-on-edit in editor | ~ "soft locks" warn [[9]](https://www.diversion.dev/unity-integration) | ? | ✓ auto-checkout setting [[8]](https://github.com/ProjectBorealis/UEGitPlugin) | ✗ |
| Auto-unlock on push/submit | ✓ [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ on check-in / merge to destination [[5]](https://docs.unity.com/en-us/unity-version-control/smart-locks) | ✓ on submit | ? | ? | ✓ on in-editor submit [[10]](https://github.com/fork-dev/TrackerWin/issues/1862) | ✗ ([#2671](https://github.com/git-lfs/git-lfs/issues/2671)) |
| Locks across branches | ✓ "outdated" cross-branch flag [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ retained lock until merged to destination [[5]](https://docs.unity.com/en-us/unity-version-control/smart-locks) | per path/stream | ? | planned [[11]](https://epicgames.github.io/lore/roadmap/) | ✓ Status Branches [[8]](https://github.com/ProjectBorealis/UEGitPlugin) | ✗ |
| "Who has this file" in UI | ✓ avatar on file [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ lock icon | ✓ blue badge = other user [[12]](https://www.perforce.com/manuals/p4v/Content/P4V/using.icons.html) | ✓ clash indicators [[9]](https://www.diversion.dev/unity-integration) | ? | ✓ in editor | Fork: Repository > Git LFS > Locks [[10]](https://github.com/fork-dev/TrackerWin/issues/1862) |
| Partial / selective checkout | ✓ sparse checkout (Team) [[13]](https://docs.anchorpoint.app/git/sparse-checkout/) | ✓ Gluon partial workspace [[14]](https://docs.unity.com/en-us/unity-version-control/gluon/configure-workspace) | ✓ workspace mapping | ? | ✓ on-demand hydration, VFS [[3]](https://github.com/EpicGames/lore) | ✗ | ✗ |
| Binary previews | ✓ thumbnails + 3D viewer (FBX shown) [[15]](https://www.anchorpoint.app/) | ✓ Gluon previews ≤1 MiB default [[14]](https://docs.unity.com/en-us/unity-version-control/gluon/configure-workspace); no FBX thumbs (vendor claim [[16]](https://www.anchorpoint.app/blog/best-version-control-software-for-unity)) | ~ | ✓ "Art Previews" (Pro) [[2]](https://www.diversion.dev/pricing) | ✓ image/video/audio compare via Anchorpoint [[1]](https://docs.anchorpoint.app/lore/quick-start/) | ✗ | GitKraken/Fork: image diff only (see landscape track) |
| Unity editor plugin | ✓ beta, UPM git URL [[17]](https://docs.anchorpoint.app/git/unity/) | ✓ built into Unity [[18]](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration) | ✓ Unity Perforce mode [[18]](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration) | ✓ open beta, Asset Store [[9]](https://www.diversion.dev/unity-integration) | ✗ (Unreal only, 2027) [[11]](https://epicgames.github.io/lore/roadmap/) | n/a | ✗ |
| Unreal plugin | ✓ [[4]](https://docs.anchorpoint.app/git/file-locking/) | ✓ | ✓ (default for UE) | ✓ Epic-recommended [[2]](https://www.diversion.dev/pricing) | 2027 [[11]](https://epicgames.github.io/lore/roadmap/) | ✓ is the plugin | ✗ |
| Smart Merge (UnityYAMLMerge) wired | manual `.git/config` [[17]](https://docs.anchorpoint.app/git/unity/) | via client merge-tool rule [[19]](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge) | via P4V ext. mapping + Unity "Premerge" [[19]](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge) | ? | ✗ | n/a | Sourcetree: manual custom tool [[19]](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge) |
| Any Git host | ✓ (Team) [[20]](https://www.anchorpoint.app/pricing) | ✗ own cloud/on-prem | ✗ | ✗ (GitHub mirror) [[9]](https://www.diversion.dev/unity-integration) | ✗ | ✓ | ✓ |
| Price | Free (personal, GitHub only); €20–25/user/mo Team [[20]](https://www.anchorpoint.app/pricing) | Free seats on cloud since 2026-03; 25 GB/org free, 100 GB egress [[21]](https://unity.com/en/products/pricing-updates) | Free ≤5 users/20 workspaces; Cloud ~$39/user/mo [[22]](https://www.perforce.com/products/helix-core/free-version-control) | Indie free (5 users, 100 GB); $25 / $35 user/mo [[2]](https://www.diversion.dev/pricing) | Free, MIT | Free, MIT | Free–paid |

## 2. Per-tool sections

### 2.1 Anchorpoint (Git client for game teams)
- **Model:** Git CLI underneath plus a proprietary **metadata server** for locks, "Attributes" (approvals, tags, notes)
  and a project timeline ([pricing](https://www.anchorpoint.app/pricing), [locking](https://docs.anchorpoint.app/git/file-locking/)). **fact.**
- **Locking mechanics** ([docs](https://docs.anchorpoint.app/git/file-locking/)): **fact.**
  - binaries auto-lock the moment they are modified; text files opt-in by extension;
  - locks release on push (others must pull before read-only clears, [blog 2025-08-21](https://www.anchorpoint.app/blog/using-file-locking-in-git));
  - locked files are made read-only; Unity ignores read-only, so the **Unity plugin is required** for locks to bite;
  - anyone may force-unlock, logged in the timeline; deletes/renames lock after asking;
  - optional "track outdated files across branches": files changed on any branch touched in the last 7 days
    (`cross_branch_lookback_days` in `.ap/config.ini`) show as locked/outdated on yours; needs merge commits, not rebase;
  - explicitly **not** Git LFS locks: "should not be used" together.
  - Claims 1,000 files locked in under a second ([blog, vendor](https://www.anchorpoint.app/blog/perforce-alternative-for-game-development)).
- **Unity** ([docs](https://docs.anchorpoint.app/git/unity/)): plugin (beta, `ap-unity`, 12 stars,
  [repo](https://github.com/Anchorpoint-Software/ap-unity)) commits from the editor and shows locks; auto-configures
  LFS; ships a Unity `.gitignore`; tells users to paste the UnityYAMLMerge `mergetool` block into `.git/config` by hand;
  recommends "split scenes into prefabs and lock the prefab", trunk-based dev, art sources in a separate repo. **fact.**
- **Selective checkout** = Git sparse checkout; other clients (GitHub Desktop) then need a full download
  ([docs](https://docs.anchorpoint.app/git/sparse-checkout/)). **fact.**
- **Scale claim:** 1 TB / 385,493 files on self-hosted GitLab; initial push 30 h at 30 Mbit; `git status` after 10
  changed files 1.58 s with sparse checkout ([blog, vendor](https://www.anchorpoint.app/blog/scaling-git-to-1tb-of-files-with-gitlab-and-anchorpoint-using-git-lfs)). **fact.**
- **2026 pivot:** "Anchorpoint for Lore" is a rewrite that "will replace the current Anchorpoint application"; Git
  support returns later; it advertises reviewing work "from teammates and AI agents"
  ([docs](https://docs.anchorpoint.app/lore/quick-start/)). **fact.** → an opening for a dedicated Git game-dev client.
  **Claude's pick.**

### 2.2 Unity Version Control (Plastic SCM)
- **Smart Locks** ([docs](https://docs.unity.com/en-us/unity-version-control/smart-locks)): **fact.**
  - lock rules (one org rule, many repo rules) turn a normal checkout into a lock for matching paths;
  - only the **latest revision** (on the destination branch, default `/main`) can be locked: you cannot lock a stale copy;
  - checking in on a non-destination branch sets **Retained**; the lock clears only when merged to the destination.
- **Gluon** = artist mode: a *partial* workspace where you tick files/folders to load, edit, check in without updating
  the whole tree; previews of unloaded files up to `MaxPreviewFileSize` (1 MiB default)
  ([docs](https://docs.unity.com/en-us/unity-version-control/gluon/configure-workspace)). **fact.**
- **Unity editor**: UVCS mode cannot lock/unlock or resolve conflicts from the Project view, those go to the UVCS GUI
  ([Unity 6.3](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)). **fact.**
- **Pricing 2026:** cloud seat fees removed 2026-03-01; storage free allowance 5 → 25 GB/org/month; new egress meter, first
  100 GB free; without a card on file access is blocked when over the free tier ([Unity](https://unity.com/en/products/pricing-updates),
  [support](https://support.unity.com/hc/en-us/articles/34748492914964-Understanding-Unity-DevOps-charges)). **fact.**
- **Users:** Unity forum thread "Unity Version Control. Why is it terrible?" (May 2025 → Jun 2026): pushes hanging,
  crashes, clumsy branch UI, Linux plugin unsupported; recommended alternatives Git+GitHub/LFS, standalone client,
  Perforce ([thread](https://discussions.unity.com/t/unity-version-control-why-is-it-terrible/1647889)). **fact.**

### 2.3 Perforce P4 / P4V (formerly Helix Core)
- Renamed "P4" in 2025; P4V docs at 2026.3 ([icons](https://www.perforce.com/manuals/p4v/Content/P4V/using.icons.html)). **fact.**
- **Exclusive checkout:** `+l` file-type modifier via typemap (e.g. `binary+l //....uasset`); typemap applies only to
  newly added files, so set it first ([typemap](https://perforce.com/manuals/p4sag/Content/P4SAG/superuser.basic.typemap_locking.html),
  [game engines](https://perforce.com/node/1164066)). **fact.**
- **Visibility:** red badge = your action, blue = another user's (checked out / open for edit / locked), yellow ▲ =
  "not latest version", hover for detail ([icons](https://www.perforce.com/manuals/p4v/Content/P4V/using.icons.html)). **fact.**
- **Shelving** keeps the exclusive lock (single third-party test on p4d 2025.2,
  [Classmethod](https://dev.classmethod.jp/en/articles/perforce-exclusive-lock-behavior/)). **fact, one source.**
- Unity's Perforce mode: auto checkout on save, "Overwrite Failed Checkout Assets" (default on), new "Auto Revert
  Unchanged Files" ([Unity 6.3](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)). **fact.**
- Free ≤5 users / 20 workspaces; Cloud ~$39/user/month (team cap stated as 50 or 100 on different pages)
  ([Perforce](https://www.perforce.com/products/helix-core/free-version-control)). **fact.**
- Opinion: an HN commenter from a studio that used Helix Core Cloud says it is the default creatives know, programmers
  dislike it, and it "does show its years" ([HN Lore thread](https://news.ycombinator.com/item?id=48571081)). **fact (paraphrase).**

### 2.4 Diversion
- YC S22, cloud-native, $12 M raised by May 2025 ([GamesBeat](https://gamesbeat.com/diversion-raises-12m-in-funding-for-its-version-control-platform-for-creatives)). **fact.**
- Unity plugin "Diversion Connect" (open beta, Asset Store): status overlays, commit/reset from editor, branch +
  sync status, **conflict-prevention alerts** when you open/save a file a teammate is editing ("live soft locks")
  ([page](https://www.diversion.dev/unity-integration)). **fact.**
- Pricing: Indie free (5 users, 100 GB), Pro $25, Studio $35 (Hard Locks, Prune), extra storage $10/100 GB
  ([pricing](https://www.diversion.dev/pricing)). **fact.** Vendor claims: 400k files committed <30 s, 5 M-file clone
  <5 min ([80.lv, sponsored](https://80.lv/articles/game-studios-are-moving-beyond-legacy-version-control-and-here-s-why)).
- HN launch (280 pts, 2024): skepticism about a closed, cloud-only VCS replacing Git
  ([HN](https://news.ycombinator.com/item?id=39088551), source 2024). **fact.**

### 2.5 Lore (Epic, new 2026)
- Open-sourced June 2026 (repo created 2026-05-21), MIT, Rust, v0.14.0, 9.2k stars; centralized, content-addressed
  Merkle trees, chunked large files, on-demand hydration, FUSE/VFS in progress ([repo](https://github.com/EpicGames/lore),
  [HN 1,276 pts](https://news.ycombinator.com/item?id=48571081)). **fact.**
- Locks today are advisory; scalable enforced locking is a 2026 roadmap item; open-source desktop client and Unreal
  plugin 2027; Unity and Git interop not on the roadmap ([roadmap](https://epicgames.github.io/lore/roadmap/),
  [FAQ](https://epicgames.github.io/lore/faq/)). **fact.**
- Relevance to gitgud: **watch, don't build for it now.** Unity + Git users have no migration path to Lore yet.
  **Claude's pick.** Whether gitgud's core should be VCS-agnostic enough to add Lore later: **open.**

### 2.6 Unreal integrations
- **ProjectBorealis UEGitPlugin** (831 stars, pushed 2026-08): refactor of SRombauts' plugin (883 stars, last push
  2024-01): multi-threaded LFS lock/unlock, local lock cache, **Status Branches** (flags files changed on configured
  remote branches), background remote refresh, explicit `*.uasset`/`*.umap` `.gitattributes` lines required, no
  prebuilt binaries ([README](https://github.com/ProjectBorealis/UEGitPlugin)). **fact.**
- Unreal submits push then LFS-unlock every pushed file; artists using Fork for the rest had to unlock by hand
  ([Fork #1862](https://github.com/fork-dev/TrackerWin/issues/1862), source 2023–24). **fact.**
- **One File Per Actor:** actors stored in separate files with encoded names so editors don't lock whole levels; the
  readable "View Changelist" works only with Perforce
  ([Epic docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/one-file-per-actor-in-unreal-engine)). **fact.**

### 2.7 Godot
- Text scenes (`.tscn`) merge as text; official `godot-git-plugin` is a libgit2 implementation of Godot's VCS
  interface, no LFS/lock features mentioned, 966 stars, active 2026-10
  ([repo](https://github.com/godotengine/godot-git-plugin)). **fact.** Low priority for gitgud. **Claude's pick.**

### 2.8 General Git GUIs and LFS locks
| Client | LFS lock support | Source |
|---|---|---|
| Sourcetree | none | [SRCTREE-4663](https://jira.atlassian.com/browse/SRCTREE-4663) (undated) |
| GitKraken | LFS track/push, locking still a feature request | [feedback](https://feedback.gitkraken.com/suggestions/220717/support-git-lfs-large-file-storage-locking-feature) |
| Fork (Win) | lock/unlock + "Repository > Git LFS > Locks" list; no unlock-on-push ("technically impossible" per dev) | [#1862](https://github.com/fork-dev/TrackerWin/issues/1862), [#605](https://github.com/fork-dev/TrackerWin/issues/605) |
| GitHub Desktop | no lock UI found | [issues search](https://github.com/desktop/desktop/issues/2812) |

### 2.9 Git LFS itself
- **Lockable:** `git lfs track "*.psd" --lockable` → `.gitattributes` gets `lockable`; Git LFS then makes those files
  **read-only** unless you hold the lock (`lfs.setlockablereadonly`, default true)
  ([wiki](https://github.com/git-lfs/git-lfs/wiki/File-Locking), [config](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-config.adoc)). **fact.**
- **Commands:** `git lfs lock --json`, `git lfs locks --json [--local|--cached|--verify] [--path]`, `unlock --force`
  ([lock](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-lock.adoc),
  [locks](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-locks.adoc)). `--local` is offline; `--cached`
  shows others' locks from the last call. **fact.**
- **API:** `POST /locks`, `GET /locks?path&cursor&limit&refspec`, `POST /locks/verify` → `{ours, theirs}`,
  `POST /locks/:id/unlock {force}`; 409 on conflict; push is blocked if it touches a `theirs` file; 404 on verify =
  server has no locking ([API](https://github.com/git-lfs/git-lfs/blob/main/docs/api/locking.md)). **fact.**
- **Push check:** `lfs.locksverify` null/true/false per host. **fact.**
- **Partial fetch:** `lfs.fetchinclude` / `fetchexclude`, `fetchrecentrefsdays` (7), `pruneoffsetdays` (3),
  `concurrenttransfers` (8) ([config](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-config.adoc)). **fact.**
- Latest release v3.8.1 (2026-10-22) ([releases](https://github.com/git-lfs/git-lfs/releases)). **fact.**
- **Git core is catching up:** large-object promisors landed in pieces from Git 2.49/2.50 and are still WIP; Git 2.54
  (Apr 2026) made the object database backend pluggable
  ([LOP doc](https://git-scm.com/docs/large-object-promisors), [GitLab on 2.54](https://about.gitlab.com/blog/whats-new-in-git-2-54-0/)). **fact.**
  LFS stays the practical answer through gitgud v1. **Claude's pick.**

**Host support and cost (LFS)**

| Host | LFS locks | Free LFS | Overage | Max file |
|---|---|---|---|---|
| GitHub | ✓ (`GET /locks` answers 200, probed 2026-11-21) | 10 GiB storage + 10 GiB bandwidth (Free/Pro); 250/250 (Team/Enterprise) | metered, $0.07/GiB-month storage, $0.0875/GiB bandwidth; $0 budget = LFS blocked for the month | 2 GB Free/Pro, 4 GB Team, 5 GB Ent. |
| GitLab | ✓ LFS exclusive locks all tiers; extra UI default-branch locks Premium+ | 10 GiB/project incl. repo + LFS (Free) | storage packs | — |
| Azure DevOps | ✓ | free (no stated cap; Anchorpoint says no charge for LFS space) | — | no SSH with LFS |
| Bitbucket Cloud | ✗ (DC/Server ✓) | 1 GB Free, 5 GB Std, 10 GB Prem | 100 GB packs | — |
| Gitea | ✓ (follows the LFS lock API, `LFS_LOCKS_PAGING_NUM`) | self-hosted | — | — |

Sources: [GitHub billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs),
[GitHub rates](https://docs.github.com/en/billing/using-the-new-billing-platform/about-billing-for-git-large-file-storage),
[GitHub sizes](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-git-large-file-storage),
[GitLab locks](https://docs.gitlab.com/user/project/file_lock/), [GitLab quotas](https://docs.gitlab.com/user/storage_usage_quotas/),
[Azure](https://learn.microsoft.com/azure/devops/repos/git/manage-large-files),
[Bitbucket limits](https://support.atlassian.com/bitbucket-cloud/docs/current-limitations-for-git-lfs-with-bitbucket),
[Bitbucket storage](https://support.atlassian.com/bitbucket-cloud/docs/storage-policy-for-git-lfs-with-bitbucket),
[Gitea locks code](https://git.fb1.uni-bremen.de/davrot/forgejo_backup/src/branch/v1.20/forgejo/services/lfs/locks.go). All **fact.**
GitHub's own example: a 1-byte change to a 500 MB LFS file costs another 500 MB storage; CI downloads bill the owner
([billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs)). **fact.**

## 3. Unity specifics

### 3.1 `.meta` files
- Each asset's `.meta` holds its unique ID (GUID) and import settings. Lose it → Unity generates a new one and
  references break (materials lose textures, prefabs lose scripts). Move/rename outside Unity → you must move the
  `.meta` too. Empty-folder metas: VCS can't store empty dirs, so Unity deletes/creates folders to match
  ([AssetMetadata](https://docs.unity3d.com/6000.2/Documentation/Manual/AssetMetadata.html)). **fact.**
- Failure modes gitgud can detect cheaply (**Claude's pick**, all from the rules above):
  - **orphan meta**: `X.meta` staged/committed without `X` (or vice versa) → a teammate's Unity regenerates a GUID;
  - **split commit**: asset and its meta in different commits / one staged, one not;
  - **rename without meta**: `git mv` of `X` but `X.meta` left behind;
  - **GUID collision**: two metas with the same `guid:` (duplicated outside Unity);
  - **meta-only diffs** (import-setting churn) that are usually noise.

### 3.2 Serialization and ignore
- Text serialization ("Force Text") is the VCS-friendly mode and the default in recent Unity
  ([Rider docs](https://www.jetbrains.com/help/rider/Asset-serialization-mode.html),
  [Unity text format](https://docs.unity3d.com/Manual/TextSceneFormat.html)); some assets still save binary
  (NavMesh, [issue](https://issuetracker.unity3d.com/issues/asset-serialization-mode-force-text-saves-navmesh-in-binary)). **fact.**
- github/gitignore `Unity.gitignore`: `/[Ll]ibrary/`, `/[Tt]emp/`, `/[Oo]bj/`, `/[Bb]uild(s)/`, `/[Ll]ogs/`,
  `/[Uu]ser[Ss]ettings/`, `*.csproj`, `*.sln`, Addressables temp, `*.unitypackage`, `.vs/` and more
  ([template](https://github.com/github/gitignore/blob/main/Unity.gitignore)). **fact.**

### 3.3 UnityYAMLMerge / Smart Merge
- **Handles:** `.unity`, `.prefab`, `.meta`. Other YAML (`.asset`, `.mat`, `.controller`) goes to the fallback tool
  unless `--force` ([Smart merge, 6000.3](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/smart-merge)). **fact.**
- **CLI:** `UnityYAMLMerge merge [opts] <base> <theirs> <mine> [<output>]`; `-p` premerge then open fallback with only
  remaining conflicts; `-h` headless (no dialogs); `-l`/`-r` take theirs/mine on conflict; `-o <file>` writes conflicts;
  `--fallback none` disables fallback tools; `--rules` custom `mergerules.txt` (array keys, exclusions, float epsilon).
  Path: `C:\Program Files\Unity\Hub\Editor\<version>\Editor\Data\Tools\UnityYAMLMerge.exe` (same doc). **fact.**
- **Documented Git setup** is `mergetool` only (`cmd = ... merge -p "$BASE" "$REMOTE" "$LOCAL" "$MERGED"`,
  `trustExitCode = false`). **fact.**
- **As an automatic merge driver** (Git: `merge.<name>.driver` with `%O` base, `%A` ours/result, `%B` theirs; exit 0 =
  clean, 1–128 = conflict, >128 = failure, [gitattributes](https://git-scm.com/docs/gitattributes)): mapping Unity's
  `<base> <theirs> <mine> <output>` gives `UnityYAMLMerge merge -h --fallback none %O %B %A %A`. **Claude's pick;
  unverified — the research plan's §4 spike must confirm exit codes and in-place output.** Unity doesn't document it.
- **Known bugs:** misses conflicts in prefab instances (both edits silently kept)
  ([issue](https://issuetracker.unity3d.com/issues/unityyamlmerge-tool-is-not-detecting-conflicts-in-prefab-instances));
  prefab-variant property conflicts keep both values
  ([issue](https://issuetracker.unity3d.com/issues/unityyamlmerge-dot-exe-doesnt-correctly-handle-merge-conflicts-in-modified-properties-on-a-prefab-variant));
  nested-prefab creation vs. parent edit → "Won't Fix"
  ([issue](https://issuetracker.unity3d.com/issues/breaking-merge-conflicts-when-creating-a-nested-prefab-while-the-parent-prefab-has-been-modified-somewhere-else)). **fact.**
  ⇒ a "clean" Smart Merge result still needs a review step. **Claude's pick.**
- Unity's own stance: if scenes/prefabs are exclusively locked, you don't need Smart Merge (same doc). **fact.**

### 3.4 Unity 6 VCS integration status
- Project Settings > Version Control modes: Visible Meta Files (default, = "any other VCS" incl. Git), Hidden Meta
  Files, Perforce; UVCS via its own package/mode. Editor operations (Check Out, Lock, Get Latest, Resolve) exist for
  Perforce; UVCS lacks Lock/Resolve in-editor; **Git: none**
  ([integrations 6000.3](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)). **fact.**
- Hook points for a plugin: `AssetDatabase.IsOpenForEdit`, `AssetDatabase.MakeEditable`,
  `Provider.PreCheckoutCallback`, `Provider.PreSubmitCallback` (same doc). **fact.** Unity's custom VCS provider API
  for Git is what Anchorpoint and Diversion use (Diversion: set Mode to "Diversion",
  [page](https://www.diversion.dev/unity-integration)). **fact.**

### 3.5 Asset previews
- Anchorpoint: grid thumbnails for images/EXR, 3D viewer for FBX in the side panel ([home](https://www.anchorpoint.app/)).
  Gluon: previews for unloaded files ≤1 MiB ([docs](https://docs.unity.com/en-us/unity-version-control/gluon/configure-workspace)).
  Diversion: "Art Previews" (Pro). **fact.**
- For gitgud (**Claude's pick**): Rust-side decode of PNG/JPG/TGA/PSD-composite/EXR via the `image` crate family,
  side-by-side + swipe/onion compare between commits (LFS blobs fetched on demand); for `.prefab`/`.unity`: a
  **structural diff** (GameObject tree, components, changed fields, resolved by GUID → asset path through `.meta`)
  instead of raw YAML. 3D (FBX) thumbnails: **open** (cost vs. value).

## 4. How game teams work day to day (step lists; ⚠ = painful today)

Sources for pains: §2 citations, [Fork #1862](https://github.com/fork-dev/TrackerWin/issues/1862),
[UVCS forum](https://discussions.unity.com/t/unity-version-control-why-is-it-terrible/1647889),
[GitHub LFS budget threads](https://github.com/orgs/community/discussions/151710). Step order is **Claude's synthesis**.

**Artist (Git + LFS, Unity)**
1. Pull latest. ⚠ LFS downloads every changed binary; slow, eats owner's bandwidth quota.
2. Find the asset, check nobody is on it. ⚠ No "who has this file" in Sourcetree/GitHub Desktop; ask in chat.
3. Lock it. ⚠ CLI `git lfs lock` or nothing; read-only files confuse Unity (it ignores the flag).
4. Edit in DCC tool, export to Unity, Unity rewrites `.meta`/import settings.
5. Commit asset + meta. ⚠ Easy to miss the `.meta` or commit Library noise; no preview of what changed visually.
6. Push. ⚠ Locks stay held after push in every Git GUI ([#2671](https://github.com/git-lfs/git-lfs/issues/2671)).
7. Unlock. ⚠ Manual, per file, often forgotten → "who locked this?" days later.

**Designer (scenes/prefabs)**
1. Pull; open scene. ⚠ If a teammate edits the same scene, conflict appears only at merge.
2. Edit prefab/scene. 3. Commit. ⚠ Unity dirty-saves touch unrelated files (`SetDirty` without change,
   [Unity](https://docs.unity.com/en-us/engine/6000.3/manual/get-started/project-configuration/version-control/versioncontrolintegration)).
4. Merge/pull conflict on `.unity`/`.prefab`. ⚠ GUI shows raw YAML conflict markers; Smart Merge needs hand-written
   `.git/config`; silent-bad-merge bugs (§3.3); fallback = "take mine/theirs" and redo work.

**Programmer**
1. Branch, code, commit hunks (normal Git). 2. Rebase/merge main. ⚠ `.meta`, `ProjectSettings`, `Packages/manifest.json`
   conflicts; scary history edits with LFS pointers. 3. Push, PR, CI. ⚠ CI clone pulls all LFS → bandwidth bill.

**Lead / tech artist**
- Set up `.gitattributes` (LFS + lockable), `.gitignore`, Force Text, Smart Merge per machine. ⚠ Per-machine config
  drift; new hires break it. Large clone for new joiners ⚠ (hours; Anchorpoint's 1 TB push took 30 h).

**Locking etiquette (observed rules):** lock before edit, only binaries + scenes/prefabs, release on push, force-unlock
logged, never restructure folders while others work ([Anchorpoint](https://docs.anchorpoint.app/git/file-locking/)). **fact.**

## 5. Opinions and data

- **Usage:** JetBrains 2025 (see §0.1). Perforce/JetBrains *Game Technology Report 2025*: 86% of 521 respondents use
  version control (69% in 2024) ([press](https://www.perforce.com/press-releases/state-of-game-technology-report-2025));
  2024 report: 54% use Git-based tools ([press 2024](https://www.perforce.com/press-releases/state-of-game-technology-report-2024)). **fact.**
- **Asset sharing is still messy:** besides VCS (48–49%), teams share art via Google Drive (33–38%), email, Dropbox
  ([JetBrains](https://lp.jetbrains.com/the-state-of-gamedev-2025/)). **fact.**
- **LFS horror stories:** collaborators blocked from cloning when the owner's LFS budget runs out; CI jobs burning
  quota ([GitHub #151710](https://github.com/orgs/community/discussions/151710),
  [#176047](https://github.com/orgs/community/discussions/176047), [ryp 2023](https://ryp.github.io/2023/git-lfs-and-github-free-tier)). **fact.**
- **HN on Lore (1,276 pts):** consensus that Git is weak at binaries/locking and Lore targets Perforce, not Git for
  code; skepticism about LLM-written docs and a binary-only desktop client
  ([HN](https://news.ycombinator.com/item?id=48571081)). **fact (paraphrase).**
- **Artists:** non-technical artists "detest Perforce", like Fork, but manual unlocking "is a lot to ask"
  ([Fork #1862](https://github.com/fork-dev/TrackerWin/issues/1862)). Content creators confused by stage/commit split
  (same thread, 2024). **fact.**
- **Reddit:** r/gamedev and r/Unity3D JSON search was blocked from this environment; not used. **open.**

## 6. How gitgud can transform it (mapped to Max's four pains)

| # | Proposal | Pain | Evidence | Label |
|---|---|---|---|---|
| T1 | **Native LFS lock panel**: `git lfs locks --verify --json` on fetch + every N min; lock badge + owner on every file row; lock/unlock from context menu; "locked by someone else" warning *before* you edit/stage | 4 | No Git GUI does this well (§2.8); API is complete (§2.9) | Claude's pick |
| T2 | **Unlock-on-push** done right: after a successful push, offer a checklist of *your* locks whose files are in the pushed commits (default ticked), never auto-silently | 4 | open since 2017 ([#2671](https://github.com/git-lfs/git-lfs/issues/2671)); a Fork user argued unlocking must stay conscious ([#605](https://github.com/fork-dev/TrackerWin/issues/605)) | Claude's pick |
| T3 | **Auto-lock suggestion on first modify** of a `lockable` file (file watcher), one click to lock; refuse to stage a lockable file locked by someone else | 4 | Anchorpoint auto-lock ([docs](https://docs.anchorpoint.app/git/file-locking/)); UVCS lock-only-latest ([Smart Locks](https://docs.unity.com/en-us/unity-version-control/smart-locks)) | Claude's pick |
| T4 | **Use LFS locks, not a private lock server** — works on GitHub/GitLab/Azure/Gitea with zero infra | 4 | Hosts table §2.9; Anchorpoint's own server forces lock-in | Claude's pick |
| T5 | **Unity Smart Merge as a merge driver**, auto-detected from Unity Hub installs, configured in `.git/config` by gitgud; conflicted scenes/prefabs open in a **structural 3-way view** (GameObject/component/field), with "UnityYAMLMerge said clean — review these N changed objects" | 2 | §3.3 docs + known silent-merge bugs | Claude's pick (driver behaviour: open, spike) |
| T6 | **`.meta` guard**: stage/unstage/discard/rename asset+meta as one unit; pre-commit checks for orphan/missing meta, GUID collisions | 2, 3 | Unity meta rules §3.1 | Claude's pick |
| T7 | **Structural prefab/scene diff** (not YAML) in history and staging, GUIDs resolved to asset names | 2, 4 | No tool found that does this for Git | Claude's pick |
| T8 | **Binary previews**: image side-by-side/swipe across commits; LFS blobs fetched lazily only for the preview | 4 | Anchorpoint/Gluon/Diversion all treat previews as core (§3.5) | Claude's pick |
| T9 | **LFS-aware speed**: read status/log via `gix` without smudging; show pointer vs. hydrated state; lazy `lfs.fetchinclude` per folder ("Gluon for Git" = sparse checkout + fetchinclude UI) | 1 | Anchorpoint 1 TB numbers; Gluon model; LFS config §2.9 | Claude's pick; sparse checkout in v1: open |
| T10 | **LFS cost/health panel**: local cache size, `git lfs prune` with preview, warn when a push adds > X MB to LFS (GitHub bills full re-upload) | 1 | GitHub billing example (§2.9) | proposal |
| T11 | **Safe history edits with LFS**: preview of what a rebase/amend does to LFS pointers and locks; undo via reflog | 3 | locks are path-based, history edits don't move them (API) | proposal |
| T12 | **One-click Unity repo setup**: `.gitattributes` (LFS + lockable for `*.unity`, `*.prefab`, textures, models, audio), `.gitignore` template, Force Text check, Smart Merge driver | 4 | per-machine config drift (§4) | Claude's pick |
| T13 | **Small Unity editor package** (later): lock state in Project view + checkout-on-edit via `AssetDatabase.IsOpenForEdit`/`MakeEditable`, talking to gitgud's CLI | 4 | Unity ignores read-only; plugins are required (§3.4) | open (scope: v1 or later?) |
| T14 | **Agent-friendly locks**: gitgud CLI/MCP exposes `locks list/lock/unlock` so an agent can see "this prefab is locked by Ana" before touching it | 4 | Max's answer 5; Anchorpoint-for-Lore advertises AI-agent review ([docs](https://docs.anchorpoint.app/lore/quick-start/)) | Claude's pick |

Open questions for Max: **(a)** which host his Unity repos use (decides whether LFS locks work, §2.9);
**(b)** solo or with artists (decides T3/T13 priority); **(c)** Lore: ignore, or keep `gitgud-core` VCS-agnostic?
