import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  Commands,
  Conflict,
  GitChoice,
  GitInventory,
  HistoryPlan,
  LibrarySnapshot,
  MergeState,
  Opened,
  OpEntry,
  PlanCommit,
  ProposalSummary,
  RebaseState,
  RepoCache,
  Row,
  RowLayout,
  Status,
  TagInfo,
} from "$lib/api";

/**
 * Dev-only fake backend so the UI can be looked at in a plain browser:
 * `pnpm dev`, then open `/?mock` (working copy) or `/?mock=merge` (a conflicted merge).
 * Shift+E on a commit opens the feature/stack history plan; any other base predicts a conflict.
 */
export function installMock(scenario: string) {
  const now = Math.floor(Date.now() / 1000);
  let graphVersion = 0;
  let commitGraphBuilt = false;
  let gitChoice: GitChoice = { mode: "auto" };
  let mockTick = 0;
  const mockCache = (root: string, gb: number): RepoCache => ({
    root,
    previewBytes: Math.round(gb * 1024 ** 3),
    previewModels: Math.round(gb * 2900),
    previewTextures: Math.round(gb * 520),
    unityBytes: gb ? 14.6e6 : 0,
    metaFiles: gb ? 31204 : 0,
  });
  const mockCaches = new Map<string, RepoCache>();
  const gitInv = (): GitInventory => {
    const system = {
      path: "C:/Program Files/Git/cmd/git.exe",
      version: "2.55.0.windows.2",
      error: null,
      lfsVersion: "3.7.0",
      lfsOk: true,
      credentialHelpers: ["manager"],
      credentialManager: "2.8.0",
    };
    const embedded = {
      path: "C:/Users/Max/AppData/Local/gitgud/resources/git/cmd/git.exe",
      version: "2.56.1.windows.1",
      error: null,
      lfsVersion: "3.8.1",
      lfsOk: true,
      credentialHelpers: ["manager"],
      credentialManager: "2.9.1",
    };
    const custom =
      gitChoice.mode === "custom" ? { ...system, path: gitChoice.path, version: "2.47.1", credentialManager: null } : null;
    const pick = gitChoice.mode === "embedded" ? embedded : (custom ?? system);
    const source = gitChoice.mode === "embedded" ? "bundled" : gitChoice.mode === "custom" ? "custom" : "system";
    return {
      choice: gitChoice,
      active: { path: pick.path, version: pick.version!.split(".windows")[0], source },
      activeError: null,
      system,
      embedded,
      custom,
      minGit: "2.45.2",
      settingsPath: "C:/Users/Max/AppData/Roaming/com.happyrave.gitgud/git.json",
    };
  };
  let mockTags: TagInfo[] = [
    {
      name: "v0.4.2",
      target: "c000000000000000000000000000000000000000",
      shortTarget: "c000000",
      annotated: true,
      message: "Milestone 4 release candidate",
      time: now - 3600 * 24,
    },
  ];
  let mockProposals: ProposalSummary[] = [
    {
      id: "p-1",
      commitId: "p100000000000000000000000000000000000000",
      shortCommit: "p100000",
      baseId: "95e25c8000000000000000000000000000000000",
      message: "Tune boat steering damping",
      why: "Reduces oscillation at high speed",
      agent: "claude-code",
      status: "pending",
      createdAt: now - 180,
      updatedAt: now - 180,
      files: ["Assets/Scripts/Boat/Engine.cs"],
      riskFlags: [
        {
          kind: "missingMeta",
          path: "Assets/Scripts/Boat/Engine.cs",
          detail: "Missing companion .meta file",
        },
      ],
      comments: [],
      headMoved: false,
    },
  ];
  let mockLib: LibrarySnapshot = {
    version: 1,
    folders: [
      { id: "f-studio", name: "Studio", parentId: null, order: 0 },
      { id: "f-arcade", name: "Arcade", parentId: "f-studio", order: 0 },
    ],
    repos: [
      {
        id: "r-boat",
        name: "boat",
        path: "C:/Users/Max/Documents/Projects/boat",
        folderId: "f-arcade",
        pinned: true,
        order: 0,
        lastOpened: now - 120,
        cached: {
          path: "C:/Users/Max/Documents/Projects/boat",
          missing: false,
          branch: "master",
          detached: false,
          dirtyCount: 3,
          untrackedCount: 1,
          ahead: 1,
          behind: 0,
          fetchedAt: now - 300,
          locksHeld: 1,
          locksCheckedAt: now - 300,
          lastActivity: now - 120,
          host: "gitHub",
          isUnity: true,
          checkedAt: now - 10,
        },
      },
    ],
    tabs: { openPaths: [], activePath: null },
    undoAction: null,
    redoAction: null,
  };
  const opened = (): Opened => ({ id: 1, root: "C:/Users/Max/Documents/Projects/boat", rows: ROWS.length, graphVersion });

  const lane = (node: number, segments: RowLayout["segments"] = []): RowLayout => ({ node, segments, overflow: false });
  const through = (...lanes: number[]) => lanes.map((l) => ({ from: l, to: l, part: "through" as const }));
  const ROWS: Row[] = [
    { id: "w", shortId: "", subject: "Uncommitted changes", author: "", time: now, labels: [], wip: true, layout: lane(0, [{ from: 0, to: 0, part: "bottom" }]) },
    {
      id: "p100000000000000000000000000000000000000",
      shortId: "p100000",
      subject: "Tune boat steering damping",
      author: "claude-code",
      time: now - 180,
      labels: [{ kind: "proposal", name: "proposal/claude-code" }],
      wip: false,
      proposal: "p-1",
      layout: lane(1, [...through(0), { from: 1, to: 0, part: "bottom", stroke: "dashed" }]),
    },
    row("95e25c8", "Raise ClearShot camera rig", "Max", now - 120, [{ kind: "branch", name: "master", current: true }], lane(0, [{ from: 0, to: 0, part: "top" }, { from: 0, to: 0, part: "bottom" }])),
    row("3493748", "Move blimp camera right", "Max", now - 2460, [{ kind: "branch", name: "feature/stack", current: false }], lane(1, [...through(0), { from: 1, to: 1, part: "bottom" }])),
    row("6bb2f7d", "fixup! WIP engine thrust tweak", "Max", now - 2470, [], lane(1, [...through(0), { from: 1, to: 1, part: "top" }, { from: 1, to: 1, part: "bottom" }])),
    row("a1b2c3d", "Tune foam shader falloff", "Lea", now - 7200, [{ kind: "remote", name: "origin/master" }], lane(0, [...through(1), { from: 0, to: 0, part: "top" }, { from: 0, to: 0, part: "bottom" }])),
    row("4592a21", "WIP engine thrust tweak", "Max", now - 9000, [], lane(1, [...through(0), { from: 1, to: 1, part: "top" }, { from: 1, to: 0, part: "bottom" }])),
    row("6acf5f7", "Remove EmojiOne assets (#209)", "upstream", now - 30 * 86400, [{ kind: "tag", name: "v0.9" }], lane(0, [{ from: 0, to: 0, part: "top" }, { from: 1, to: 0, part: "top" }, { from: 0, to: 0, part: "bottom" }])),
    ...Array.from({ length: 40 }, (_, i) =>
      row(`c${String(i).padStart(6, "0")}`, `Older work ${40 - i}`, i % 3 ? "Sam" : "Lea", now - (40 + i) * 86400, [], lane(0, [{ from: 0, to: 0, part: "top" }, { from: 0, to: 0, part: "bottom" }])),
    ),
  ];
  function row(id: string, subject: string, author: string, time: number, labels: Row["labels"], layout: RowLayout): Row {
    return { id: id.padEnd(40, "0"), shortId: id, subject, author, time, labels, wip: false, layout };
  }

  let status: Status = {
    root: opened().root,
    head: { kind: "branch", name: "master" },
    entries: [
      { path: "Assets/Scripts/Boat/Engine.cs", kind: "tracked", index: "modified", worktree: "modified" },
      { path: "Assets/Art/Trees/Maps/TreeBillBoards_albedo.png", kind: "tracked", index: null, worktree: "modified" },
      { path: "Assets/Scripts/Boat/Buoyancy.cs", kind: "tracked", index: null, worktree: "deleted" },
      { path: "notes.txt", kind: "untracked", index: null, worktree: null },
    ],
  };
  let merge: MergeState | null =
    scenario === "merge"
      ? {
          theirs: "conflict/theirs",
          message: "Merge branch 'conflict/theirs' into conflict/ours",
          conflicts: [
            { path: "Assets/Prefabs/ReplayCameras.prefab", kind: "unityYaml", resolved: false },
            { path: "Assets/Scripts/Boat/Engine.cs", kind: "code", resolved: false },
            { path: "Assets/Art/Trees/Maps/TreeBillBoards_albedo.png", kind: "binary", resolved: false },
          ],
        }
      : null;
  if (merge) {
    status = { ...status, head: { kind: "branch", name: "conflict/ours" } };
  }
  const log: OpEntry[] = [
    { id: 3, time: now - 300, action: "Stage Assets/Scripts/Boat/Engine.cs", undone: false, paths: [] },
    { id: 2, time: now - 900, action: "Discard Assets/Scripts/Boat/Old.cs", undone: false, paths: ["Assets/Scripts/Boat/Old.cs"] },
    { id: 1, time: now - 1800, action: "Commit \"Raise ClearShot camera rig\"", undone: false, paths: [] },
  ];
  let rebaseState: RebaseState | null = null;

  const planCommit = (r: Row, action: PlanCommit["action"] = "pick"): PlanCommit => ({
    id: r.id,
    shortId: r.shortId,
    subject: r.subject,
    message: r.subject,
    author: r.author,
    time: r.time,
    action,
  });
  const PLAN: HistoryPlan = {
    branch: "feature/stack",
    base: ROWS[6].id,
    baseShort: ROWS[6].shortId,
    baseSubject: ROWS[6].subject,
    commits: [planCommit(ROWS[2]), planCommit(ROWS[3], "fixup"), planCommit(ROWS[5])],
    dependents: [],
    pushed: "origin/feature/stack",
  };
  /** The plan's after rows, newest first, drawn dashed above the faded commits. */
  let ghosts: { subject: string; tag: string }[] = [];
  const dashed = (segments: RowLayout["segments"]) => segments.map((s) => ({ ...s, stroke: "dashed" as const }));
  const previewRows = (): Row[] => [
    ROWS[0],
    ROWS[1],
    ...ghosts.map((g, i) => ({
      ...ROWS[2],
      id: "",
      shortId: "",
      author: "",
      labels: [],
      subject: g.subject,
      after: g.tag,
      layout: lane(1, [...through(0), ...dashed([...(i ? [{ from: 1, to: 1, part: "top" as const }] : []), { from: 1, to: 1, part: "bottom" }])]),
    })),
    ...[2, 3, 4, 5].map((i) => (i === 4 ? { ...ROWS[i], layout: lane(0, [...dashed(through(1)), ...through(2), { from: 0, to: 0, part: "top" }, { from: 0, to: 0, part: "bottom" }]) } : { ...ROWS[i], faded: true, layout: lane(2, [...through(0), ...dashed(through(1)), ...(i > 2 ? [{ from: 2, to: 2, part: "top" as const, stroke: "faded" as const }] : []), { from: 2, to: 2, part: "bottom", stroke: "faded" }]) })),
    { ...ROWS[6], layout: lane(0, [{ from: 0, to: 0, part: "top" }, ...dashed([{ from: 1, to: 0, part: "top" }]), { from: 2, to: 0, part: "top", stroke: "faded" }, { from: 0, to: 0, part: "bottom" }]) },
    ...ROWS.slice(7),
  ];
  const resolve = (path: string) => {
    const mark = (c: Conflict) => (c.path === path ? { ...c, resolved: true } : c);
    if (merge) {
      merge = { ...merge, conflicts: merge.conflicts.map(mark) };
    }
    if (rebaseState) {
      rebaseState = { ...rebaseState, conflicts: rebaseState.conflicts.map(mark) };
    }
    return opened();
  };

  const handlers: {
    [K in keyof Commands]?: (args: Commands[K]["args"]) => Commands[K]["result"] | Promise<Commands[K]["result"]>;
  } = {
    git_info: () => ({ path: "C:/Program Files/Git/cmd/git.exe", version: "2.56.1" }),
    launch_path: () => opened().root,
    open_repo: () => opened(),
    close_repo: () => null,
    refresh_repo: () => opened(),
    repo_status: () => status,
    merge_state: () => merge,
    rebase_state: () => rebaseState,
    predict_rebase: () => ({
      commits: 3,
      behind: 1,
      conflict: { step: 1, shortId: "4592a21", subject: "WIP engine thrust tweak", paths: ["Assets/Scripts/Boat/Engine.cs"] },
    }),
    rebase: () => {
      rebaseState = {
        branch: "feature/stack",
        onto: "master",
        step: 1,
        total: 3,
        subject: "WIP engine thrust tweak",
        conflicts: [{ path: "Assets/Scripts/Boat/Engine.cs", kind: "code", resolved: false }],
        editing: false,
      };
      return opened();
    },
    rebase_step: () => {
      rebaseState = null;
      return opened();
    },
    history_plan: () => PLAN,
    plan_preview: ({ onto, steps }) => {
      const kept = steps.filter((s) => s.action !== "drop" && s.action !== "squash" && s.action !== "fixup");
      ghosts = kept.map((s) => {
        const c = PLAN.commits.find((p) => p.id === s.id)!;
        const tags = [s.action === "reword" && s.message ? "reworded" : "", onto !== PLAN.base && c.shortId === "4592a21" ? "⚠ conflict" : ""];
        return { subject: (s.message ?? c.subject).split("\n")[0], tag: tags.filter(Boolean).join(", ") };
      });
      let n = 0;
      return {
        prediction: {
          steps: steps.map((s) => {
            if (s.action === "drop") {
              return { id: s.id, step: null, outcome: { kind: "dropped" as const } };
            }
            n++;
            const conflict = onto !== PLAN.base && s.id.startsWith("4592a21");
            return { id: s.id, step: steps.filter((t) => t.action !== "drop").length - n + 1, outcome: conflict ? { kind: "conflict" as const, paths: ["Assets/Scripts/Boat/Engine.cs"], tree: "t" } : { kind: "clean" as const } };
          }),
          before: steps.length,
          after: kept.length,
          changed: onto !== PLAN.base ? null : steps.some((s) => s.action === "drop") ? 2 : 0,
        },
        rows: previewRows().length,
      };
    },
    preview_rows: ({ start, count }) => previewRows().slice(start, start + count),
    predicted_file: () => "public float thrust;\n<<<<<<< master\n    var thrust = torque.Lookup(rpm);\n=======\n    var thrust = power.Evaluate(rpm) * horsePower;\n>>>>>>> 4592a21 (WIP engine thrust tweak)\n}\n",
    run_plan: () => opened(),
    reword_head: () => opened(),
    quick_edit: () => opened(),
    op_log: () => log,
    graph_rows: ({ start, count }) => ROWS.slice(start, start + count),
    graph_find: ({ query }) =>
      ROWS.flatMap((r, i) => (!r.wip && `${r.subject} ${r.author}`.toLowerCase().includes(query.toLowerCase()) ? [i] : [])),
    branches: () => ({
      local: [
        { name: merge ? "conflict/ours" : "master", current: true, upstream: "origin/master", ahead: 1, behind: 0 },
        { name: "conflict/theirs", current: false, upstream: null, ahead: 0, behind: 0 },
        { name: "feature/stack", current: false, upstream: "origin/feature/stack", ahead: 3, behind: 0 },
        { name: "hotfix/buoyancy", current: false, upstream: null, ahead: 0, behind: 0 },
      ],
      remotes: [{ name: "origin", branches: ["origin/master", "origin/feature/stack"] }],
      fetched: now - 720,
    }),
    worktree_file_diff: ({ path }) => ({
      path,
      binary: path.endsWith(".png"),
      hunks: [
        {
          oldStart: 3,
          oldLines: 4,
          newStart: 3,
          newLines: 5,
          lines: [
            { kind: "context", old: 3, new: 3, text: "using Unity.Collections;" },
            { kind: "context", old: 4, new: 4, text: "using Unity.Mathematics;" },
            { kind: "removed", old: 5, new: null, text: "using WaterSystem.Legacy;" },
            { kind: "added", old: null, new: 5, text: "using WaterSystem;" },
            { kind: "added", old: null, new: 6, text: "// engine uses power curve" },
            { kind: "context", old: 6, new: 7, text: "" },
          ],
        },
      ],
    }),
    commit_detail: ({ commit }) => ({
      id: commit,
      shortId: commit.slice(0, 7),
      subject: ROWS.find((r) => r.id === commit)?.subject ?? "",
      body: "Moves the rig up so the camera clears the pier.",
      author: { name: "Max", email: "max@example.invalid", time: now - 120 },
      committer: { name: "Max", email: "max@example.invalid", time: now - 120 },
      parents: ["a1b2c3d".padEnd(40, "0")],
      files: [
        { path: "Assets/Prefabs/ReplayCameras.prefab", change: "modified" },
        { path: "Assets/Scripts/Camera/ClearShot.cs", change: "added" },
      ],
    }),
    commit_file_diff: ({ path }) => ({
      path,
      binary: false,
      hunks: [{ oldStart: 117, oldLines: 2, newStart: 117, newLines: 2, lines: [
        { kind: "context", old: 117, new: 117, text: "  m_GameObject: {fileID: 2671655050577180351}" },
        { kind: "removed", old: 118, new: null, text: "  m_LocalPosition: {x: 4.409273, y: 38.868496, z: 72.38463}" },
        { kind: "added", old: null, new: 118, text: "  m_LocalPosition: {x: 5.0, y: 38.868496, z: 72.38463}" },
      ] }],
    }),
    predict_merge: ({ branch }) => (branch.includes("theirs") ? ["Assets/Scripts/Boat/Engine.cs", "Assets/Prefabs/ReplayCameras.prefab"] : []),
    merge: () => opened(),
    conflict_text: ({ path }) => ({
      finalNewline: true,
      chunks: path.endsWith(".prefab")
        ? [
            { kind: "same", lines: ["--- !u!4 &571479311832868573", "Transform:", "  m_GameObject: {fileID: 2671655050577180351}"] },
            { kind: "conflict", yours: ["  m_LocalPosition: {x: 1.0, y: 38.868496, z: 72.38463}"], base: ["  m_LocalPosition: {x: 4.409273, y: 38.868496, z: 72.38463}"], theirs: ["  m_LocalPosition: {x: 2.0, y: 38.868496, z: 72.38463}"] },
            { kind: "same", lines: ["  m_LocalScale: {x: 1, y: 1, z: 1}"] },
          ]
        : [
            { kind: "same", lines: ["using UnityEngine;", "using Unity.Collections;", "using Unity.Mathematics;", "using WaterSystem;"] },
            { kind: "conflict", yours: ["// ours: engine uses power curve"], base: [], theirs: ["// theirs: engine uses torque table"] },
            { kind: "same", lines: ["", "namespace BoatAttack", "{", "    public class Engine : MonoBehaviour", "    {", "        public float horsePower = 18f;", "        public AnimationCurve torqueCurve;", "        public float steeringTorque = 5f;", "        public float idleRpm = 900f;", "        public float maxRpm = 6200f;", "        private Rigidbody rb;"] },
            { kind: "conflict", yours: ["        var thrust = power.Evaluate(rpm) * horsePower;"], base: ["        var thrust = horsePower;"], theirs: ["        var thrust = torque.Lookup(rpm) * gearRatio;", "        thrust *= efficiency;"] },
            { kind: "same", lines: ["        rb.AddForce(transform.forward * thrust);", "    }", "}"] },
          ],
    }),
    conflict_sides: () => ({
      yours: { size: 2_034_112, lfs: true, oid: "a".repeat(64), hydrated: true },
      theirs: { size: 2_076_180, lfs: true, oid: "b".repeat(64), hydrated: true },
    }),
    resolve_with: ({ path }) => resolve(path),
    resolve_side: ({ path }) => resolve(path),
    merge_commit: () => {
      merge = null;
      graphVersion++;
      return opened();
    },
    merge_abort: () => {
      merge = null;
      return opened();
    },
    stage: () => opened(),
    unstage: () => opened(),
    discard: () => opened(),
    stage_hunk: () => opened(),
    unstage_hunk: () => opened(),
    discard_hunk: () => opened(),
    stage_lines: () => opened(),
    unstage_lines: () => opened(),
    discard_lines: () => opened(),
    commit: () => opened(),
    checkout: () => opened(),
    undo: () => ({ opened: opened(), action: log[0].action }),
    redo: () => ({ opened: opened(), action: log[0].action }),
    restore_to: () => ({ opened: opened(), action: log[1].action }),
    restore_files: () => opened(),
    fetch: () => new Promise<Opened>((r) => setTimeout(() => r(opened()), 1500)),
    cancel_fetch: () => true,
    cancel_task: () => true,
    pull: () => ({ opened: opened(), outcome: { kind: "fastForwarded" } }),
    push_info: () => ({
      branch: "master",
      remote: "origin",
      remoteBranch: "master",
      upstream: "origin/master",
      ahead: 1,
      behind: 0,
      needsForce: false,
      commits: [{ shortId: "95e25c8", subject: "Raise ClearShot camera rig" }],
      remotes: ["origin"],
      prUrl: "https://github.com/Max/boat/pull/new/master",
    }),
    push: () => opened(),
    delete_remote_branch: () => opened(),
    create_branch: () => opened(),
    rename_branch: () => opened(),
    delete_branch: () => opened(),
    set_upstream: () => opened(),
    checkout_remote: () => opened(),
    head_commit_message: () => "Raise ClearShot camera rig",
    reset: () => opened(),
    predict_cherry_pick: () => [],
    predict_revert: () => [],
    cherry_pick: () => opened(),
    revert_commit: () => opened(),
    sequencer_state: () => null,
    sequencer_step: () => opened(),
    stashes: () => [],
    stash_detail: () => ({
      id: "0".repeat(40),
      shortId: "0000000",
      subject: "WIP on master",
      body: "",
      author: { name: "Max", email: "max@example.invalid", time: now - 300 },
      committer: { name: "Max", email: "max@example.invalid", time: now - 300 },
      parents: [],
      files: [],
    }),
    stash_file_diff: ({ path }) => ({ path, binary: false, hunks: [] }),
    stash_save: () => opened(),
    stash_apply: () => opened(),
    stash_pop: () => opened(),
    stash_drop: () => opened(),
    config_get_identity: ({ id }) => ({
      userName: id != null ? "Max Local" : "Max",
      userEmail: "max@example.invalid",
    }),
    config_set_identity: () => null,
    tool_diagnostics: () => ({
      gitPath: "C:/Program Files/Git/cmd/git.exe",
      gitVersion: "2.56.1",
      lfsVersion: "git-lfs/3.6.0",
    }),
    git_inventory: () => gitInv(),
    git_set_choice: ({ choice }) => {
      gitChoice = choice;
      return gitInv();
    },
    git_latest: () => ({ version: "2.56.1.windows.1", systemBehind: true, embeddedBehind: false }),
    git_update_system: () => ({ kind: "installerStarted", message: "" }),
    git_install_system: () => null,
    git_config_compare: () => [
      { scope: "system", key: "http.sslbackend", system: ["openssl"], embedded: ["schannel"], differs: true },
      { scope: "system", key: "core.editor", system: ["code --wait"], embedded: [], differs: true },
      { scope: "system", key: "core.autocrlf", system: ["true"], embedded: ["true"], differs: false },
      { scope: "global", key: "user.name", system: ["Max"], embedded: ["Max"], differs: false },
    ],
    git_check_remote: () => ({
      remote: "origin",
      url: "git@github.com:j2studio/boat.git",
      ok: true,
      elapsedMs: 412,
      message: null,
    }),
    app_stats: () => {
      mockTick += 1;
      const spike = mockTick % 25 > 18 ? 55 + ((mockTick * 37) % 40) : 0;
      return {
        cpu: Math.min(100, 1 + ((mockTick * 7) % 4) + spike),
        appBytes: 138e6,
        viewBytes: 296e6 + spike * 4e6,
        gitBytes: spike ? 24e6 : 0,
      };
    },
    repo_caches: ({ roots }) => roots.map((root) => mockCaches.get(root) ?? mockCache(root, 0.6 + (root.length % 7) / 10)),
    clear_repo_caches: ({ root }) => {
      mockCaches.set(root, mockCache(root, 0));
      return mockCaches.get(root)!;
    },
    unity_info: () => ({
      isUnity: true,
      projectRoot: opened().root,
      editorOpen: false,
      editorVersion: "6000.0.12f1",
      smartMergeConfigured: false,
      smartMergeToolPath: "C:/Program Files/Unity/Hub/Editor/6000.0.12f1/Editor/Data/Tools/UnityYAMLMerge.exe",
      detectedEditors: ["C:/Program Files/Unity/Hub/Editor/6000.0.12f1/Editor/Data/Tools/UnityYAMLMerge.exe"],
    }),
    unity_configure_smart_merge: () => ({
      isUnity: true,
      projectRoot: opened().root,
      editorOpen: false,
      editorVersion: "6000.0.12f1",
      smartMergeConfigured: true,
      smartMergeToolPath: "C:/Program Files/Unity/Hub/Editor/6000.0.12f1/Editor/Data/Tools/UnityYAMLMerge.exe",
      detectedEditors: ["C:/Program Files/Unity/Hub/Editor/6000.0.12f1/Editor/Data/Tools/UnityYAMLMerge.exe"],
    }),
    unity_meta_issues: () => [],
    unity_remerge_conflicts: () => opened(),
    unity_confirm_verify: () => opened(),
    unity_check_open_assets: () => false,
    lfs_info: () => ({
      installed: true,
      version: "git-lfs/3.6.0",
      trackedPatterns: ["*.png", "*.psd", "*.tga", "*.fbx"],
      supportsLocks: true,
      locksUnsupportedReason: null,
    }),
    lfs_locks: () => [
      { id: "101", path: "Assets/Art/Terrain.psd", owner: "Max", lockedAt: "2026-11-22T12:00:00Z", ours: true },
    ],
    lfs_lock: () => null,
    lfs_unlock: () => null,
    lfs_locks_in_push: () => [
      { id: "101", path: "Assets/Art/Terrain.psd", owner: "Max", lockedAt: "2026-11-22T12:00:00Z", ours: true },
    ],
    background_refresh: () => false,
    tags: () => [...mockTags],
    create_tag: ({ name, target, message }) => {
      const hex = (target ?? "c000000000000000000000000000000000000000").padEnd(40, "0");
      mockTags = [
        {
          name,
          target: hex,
          shortTarget: hex.slice(0, 7),
          annotated: Boolean(message?.trim()),
          message: message?.trim() || null,
          time: Math.floor(Date.now() / 1000),
        },
        ...mockTags.filter((t) => t.name !== name),
      ];
      graphVersion += 1;
      return opened();
    },
    delete_tag: ({ name }) => {
      mockTags = mockTags.filter((t) => t.name !== name);
      graphVersion += 1;
      return opened();
    },
    push_tag: () => null,
    delete_remote_tag: () => null,
    file_history: ({ path }) => [
      {
        id: "c000000000000000000000000000000000000000",
        shortId: "c000000",
        subject: `Refine ${path}`,
        author: "Max",
        time: now - 3600 * 4,
        change: "modified",
      },
      {
        id: "b000000000000000000000000000000000000000",
        shortId: "b000000",
        subject: `Add ${path}`,
        author: "Max",
        time: now - 3600 * 48,
        change: "added",
      },
    ],
    blame_file: ({ path }) => [
      {
        lineNo: 1,
        commit: "b000000000000000000000000000000000000000",
        shortCommit: "b000000",
        author: "Max",
        time: now - 3600 * 48,
        summary: `Add ${path}`,
        text: "using UnityEngine;",
      },
      {
        lineNo: 2,
        commit: "b000000000000000000000000000000000000000",
        shortCommit: "b000000",
        author: "Max",
        time: now - 3600 * 48,
        summary: `Add ${path}`,
        text: "",
      },
      {
        lineNo: 3,
        commit: "c000000000000000000000000000000000000000",
        shortCommit: "c000000",
        author: "Max",
        time: now - 3600 * 4,
        summary: `Refine ${path}`,
        text: "public sealed class BoatController : MonoBehaviour {}",
      },
    ],
    op_log_stats: () => ({
      entryCount: log.length,
      totalBytes: log.length * 512,
      prunableCount: 0,
      reclaimableBytes: 0,
    }),
    op_log_prune: () => ({
      entryCount: log.length,
      totalBytes: log.length * 512,
      prunableCount: 0,
      reclaimableBytes: 0,
    }),
    outside_reflog: () => [
      {
        id: "c000000000000000000000000000000000000000",
        shortId: "c000000",
        selector: "HEAD@{0}",
        action: "commit: Polish physics dampening",
        time: now - 1800,
      },
    ],
    has_commit_graph: () => commitGraphBuilt,
    write_commit_graph: () => {
      commitGraphBuilt = true;
      return null;
    },
    worktree_unity_diff: ({ path }) => ({
      path,
      addedObjects: 0,
      removedObjects: 0,
      modifiedProperties: 2,
      objects: [
        {
          fileId: "400001",
          objectPath: "Boat / Hull",
          component: "Transform",
          kind: "modified",
          properties: [
            {
              property: "m_LocalPosition",
              before: "{x: 0, y: 0, z: 0}",
              after: "{x: 0, y: 1.5, z: 0}",
              beforeDisplay: "{x: 0, y: 0, z: 0}",
              afterDisplay: "{x: 0, y: 1.5, z: 0}",
            },
          ],
        },
        {
          fileId: "1140002",
          objectPath: "Boat",
          component: "BoatController.cs",
          kind: "modified",
          properties: [
            {
              property: "maxSpeed",
              before: "14",
              after: "18",
              beforeDisplay: "14",
              afterDisplay: "18",
            },
          ],
        },
      ],
    }),
    commit_unity_diff: ({ path }) => ({
      path,
      addedObjects: 0,
      removedObjects: 0,
      modifiedProperties: 1,
      objects: [
        {
          fileId: "1140002",
          objectPath: "Boat",
          component: "BoatController.cs",
          kind: "modified",
          properties: [
            {
              property: "maxSpeed",
              before: "12",
              after: "14",
              beforeDisplay: "12",
              afterDisplay: "14",
            },
          ],
        },
      ],
    }),
    unity_conflict_table: ({ path }) => ({
      path,
      conflicts: [
        {
          id: "1140002::maxSpeed",
          fileId: "1140002",
          objectPath: "Boat / Engine",
          component: "BoatController.cs",
          property: "maxSpeed",
          base: "12",
          yours: "18",
          theirs: "22",
          baseDisplay: "12",
          yoursDisplay: "18",
          theirsDisplay: "22",
        },
      ],
      autoMerged: [
        {
          fileId: "400001",
          objectPath: "Boat / Hull",
          component: "Transform",
          property: "m_LocalRotation.y",
          side: "theirs",
          value: "0.707",
          display: "0.707",
        },
      ],
    }),
    resolve_unity_conflict: () => opened(),
    unity_post_merge_check: () => [],
    unity_repo_setup_status: () => ({
      isUnity: true,
      hasGitignore: true,
      gitignoreMissing: [],
      hasGitattributes: true,
      gitattributesMissing: ["*.png filter=lfs diff=lfs merge=lfs -text"],
      forceTextOk: true,
      serializationMode: 2,
    }),
    unity_apply_repo_setup: () => ({
      isUnity: true,
      hasGitignore: true,
      gitignoreMissing: [],
      hasGitattributes: true,
      gitattributesMissing: [],
      forceTextOk: true,
      serializationMode: 2,
    }),
    proposals_list: () => mockProposals.map((p) => ({ ...p, comments: [...p.comments] })),
    proposal_file_diff: ({ path }) => ({
      path,
      binary: false,
      hunks: [
        {
          oldStart: 12,
          oldLines: 4,
          newStart: 12,
          newLines: 5,
          lines: [
            { kind: "context", old: 12, new: 12, text: "    float rudder = Input.GetAxis(\"Horizontal\");" },
            { kind: "removed", old: 13, new: null, text: "    float damping = 0.8f;" },
            { kind: "added", old: null, new: 13, text: "    float damping = 1.4f;" },
            { kind: "added", old: null, new: 14, text: "    rudder *= Mathf.Clamp01(1f - speed * 0.02f);" },
            { kind: "context", old: 14, new: 15, text: "    ApplySteer(rudder, damping);" },
          ],
        },
      ],
    }),
    proposal_approve: ({ proposalId }) => {
      mockProposals = mockProposals.map((p) => (p.id === proposalId ? { ...p, status: "approved", updatedAt: now } : p));
      graphVersion += 1;
      return opened();
    },
    proposal_request_changes: ({ proposalId, comment, path }) => {
      let updated: ProposalSummary | null = null;
      mockProposals = mockProposals.map((p) => {
        if (p.id !== proposalId) {
          return p;
        }
        updated = {
          ...p,
          status: "changesRequested",
          updatedAt: now,
          comments: [
            ...p.comments,
            { id: `c-${p.comments.length + 1}`, author: "Max", text: comment, path: path ?? null, time: now },
          ],
        };
        return updated;
      });
      if (!updated) {
        throw `Unknown proposal ${proposalId}`;
      }
      return updated;
    },
    proposal_reject: ({ proposalId }) => {
      mockProposals = mockProposals.map((p) => (p.id === proposalId ? { ...p, status: "rejected", updatedAt: now } : p));
      graphVersion += 1;
      return opened();
    },
    pr_url: ({ branch }) => `https://github.com/Max/boat/pull/new/${branch}`,
    pr_url_with_base: ({ branch, base }) =>
      base ? `https://github.com/Max/boat/compare/${base}...${branch}?expand=1` : `https://github.com/Max/boat/pull/new/${branch}`,
    branch_stacks: () => [
      {
        root: "master",
        levels: [
          {
            branch: "feature/camera-rig",
            base: "master",
            commits: 1,
            current: false,
            needsRestack: true,
            prUrl: "https://github.com/Max/boat/compare/master...feature/camera-rig?expand=1",
          },
          {
            branch: "feature/stack",
            base: "feature/camera-rig",
            commits: 2,
            current: false,
            needsRestack: true,
            prUrl: "https://github.com/Max/boat/compare/feature/camera-rig...feature/stack?expand=1",
          },
        ],
      },
    ],
    restack: () => {
      graphVersion += 1;
      return opened();
    },
    lfs_cache_stats: () => ({ objectCount: 4, totalBytes: 12_500_000, prunableCount: 2, prunableBytes: 4_200_000 }),
    lfs_prune: () => ({ objectCount: 2, totalBytes: 8_300_000, prunableCount: 0, prunableBytes: 0 }),
    lfs_fetch_object: () => true,
    ai_providers: () => ({ cliAvailable: ["claude"], configuredEnvKey: false }),
    ai_draft_commit_message: () => "Tune engine torque curve for rough waves",
    agent_setup: ({ target, withHooks }) => {
      const base =
        target === "claude"
          ? ".claude/skills/gitgud/SKILL.md"
          : target === "codex"
            ? ".codex/skills/gitgud/SKILL.md"
            : target === "cursor"
              ? ".cursor/rules/gitgud.mdc"
              : "GITGUD_AGENT.md";
      return target === "claude" && withHooks ? `${base} + .claude/settings.local.json` : base;
    },
    stale_origin_commits: () => ["4592a21000000000000000000000000000000000"],
    refresh_index: () => opened(),
    recent_timings: () => [
      { op: "status", elapsedMs: 14, at: now - 10 },
      { op: "graph_rows", elapsedMs: 6, at: now - 20 },
      { op: "open", elapsedMs: 42, at: now - 60 },
    ],
    worktrees_list: () => [
      {
        path: "C:/Users/Max/Documents/Projects/boat-agent",
        branch: "agent/steering",
        head: "95e25c8000000000000000000000000000000000",
        dirtyCount: 2,
        isMain: false,
      },
    ],
    propose_from_worktree: ({ worktreePath, message, why, agent }) => {
      const created: ProposalSummary = {
        id: `p-wt-${mockProposals.length + 1}`,
        commitId: "e".repeat(40),
        shortCommit: "eeeeeee",
        baseId: "95e25c8000000000000000000000000000000000",
        message,
        why: why ?? `Imported from ${worktreePath}`,
        agent: agent ?? "worktree",
        status: "pending",
        createdAt: now,
        updatedAt: now,
        files: ["Assets/Scripts/Boat/Steering.cs"],
        riskFlags: [],
        comments: [],
        headMoved: false,
      };
      mockProposals = [created, ...mockProposals];
      graphVersion += 1;
      return created;
    },
    open_external_url: () => null,
    update_check: ({ manual }) =>
      manual
        ? {
            version: "9.9.9",
            current: "0.1.3",
            notes: "A fake release from the mock backend.",
            portable: false,
            page: "https://github.com/HappyRave/gitgud/releases",
          }
        : null,
    update_install: () => new Promise(() => {}),
    library_load: () => mockLib,
    library_summaries: () => mockLib.repos.map((r) => r.cached!).filter(Boolean),
    library_add_repos: ({ entries }) => {
      for (const e of entries) {
        mockLib = {
          ...mockLib,
          repos: [
            ...mockLib.repos,
            {
              id: `r-${Date.now()}`,
              name: e.name ?? e.path.split(/[\\/]/).pop() ?? e.path,
              path: e.path,
              folderId: null,
              pinned: false,
              order: mockLib.repos.length,
              lastOpened: null,
              cached: null,
            },
          ],
          undoAction: "Add repository",
        };
      }
      return mockLib;
    },
    library_remove_repo: ({ id }) => {
      mockLib = { ...mockLib, repos: mockLib.repos.filter((r) => r.id !== id), undoAction: "Remove repository" };
      return mockLib;
    },
    library_locate_repo: ({ id, newPath }) => {
      mockLib = {
        ...mockLib,
        repos: mockLib.repos.map((r) => (r.id === id ? { ...r, path: newPath } : r)),
        undoAction: "Locate repository",
      };
      return mockLib;
    },
    library_move_repo: ({ id, folderId, order }) => {
      mockLib = {
        ...mockLib,
        repos: mockLib.repos.map((r) => (r.id === id ? { ...r, folderId, order } : r)),
        undoAction: "Move repository",
      };
      return mockLib;
    },
    library_pin_repo: ({ id, pinned }) => {
      mockLib = {
        ...mockLib,
        repos: mockLib.repos.map((r) => (r.id === id ? { ...r, pinned } : r)),
        undoAction: pinned ? "Pin" : "Unpin",
      };
      return mockLib;
    },
    library_create_folder: ({ name, parentId }) => {
      mockLib = {
        ...mockLib,
        folders: [...mockLib.folders, { id: `f-${Date.now()}`, name, parentId, order: mockLib.folders.length }],
        undoAction: `Create folder ${name}`,
      };
      return mockLib;
    },
    library_rename_folder: ({ id, name }) => {
      mockLib = {
        ...mockLib,
        folders: mockLib.folders.map((f) => (f.id === id ? { ...f, name } : f)),
        undoAction: `Rename folder`,
      };
      return mockLib;
    },
    library_delete_folder: ({ id }) => {
      mockLib = { ...mockLib, folders: mockLib.folders.filter((f) => f.id !== id), undoAction: "Delete folder" };
      return mockLib;
    },
    library_move_folder: ({ id, parentId, order }) => {
      mockLib = {
        ...mockLib,
        folders: mockLib.folders.map((f) => (f.id === id ? { ...f, parentId, order } : f)),
        undoAction: "Move folder",
      };
      return mockLib;
    },
    library_save_tabs: ({ tabs }) => {
      mockLib = { ...mockLib, tabs };
      return mockLib;
    },
    library_undo: () => mockLib,
    library_redo: () => mockLib,
    scan_folder: ({ root }) => ({
      root,
      totalFound: 1,
      elapsedMs: 12,
      groups: [
        {
          dir: root,
          suggestedFolder: "Projects",
          repos: [{ name: "sample-game", path: `${root}/sample-game`, alreadyInLibrary: false }],
        },
      ],
    }),
    clone_repo: ({ dest }) => dest,
    cancel_clone: () => true,
    fetch_many: () => mockLib.repos.map((r) => r.cached!).filter(Boolean),
    "plugin:window|set_title": () => null,
  };

  mockIPC(
    (cmd, args) => {
      const handler = handlers[cmd as keyof Commands] as ((a: unknown) => unknown) | undefined;
      if (!handler) {
        throw new Error(`mock: no handler for ${cmd}`);
      }
      return handler(args);
    },
    { shouldMockEvents: true },
  );
}
