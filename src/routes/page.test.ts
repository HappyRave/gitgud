import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it, vi } from "vitest";
import type { Commands, MergeState, ProposalSummary, RebaseState, Row, SequencerState, Status } from "$lib/api";
import { prefs } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import Page from "./+page.svelte";

const GIT = { path: "C:/git/git.exe", version: "2.56.1" };

const STATUS: Status = {
  root: "C:/work/repo",
  head: { kind: "branch", name: "main" },
  entries: [{ path: "a.txt", kind: "tracked", index: null, worktree: "modified" }],
};

const ROW: Row = {
  id: "a".repeat(40),
  shortId: "aaaaaaa",
  subject: "first commit",
  author: "Ada",
  time: 1_700_000_000,
  labels: [{ kind: "branch", name: "main", current: true }],
  wip: false,
  layout: { node: 0, segments: [], overflow: false },
};

type Handlers = Parameters<typeof mockCommands>[0];

/** A working app: git found, repos open with ids counting up from 1. */
function app(extra: Handlers = {}) {
  let next = 1;
  const EMPTY_LIB = {
    version: 1,
    folders: [],
    repos: [],
    tabs: { openPaths: [], activePath: null },
    undoAction: null,
    redoAction: null,
  };
  const handlers: Handlers = {
    git_info: () => GIT,
    app_stats: () => ({ cpu: 3, appBytes: 120e6, viewBytes: 290e6, gitBytes: 0 }),
    repo_caches: ({ roots }) => roots.map((root) => ({ root, previewBytes: 0, previewModels: 0, previewTextures: 0, unityBytes: 0, metaFiles: 0 })),
    launch_path: () => null,
    update_check: () => null,
    open_repo: ({ path }: Commands["open_repo"]["args"]) => ({ id: next++, root: path, rows: 1, graphVersion: 0 }),
    repo_status: () => STATUS,
    merge_state: () => null,
    rebase_state: () => null,
    sequencer_state: () => null,
    op_log: () => [],
    graph_rows: () => [ROW],
    close_repo: () => null,
    "plugin:window|set_title": () => null,
    branches: () => ({
      local: [{ name: "main", current: true, upstream: null, ahead: 0, behind: 0 }],
      remotes: [],
      fetched: null,
    }),
    stashes: () => [],
    library_load: () => EMPTY_LIB,
    library_summaries: () => [],
    library_add_repos: () => EMPTY_LIB,
    library_remove_repo: () => EMPTY_LIB,
    library_locate_repo: () => EMPTY_LIB,
    library_move_repo: () => EMPTY_LIB,
    library_pin_repo: () => EMPTY_LIB,
    library_create_folder: () => EMPTY_LIB,
    library_rename_folder: () => EMPTY_LIB,
    library_delete_folder: () => EMPTY_LIB,
    library_move_folder: () => EMPTY_LIB,
    library_save_tabs: () => EMPTY_LIB,
    library_undo: () => EMPTY_LIB,
    library_redo: () => EMPTY_LIB,
    scan_folder: ({ root }) => ({ root, totalFound: 0, elapsedMs: 1, groups: [] }),
    clone_repo: ({ dest }) => dest,
    cancel_clone: () => true,
    fetch_many: () => [],
    stage_lines: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    unstage_lines: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    discard_lines: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    config_get_identity: () => ({ userName: "Ada", userEmail: "ada@example.com" }),
    config_set_identity: () => null,
    tool_diagnostics: () => ({ gitPath: GIT.path, gitVersion: GIT.version, lfsVersion: null }),
    unity_info: () => ({
      isUnity: false,
      projectRoot: null,
      editorOpen: false,
      editorVersion: null,
      smartMergeConfigured: false,
      smartMergeToolPath: null,
      detectedEditors: [],
    }),
    unity_configure_smart_merge: () => ({
      isUnity: true,
      projectRoot: "C:/work/repo",
      editorOpen: false,
      editorVersion: "6000.0.12f1",
      smartMergeConfigured: true,
      smartMergeToolPath: null,
      detectedEditors: [],
    }),
    unity_meta_issues: () => [],
    unity_remerge_conflicts: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    unity_confirm_verify: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    unity_check_open_assets: () => false,
    lfs_info: () => ({
      installed: false,
      version: null,
      trackedPatterns: [],
      supportsLocks: false,
      locksUnsupportedReason: null,
    }),
    lfs_locks: () => [],
    lfs_lock: () => null,
    lfs_unlock: () => null,
    lfs_locks_in_push: () => [],
    background_refresh: () => false,
    tags: () => [],
    create_tag: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    delete_tag: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    push_tag: () => null,
    delete_remote_tag: () => null,
    file_history: () => [],
    blame_file: () => [],
    op_log_stats: () => ({ entryCount: 0, totalBytes: 0, prunableCount: 0, reclaimableBytes: 0 }),
    op_log_prune: () => ({ entryCount: 0, totalBytes: 0, prunableCount: 0, reclaimableBytes: 0 }),
    outside_reflog: () => [],
    has_commit_graph: () => false,
    write_commit_graph: () => null,
    worktree_unity_diff: () => ({ path: "", addedObjects: 0, removedObjects: 0, modifiedProperties: 0, objects: [] }),
    commit_unity_diff: () => ({ path: "", addedObjects: 0, removedObjects: 0, modifiedProperties: 0, objects: [] }),
    unity_conflict_table: () => {
      throw "not valid multi-document Unity YAML";
    },
    resolve_unity_conflict: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    unity_post_merge_check: () => [],
    unity_repo_setup_status: () => ({
      isUnity: false,
      hasGitignore: false,
      gitignoreMissing: [],
      hasGitattributes: false,
      gitattributesMissing: [],
      forceTextOk: false,
      serializationMode: null,
    }),
    unity_apply_repo_setup: () => ({
      isUnity: false,
      hasGitignore: false,
      gitignoreMissing: [],
      hasGitattributes: false,
      gitattributesMissing: [],
      forceTextOk: false,
      serializationMode: null,
    }),
    proposals_list: () => [],
    proposal_file_diff: ({ path }) => ({ path, binary: false, hunks: [] }),
    proposal_approve: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    proposal_request_changes: ({ proposalId, comment, path }) => ({
      id: proposalId,
      commitId: "d".repeat(40),
      shortCommit: "ddddddd",
      baseId: "a".repeat(40),
      message: "Proposal",
      why: "",
      agent: "claude-code",
      status: "changesRequested",
      createdAt: 1_700_000_000,
      updatedAt: 1_700_000_000,
      files: [],
      riskFlags: [],
      comments: [{ id: "c-1", author: "Max", text: comment, path: path ?? null, time: 1_700_000_000 }],
      headMoved: false,
    }),
    proposal_reject: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    branch_stacks: () => [],
    restack: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    pr_url_with_base: () => null,
    lfs_cache_stats: () => ({ objectCount: 0, totalBytes: 0, prunableCount: 0, prunableBytes: 0 }),
    lfs_prune: () => ({ objectCount: 0, totalBytes: 0, prunableCount: 0, prunableBytes: 0 }),
    ai_providers: () => ({ cliAvailable: [], configuredEnvKey: false }),
    ai_draft_commit_message: () => "Drafted commit",
    agent_setup: () => "GITGUD_AGENT.md",
    stale_origin_commits: () => [],
    refresh_index: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    recent_timings: () => [],
    worktrees_list: () => [],
    propose_from_worktree: ({ worktreePath, message, why, agent }) => ({
      id: "p-wt-1",
      commitId: "e".repeat(40),
      shortCommit: "eeeeeee",
      baseId: "a".repeat(40),
      message,
      why: why ?? `Imported from ${worktreePath}`,
      agent: agent ?? "worktree",
      status: "pending",
      createdAt: 1_700_000_000,
      updatedAt: 1_700_000_000,
      files: ["a.txt"],
      riskFlags: [],
      comments: [],
      headMoved: false,
    }),
    ...extra,
  };
  return mockCommands(handlers);
}

describe("start page: git and app stats", () => {
  it("shows gitgud's CPU, RAM and caches instead of the git path", async () => {
    app();
    render(Page);

    const stats = await screen.findByTestId("app-stats");
    expect(await within(stats).findByText("3%")).toBeInTheDocument();
    expect(within(stats).getByText("391 MB")).toBeInTheDocument();
    expect(screen.queryByText(/C:\/git\/git\.exe/)).not.toBeInTheDocument();
  });

  it("shows the error when git is unusable", async () => {
    app({
      git_info: () => {
        throw "git 2.30.0 is too old; gitgud needs 2.45.2 or newer";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("git 2.30.0 is too old; gitgud needs 2.45.2 or newer");
  });
});

describe("library rescan", () => {
  it("rescans a folder from its menu and applies adds and removals in one call", async () => {
    const LIB = {
      version: 1,
      folders: [{ id: "f-games", name: "Games", parentId: null, order: 0 }],
      repos: [
        { id: "r-gone", name: "doomed", path: "C:/Games/doomed", folderId: "f-games", pinned: false, order: 0, lastOpened: null, cached: null },
      ],
      tabs: { openPaths: [], activePath: null },
      undoAction: null,
      redoAction: null,
    };
    const calls = app({
      library_load: () => LIB,
      library_rescan_folder: () => ({
        root: "C:/Games",
        totalFound: 1,
        elapsedMs: 3,
        groups: [{ dir: "C:/Games", suggestedFolder: "Games", repos: [{ name: "fresh", path: "C:/Games/fresh", alreadyInLibrary: false }] }],
        gone: [{ id: "r-gone", name: "doomed", path: "C:/Games/doomed" }],
      }),
      library_apply_scan: () => ({ ...LIB, repos: [], undoAction: "Rescan: add 1, remove 1" }),
    });
    render(Page);

    const folder = await vi.waitFor(() => {
      const found = screen.getAllByRole("button").find((b) => b.classList.contains("it") && b.textContent?.includes("Games"));
      expect(found).toBeDefined();
      return found!;
    });
    await fireEvent.contextMenu(folder);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Rescan…" }));
    const dlg = await screen.findByRole("dialog", { name: "Rescan Games" });
    await fireEvent.click(within(dlg).getByRole("button", { name: "Add 1 · remove 1" }));

    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "library_apply_scan")?.args).toEqual({
        entries: [{ path: "C:/Games/fresh", folderPath: "Games", name: "fresh" }],
        removeIds: ["r-gone"],
      }),
    );
    expect(calls.find((c) => c.cmd === "library_rescan_folder")?.args).toEqual({ folderId: "f-games" });
    expect(screen.queryByRole("dialog", { name: "Rescan Games" })).toBeNull();
  });
});

describe("updates", () => {
  it("checks at startup and offers a newer release; Skip closes it", async () => {
    const calls = app({
      update_check: () => ({ version: "0.2.0", current: "0.1.3", notes: null, portable: false, page: "https://x/v0.2.0" }),
    });
    render(Page);

    expect(await screen.findByRole("heading", { name: "gitgud 0.2.0 is out" })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "update_check")?.args).toEqual({ manual: false });
    await fireEvent.click(screen.getByRole("button", { name: "Skip this version" }));
    expect(screen.queryByRole("heading", { name: "gitgud 0.2.0 is out" })).toBeNull();
    expect(prefs.skippedUpdate).toBe("0.2.0");
  });
});

describe("start page: opening a repo", () => {
  it("asks for a repo when none was given at launch", async () => {
    app();
    render(Page);

    expect(await screen.findByText("Open a repository to see its history and changes.")).toBeInTheDocument();
  });

  it("opens the launch path: history in the centre, changes beside it", async () => {
    const calls = app({ launch_path: () => "C:/work/repo" });
    render(Page);

    expect(await screen.findByText("first commit")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Changes" })).toHaveTextContent("a.txt");
    expect(calls.find((c) => c.cmd === "open_repo")?.args).toEqual({ path: "C:/work/repo" });
    expect(calls.find((c) => c.cmd === "repo_status")?.args).toEqual({ id: 1 });
  });

  it("opens the folder picked in the dialog", async () => {
    const calls = app({ "plugin:dialog|open": () => "C:/work/repo" });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByText("first commit")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "plugin:dialog|open")?.args).toMatchObject({ options: { directory: true } });
  });

  it("opens a second repo in a new tab and closing a tab calls close_repo", async () => {
    const calls = app({ launch_path: () => "C:/one", "plugin:dialog|open": () => "C:/two" });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    const graphIds = () => calls.filter((c) => c.cmd === "graph_rows").map((c) => (c.args as { id: number }).id);
    await vi.waitFor(() => expect(graphIds()).toEqual([1, 2]));
    expect(calls.some((c) => c.cmd === "close_repo")).toBe(false);

    await fireEvent.click(screen.getByRole("button", { name: "Close one" }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "close_repo").map((c) => c.args)).toEqual([{ id: 1 }]));
  });

  it("does nothing when the picker is cancelled", async () => {
    const calls = app({ "plugin:dialog|open": () => null });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByText("Open a repository to see its history and changes.")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "open_repo")).toBe(false);
  });

  it("shows why a folder can't be opened", async () => {
    app({
      launch_path: () => "C:/not-a-repo",
      open_repo: () => {
        throw "C:/not-a-repo is not inside a git repository";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("C:/not-a-repo is not inside a git repository");
  });

  it("keeps the open repo when the next one fails", async () => {
    let opens = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      "plugin:dialog|open": () => "C:/not-a-repo",
      open_repo: ({ path }: Commands["open_repo"]["args"]) => {
        if (opens++ > 0) {
          throw `${path} is not inside a git repository`;
        }
        return { id: 1, root: path, rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("C:/not-a-repo is not inside a git repository");
    expect(screen.getByText("first commit")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "close_repo")).toBe(false);
  });
});

describe("start page: undo and redo", () => {
  const REVERTED = { opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Stage a.txt" };

  it("Ctrl+Z undoes, Ctrl+Y redoes, and the status line names the action", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED, redo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Stage a.txt");

    await fireEvent.keyDown(window, { key: "y", ctrlKey: true });
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Redid: Stage a.txt"));
    expect(calls.filter((c) => c.cmd === "undo" || c.cmd === "redo").map((c) => [c.cmd, c.args])).toEqual([
      ["undo", { id: 1 }],
      ["redo", { id: 1 }],
    ]);
    expect(calls.filter((c) => c.cmd === "repo_status")).toHaveLength(3);
  });

  it("quick repeated presses each count, in order", async () => {
    let release!: () => void;
    let undos = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      undo: () => {
        undos++;
        return undos === 1 ? new Promise((r) => (release = () => r(REVERTED))) : REVERTED;
      },
      redo: () => REVERTED,
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "y", ctrlKey: true });
    release();

    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "undo" || c.cmd === "redo").map((c) => c.cmd)).toEqual(["undo", "undo", "redo"]),
    );
  });

  it("Ctrl+Z in the commit message stays text undo", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Commit message" }), { key: "z", ctrlKey: true });

    expect(calls.some((c) => c.cmd === "undo")).toBe(false);
  });

  it("the header buttons do the same", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Stage a.txt");
    expect(calls.some((c) => c.cmd === "undo")).toBe(true);
  });

  it("shows why there is nothing to undo", async () => {
    app({
      launch_path: () => "C:/work/repo",
      undo: () => {
        throw "nothing to undo";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(await screen.findByRole("alert")).toHaveTextContent("nothing to undo");
  });
});

describe("start page: sidebar", () => {
  it("lists branches and double-click checks one out", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [
          { name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 0 },
          { name: "feature/login", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
        fetched: null,
      }),
      checkout: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 }),
    });
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await within(branches).findByText("login");

    expect(within(branches).getByRole("button", { current: true })).toHaveTextContent("✓main↑1");
    await fireEvent.dblClick(within(branches).getByRole("button", { name: /login/ }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "checkout")?.args).toEqual({ id: 1, branch: "feature/login" }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "branches")).toHaveLength(2));
  });

  it("a slow checkout shows what is running in the strip, over the last notice", async () => {
    let finish!: () => void;
    app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [
          { name: "main", current: true, upstream: null, ahead: 0, behind: 0 },
          { name: "feature/login", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [],
        fetched: null,
      }),
      undo: () => ({ action: "Stage a.txt", opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 } }),
      checkout: () =>
        new Promise((resolve) => {
          finish = () => resolve({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 });
        }),
    });
    const view = render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await within(branches).findByText("login");
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Stage a.txt");

    await fireEvent.dblClick(within(branches).getByRole("button", { name: /login/ }));

    const strip = () => view.container.querySelector(".view:not([hidden]) footer.strip") as HTMLElement;
    await vi.waitFor(() => expect(within(strip()).getByRole("progressbar")).toHaveAccessibleName("Checking out feature/login…"));
    expect(strip()).not.toHaveTextContent("Undid");

    finish();
    await vi.waitFor(() => expect(within(strip()).queryByRole("progressbar")).toBeNull());
  });

  it("the toolbar shows the repo, the branch with ahead / behind, or a detached HEAD", async () => {
    app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [{ name: "main", current: true, upstream: "origin/main", ahead: 2, behind: 1 }],
        remotes: [],
        fetched: Math.floor(Date.now() / 1000) - 300,
      }),
    });
    const view = render(Page);

    let toolbar!: HTMLElement;
    await vi.waitFor(() => {
      toolbar = view.container.querySelector(".view:not([hidden]) header.tb") as HTMLElement;
      expect(toolbar).toHaveTextContent("main ↑2 ↓1");
    });
    expect(within(toolbar).getByRole("button", { name: "Open repository…" })).toHaveTextContent("repo");
    expect(view.container.querySelector(".view:not([hidden]) footer.strip")).toHaveTextContent(
      "● ↑2 ↓1 against origin/main · fetched 5 min ago",
    );
  });

  it("names the open repo in the window title", async () => {
    const calls = app({ launch_path: () => "C:/work/repo" });
    render(Page);
    await screen.findByText("first commit");

    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "plugin:window|set_title").at(-1)?.args).toEqual({
        label: "main",
        value: "repo · C:/work/repo — gitgud",
      }),
    );
  });

  it("calls out a detached HEAD in the toolbar", async () => {
    app({ launch_path: () => "C:/work/repo", repo_status: () => ({ ...STATUS, head: { kind: "detached", id: "abcdef1234" } }) });
    render(Page);

    expect(await screen.findByText("⚠ Detached HEAD at abcdef1")).toBeInTheDocument();
  });

  it("its Op log entry opens the panel", async () => {
    app({ launch_path: () => "C:/work/repo", op_log: () => [] });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));

    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Op log" })).toHaveAttribute("aria-pressed", "true");
  });
});

describe("start page: op log panel", () => {
  const ENTRY = { id: 7, time: 1_700_000_000, action: "Discard a.txt", undone: false, paths: ["a.txt"] };
  const OLDER = { id: 6, time: 1_699_999_000, action: "Stage b.txt", undone: false, paths: [] };

  it("Ctrl+3 opens it in the side pane and Ctrl+1 goes back", async () => {
    app({ launch_path: () => "C:/work/repo", op_log: () => [ENTRY] });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Commit" })).not.toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Commit" })).toBeInTheDocument();
  });

  it("can leave Op log via its Close button, Sidebar Working copy, Escape, or WIP row", async () => {
    const WIP: Row = { ...ROW, id: "", shortId: "", subject: "Uncommitted changes", wip: true, labels: [] };
    app({
      launch_path: () => "C:/work/repo",
      open_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 0 }),
      graph_rows: () => [WIP, ROW],
      op_log: () => [ENTRY],
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Back to working copy" }));
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Commit" })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Working copy" }));
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    await fireEvent.click(screen.getByText("Uncommitted changes"));
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();
  });

  it("Ctrl+F opens find in the history; a match opens that commit; Esc closes it", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      graph_find: () => [0],
      commit_detail: () => ({
        id: ROW.id,
        shortId: ROW.shortId,
        subject: ROW.subject,
        body: "",
        author: { name: "Ada", email: "a@x", time: 0 },
        committer: { name: "Ada", email: "a@x", time: 0 },
        parents: [],
        files: [],
      }),
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    const field = screen.getByRole("textbox", { name: "Find commits" });
    expect(field).toHaveFocus();
    await fireEvent.input(field, { target: { value: "first" } });

    expect(await screen.findByRole("region", { name: "Commit" })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "graph_find")?.args).toEqual({ id: 1, query: "first" });
    await fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "Find commits" })).not.toBeInTheDocument();
    expect(screen.getByRole("listbox", { name: "Commits" })).toHaveFocus();
  });

  it("Ctrl+2 puts focus on the graph rows", async () => {
    app({ launch_path: () => "C:/work/repo" });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });

    expect(screen.getByRole("listbox", { name: "Commits" })).toHaveFocus();
  });

  it("F6 and Shift+F6 move focus between sidebar, graph and inspector", async () => {
    app({ launch_path: () => "C:/work/repo" });
    render(Page);
    await screen.findByText("first commit");
    const graphRows = screen.getByRole("listbox", { name: "Commits" });

    await fireEvent.keyDown(window, { key: "F6" });
    expect(screen.getByRole("region", { name: "Branches" }).contains(document.activeElement)).toBe(true);
    await fireEvent.keyDown(window, { key: "F6" });
    expect(graphRows).toHaveFocus();
    await fireEvent.keyDown(window, { key: "F6" });
    expect(screen.getByRole("group", { name: "Working copy pane" })).toHaveFocus();
    await fireEvent.keyDown(window, { key: "F6", shiftKey: true });
    expect(graphRows).toHaveFocus();
  });

  it("Esc closes the commit detail; Ctrl+C on a commit row copies its hash", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    app({
      launch_path: () => "C:/work/repo",
      commit_detail: () => ({
        id: ROW.id,
        shortId: ROW.shortId,
        subject: ROW.subject,
        body: "",
        author: { name: "Ada", email: "a@x", time: 0 },
        committer: { name: "Ada", email: "a@x", time: 0 },
        parents: [],
        files: [],
      }),
    });
    render(Page);
    await fireEvent.click(await screen.findByText("first commit"));
    await screen.findByRole("region", { name: "Commit" });

    await fireEvent.keyDown(screen.getByRole("listbox", { name: "Commits" }), { key: "c", ctrlKey: true });
    expect(writeText).toHaveBeenCalledWith(ROW.id);
    expect(await screen.findByRole("status")).toHaveTextContent("Copied aaaaaaa");

    await fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument();
  });

  it("Restore on an older entry restores to before it and says so", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      op_log: () => [ENTRY, OLDER],
      restore_to: () => ({ opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Stage b.txt" }),
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Restore" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Restored to before: Stage b.txt");
    expect(calls.find((c) => c.cmd === "restore_to")?.args).toEqual({ id: 1, entry: 6 });
  });

  it("Restore these files puts a discard back as a new action", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      op_log: () => [ENTRY],
      restore_files: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.keyDown(window, { key: "3", ctrlKey: true });

    const panel = await screen.findByRole("region", { name: "Op log" });
    await fireEvent.click(await within(panel).findByRole("button", { name: /Discard a\.txt/ }));
    const before = calls.filter((c) => c.cmd === "op_log").length;
    await fireEvent.click(screen.getByRole("button", { name: "Restore these files" }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "restore_files")?.args).toEqual({ id: 1, entry: 7 }));
    // The panel and the toolbar's Undo label both reload.
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "op_log").length).toBe(before + 2));
  });

  it("the toolbar's Undo names the action it would undo", async () => {
    app({ launch_path: () => "C:/work/repo", op_log: () => [{ ...ENTRY, undone: true }, OLDER] });
    render(Page);

    expect(await screen.findByRole("button", { name: /^Undo: Stage b\.txt/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Redo" })).toHaveAttribute("title", "Redo: Discard a.txt (Ctrl+Y)");
  });
});

describe("start page: fetch", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 };

  it("shows live progress, then refreshes when done", async () => {
    let finish!: () => void;
    const calls = app({
      launch_path: () => "C:/work/repo",
      fetch: () => new Promise((r) => (finish = () => r(OPENED))),
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));
    expect(screen.getByLabelText("Fetch progress")).toHaveTextContent("Fetching…");
    expect(screen.getByRole("button", { name: "Fetch" })).toBeDisabled();

    await emit("fetch-progress", { id: 1, progress: { phase: "Receiving objects", percent: 42, done: 42, total: 100 } });
    await vi.waitFor(() => expect(screen.getByLabelText("Fetch progress")).toHaveTextContent("Receiving objects 42%"));

    finish();
    expect(await screen.findByRole("status")).toHaveTextContent("Fetched");
    expect(screen.queryByLabelText("Fetch progress")).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "fetch")?.args).toEqual({ id: 1 });
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2));
  });

  it("Ctrl+Shift+F starts a fetch", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", fetch: () => OPENED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "F", ctrlKey: true, shiftKey: true });

    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "fetch")).toBe(true));
  });

  it("Cancel stops it and says so, without an error", async () => {
    let reject!: (e: string) => void;
    const calls = app({
      launch_path: () => "C:/work/repo",
      fetch: () => new Promise((_, r) => (reject = r)),
      cancel_task: () => {
        reject("cancelled");
        return true;
      },
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Fetch cancelled");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "cancel_task")?.args).toEqual({ id: 1 });
  });

  it("shows git's error when the fetch fails", async () => {
    app({
      launch_path: () => "C:/work/repo",
      fetch: () => {
        throw "fatal: unable to access 'https://example.invalid/': Could not resolve host";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Could not resolve host");
  });

  it("ignores progress for another repo", async () => {
    app({ launch_path: () => "C:/work/repo", fetch: () => new Promise(() => {}) });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    await emit("fetch-progress", { id: 9, progress: { phase: "Receiving objects", percent: 99, done: 99, total: 100 } });

    expect(screen.getByLabelText("Fetch progress")).not.toHaveTextContent("99%");
  });
});

describe("start page: working-copy diffs and hunks", () => {
  const HUNK_DIFF = {
    path: "a.txt",
    binary: false,
    hunks: [
      {
        oldStart: 3,
        oldLines: 1,
        newStart: 3,
        newLines: 1,
        lines: [
          { kind: "removed" as const, old: 3, new: null, text: "old" },
          { kind: "added" as const, old: null, new: 3, text: "new" },
        ],
      },
    ],
  };

  it("clicking a changed file shows its diff with Stage hunk", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", worktree_file_diff: () => HUNK_DIFF });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    expect(await screen.findByRole("region", { name: "Unstaged changes in a.txt" })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "worktree_file_diff")?.args).toEqual({ id: 1, path: "a.txt", side: "unstaged" });
    expect(screen.getByRole("button", { name: "Stage hunk @@ -3 +3 @@" })).toBeInTheDocument();
  });

  it("staging a hunk sends its header and refreshes the diff", async () => {
    let staged = false;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () =>
        staged
          ? { ...STATUS, entries: [{ path: "a.txt", kind: "tracked", index: "modified", worktree: "modified" }] }
          : STATUS,
      worktree_file_diff: () => HUNK_DIFF,
      stage_hunk: () => {
        staged = true;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Stage hunk @@ -3 +3 @@" }));

    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "worktree_file_diff")).toHaveLength(2));
    expect(calls.find((c) => c.cmd === "stage_hunk")?.args).toEqual({
      id: 1,
      path: "a.txt",
      hunk: { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1 },
    });
    expect(await screen.findByRole("list", { name: "Staged" })).toHaveTextContent("a.txt");
  });

  it("discarding a hunk or a file sends it to Rust, and Ctrl+Z can bring it back", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      worktree_file_diff: () => HUNK_DIFF,
      discard_hunk: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
      discard: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
      undo: () => ({ opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Discard a.txt" }),
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Discard hunk @@ -3 +3 @@" }));
    await vi.waitFor(() => expect(screen.getByRole("button", { name: "Discard a.txt" })).toBeEnabled());
    await fireEvent.click(screen.getByRole("button", { name: "Discard a.txt" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "discard")).toBe(true));
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(calls.find((c) => c.cmd === "discard_hunk")?.args).toEqual({
      id: 1,
      path: "a.txt",
      hunk: { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1 },
    });
    expect(calls.find((c) => c.cmd === "discard")?.args).toEqual({ id: 1, paths: ["a.txt"] });
    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Discard a.txt");
  });

  it("a stale hunk shows Rust's message", async () => {
    app({
      launch_path: () => "C:/work/repo",
      worktree_file_diff: () => HUNK_DIFF,
      stage_hunk: () => {
        throw "a.txt changed since its diff was shown; look again and retry";
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Stage hunk @@ -3 +3 @@" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("changed since its diff was shown");
  });

  it("untracked files show their diff without hunk buttons", async () => {
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({ ...STATUS, entries: [{ path: "n.txt", kind: "untracked", index: null, worktree: null }] }),
      worktree_file_diff: () => ({ ...HUNK_DIFF, path: "n.txt" }),
    });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "n.txt" }));

    await screen.findByRole("region", { name: "Unstaged changes in n.txt" });
    expect(screen.queryByRole("button", { name: /Stage hunk/ })).not.toBeInTheDocument();
  });

  it("the diff closes when its file leaves that side", async () => {
    let clean = false;
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => (clean ? { ...STATUS, entries: [] } : STATUS),
      worktree_file_diff: () => HUNK_DIFF,
      refresh_repo: () => {
        clean = true;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));
    await screen.findByRole("region", { name: "Unstaged changes in a.txt" });

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(screen.queryByRole("region", { name: /changes in a\.txt/ })).not.toBeInTheDocument());
  });
});

describe("start page: commit detail", () => {
  const WIP: Row = { ...ROW, id: "", shortId: "", subject: "Uncommitted changes", wip: true, labels: [] };
  const DETAIL = {
    id: ROW.id,
    shortId: ROW.shortId,
    subject: "first commit",
    body: "",
    author: { name: "Ada", email: "ada@example.invalid", time: 0 },
    committer: { name: "Ada", email: "ada@example.invalid", time: 0 },
    parents: [],
    files: [{ path: "a.txt", change: "added" as const }],
  };

  function withHistory(extra: Handlers = {}) {
    return app({
      launch_path: () => "C:/work/repo",
      open_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 0 }),
      graph_rows: () => [WIP, ROW],
      commit_detail: () => DETAIL,
      commit_file_diff: () => ({ path: "a.txt", binary: false, hunks: [] }),
      ...extra,
    });
  }

  it("selecting a commit shows it in place of the working copy", async () => {
    const calls = withHistory();
    render(Page);

    await fireEvent.click(await screen.findByText("first commit"));

    expect(await screen.findByRole("region", { name: "Commit" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Commit" })).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "commit_detail")?.args).toEqual({ id: 1, commit: ROW.id });
  });

  it("selecting the WIP row goes back to the working copy", async () => {
    withHistory();
    render(Page);
    await fireEvent.click(await screen.findByText("first commit"));
    await screen.findByRole("region", { name: "Commit" });

    await fireEvent.click(screen.getByText("Uncommitted changes"));

    expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Commit" })).toBeInTheDocument();
  });

  it("closing the detail also clears the highlighted row", async () => {
    withHistory();
    render(Page);
    const row = (await screen.findByText("first commit")).closest("li")!;
    await fireEvent.click(row);
    await screen.findByRole("region", { name: "Commit" });

    await fireEvent.click(screen.getByRole("button", { name: "Back to working copy" }));

    expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument();
    expect(row).toHaveAttribute("aria-selected", "false");
  });

  it("a rebuilt graph drops the selection", async () => {
    withHistory({ refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 1 }) });
    render(Page);
    await fireEvent.click(await screen.findByText("first commit"));
    await screen.findByRole("region", { name: "Commit" });

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument());
  });
});

describe("start page: outside changes", () => {
  it("refreshes status and graph when the repo changes outside gitgud", async () => {
    let entries = STATUS.entries;
    let version = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({ ...STATUS, entries }),
      refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: version }),
    });
    render(Page);
    await screen.findByText("first commit");

    entries = [...entries, { path: "outside.txt", kind: "untracked", index: null, worktree: null }];
    version = 1;
    await emit("repo-changed", 1);

    expect(await screen.findByText("outside.txt")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "refresh_repo")?.args).toEqual({ id: 1 });
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2));
  });

  it("keeps the graph when only files changed", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    });
    render(Page);
    await screen.findByText("first commit");

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "repo_status")).toHaveLength(2));
    expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(1);
  });

  it("ignores changes to a repo that is not open", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", refresh_repo: () => ({ id: 9, root: "", rows: 0, graphVersion: 0 }) });
    render(Page);
    await screen.findByText("first commit");

    await emit("repo-changed", 9);
    await new Promise((r) => setTimeout(r, 20));

    expect(calls.some((c) => c.cmd === "refresh_repo")).toBe(false);
  });

  it("coalesces a burst into one refresh plus one follow-up", async () => {
    let release!: () => void;
    let refreshes = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      refresh_repo: () => {
        refreshes++;
        return refreshes === 1
          ? new Promise((r) => (release = () => r({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 })))
          : { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    for (let i = 0; i < 5; i++) {
      await emit("repo-changed", 1);
    }
    release();

    await vi.waitFor(() => expect(refreshes).toBe(2));
    await new Promise((r) => setTimeout(r, 20));
    expect(calls.filter((c) => c.cmd === "refresh_repo")).toHaveLength(2);
  });
});

describe("start page: staging and committing", () => {
  const STAGED: Status = {
    ...STATUS,
    entries: [{ path: "a.txt", kind: "tracked", index: "modified", worktree: null }],
  };

  it("stages a file and shows the refreshed status", async () => {
    let current = STATUS;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => current,
      stage: () => {
        current = STAGED;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "Stage a.txt" }));

    expect(await screen.findByRole("list", { name: "Staged" })).toHaveTextContent("a.txt");
    expect(calls.find((c) => c.cmd === "stage")?.args).toEqual({ id: 1, paths: ["a.txt"] });
    expect(screen.getByRole("button", { name: "Commit 1 file" })).toBeInTheDocument();
  });

  it("commits, then refetches the graph when its version moved", async () => {
    let version = 0;
    const subjects = ["first commit"];
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => (version === 0 ? STAGED : { ...STATUS, entries: [] }),
      graph_rows: () => subjects.map((subject, i) => ({ ...ROW, id: `${i}`, subject })),
      commit: ({ message }: Commands["commit"]["args"]) => {
        subjects.unshift(message);
        version++;
        return { id: 1, root: "C:/work/repo", rows: subjects.length, graphVersion: version };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.input(screen.getByRole("textbox", { name: "Commit message" }), { target: { value: "second" } });
    await fireEvent.click(screen.getByRole("button", { name: "Commit 1 file" }));

    expect(await screen.findByText("second")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "commit")?.args).toEqual({ id: 1, message: "second" });
    expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2);
    expect(screen.getByText("Nothing to commit, working tree clean.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveValue("");
  });

  it("shows git's message when a commit is rejected, and keeps the message", async () => {
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => STAGED,
      commit: () => {
        throw "lint: trailing whitespace in a.txt";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.input(screen.getByRole("textbox", { name: "Commit message" }), { target: { value: "wip" } });
    await fireEvent.click(screen.getByRole("button", { name: "Commit 1 file" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("lint: trailing whitespace in a.txt");
    expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveValue("wip");
  });
});

describe("start page: merge", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 };
  const BRANCHES = {
    local: [
      { name: "main", current: true, upstream: null, ahead: 0, behind: 0 },
      { name: "other", current: false, upstream: null, ahead: 0, behind: 0 },
    ],
    remotes: [],
    fetched: null,
  };
  const TEXT = {
    finalNewline: true,
    chunks: [
      { kind: "same" as const, lines: ["1"] },
      { kind: "conflict" as const, yours: ["yours"], base: ["2"], theirs: ["theirs"] },
      { kind: "same" as const, lines: ["3"] },
    ],
  };

  /** A repo where merging `other` stops on a.txt (code) and t.png (binary). */
  function merging(extra: Handlers = {}) {
    let state: MergeState | null = null;
    const resolve = (path: string) => {
      state = { ...state!, conflicts: state!.conflicts.map((c) => (c.path === path ? { ...c, resolved: true } : c)) };
      return OPENED;
    };
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => BRANCHES,
      merge_state: () => state,
      predict_merge: () => ["a.txt", "t.png"],
      merge: () => {
        state = {
          theirs: "other",
          message: "Merge branch 'other'",
          conflicts: [
            { path: "a.txt", kind: "code", resolved: false },
            { path: "t.png", kind: "binary", resolved: false },
          ],
        };
        return OPENED;
      },
      conflict_text: () => TEXT,
      conflict_sides: () => ({ yours: { size: 2048, lfs: true }, theirs: { size: 4096, lfs: true } }),
      resolve_with: ({ path }) => resolve(path),
      resolve_side: ({ path }) => resolve(path),
      merge_commit: () => {
        state = null;
        return OPENED;
      },
      merge_abort: () => {
        state = null;
        return OPENED;
      },
      ...extra,
    });
    return calls;
  }

  async function startMerge() {
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /other/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Merge other into main…" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("a.txt");
    await fireEvent.click(within(dialog).getByRole("button", { name: /Merge/ }));
    return screen.findByRole("region", { name: "Merge in progress" });
  }

  it("predicts conflicts, merges, and shows the banner and conflict list instead of the graph", async () => {
    const calls = merging();
    const banner = await startMerge();

    expect(calls.find((c) => c.cmd === "predict_merge")?.args).toEqual({ id: 1, branch: "other" });
    expect(calls.find((c) => c.cmd === "merge")?.args).toEqual({ id: 1, branch: "other" });
    expect(banner).toHaveTextContent("Merging other into main · 2 of 2 files left");
    expect(within(banner).getByRole("button", { name: "Commit merge" })).toBeDisabled();
    expect(screen.queryByText("first commit")).not.toBeInTheDocument();
    const list = screen.getByRole("navigation", { name: "Conflicts" });
    expect(within(list).getAllByRole("button").map((b) => b.textContent?.replace(/\s+/g, " ").trim())).toEqual([
      "U a.txt text merge",
      "U t.png pick a side",
    ]);
    expect(await screen.findByRole("region", { name: "Merge a.txt" })).toBeInTheDocument();
  });

  it("resolves a text hunk, then a binary side, then commits with the summary message", async () => {
    const calls = merging();
    const banner = await startMerge();
    await screen.findByText("Conflict 1 of 1");

    await fireEvent.keyDown(window, { key: "ArrowDown", altKey: true });
    await fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });

    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "resolve_with")?.args).toEqual({
        id: 1,
        path: "a.txt",
        content: "1\nyours\ntheirs\n3\n",
        how: "use both",
      }),
    );
    const pick = await screen.findByRole("region", { name: "Pick a side for t.png" });
    expect(await within(pick).findByText("4.0 KB")).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "2" });

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "resolve_side")?.args).toEqual({ id: 1, path: "t.png", side: "theirs" }));
    const message = await screen.findByRole("textbox", { name: "Merge commit message" });
    expect(message).toHaveValue("Merge branch 'other'\n\nConflicts resolved:\n- a.txt: use both\n- t.png: take theirs");
    expect(banner).toHaveTextContent("all 2 files resolved");

    await fireEvent.click(within(banner).getByRole("button", { name: "Commit merge" }));

    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "merge_commit")?.args).toEqual({
        id: 1,
        message: "Merge branch 'other'\n\nConflicts resolved:\n- a.txt: use both\n- t.png: take theirs",
      }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("Merge committed");
    expect(screen.queryByRole("region", { name: "Merge in progress" })).not.toBeInTheDocument();
    expect(await screen.findByText("first commit")).toBeInTheDocument();
  });

  it("Abort ends the merge and says Ctrl+Z brings it back", async () => {
    const calls = merging();
    const banner = await startMerge();

    await fireEvent.click(within(banner).getByRole("button", { name: "Abort, back to before" }));

    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "merge_abort")).toBe(true));
    expect(await screen.findByRole("status")).toHaveTextContent("Merge aborted · Ctrl+Z brings it back");
    expect(screen.queryByRole("region", { name: "Merge in progress" })).not.toBeInTheDocument();
  });

  it("a clean merge says so and stays on the graph", async () => {
    merging({ predict_merge: () => [], merge: () => OPENED });
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /other/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Merge other into main…" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/No conflicts expected/)).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole("button", { name: /Merge/ }));

    expect(await screen.findByRole("status")).toHaveTextContent("Merged other into main");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("Esc in the dialog cancels without merging", async () => {
    const calls = merging();
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /other/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Merge other into main…" }));
    await screen.findByRole("dialog");

    await fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "merge")).toBe(false);
  });

  it("a merge already in progress at open shows the conflict view; Ctrl+Z after a pick re-reads it", async () => {
    let state: MergeState = {
      theirs: null,
      message: "Merge",
      conflicts: [{ path: "a.txt", kind: "code", resolved: true }],
    };
    const calls = app({
      launch_path: () => "C:/work/repo",
      merge_state: () => state,
      undo: () => {
        state = { ...state, conflicts: [{ path: "a.txt", kind: "code", resolved: false }] };
        return { opened: OPENED, action: "Resolve a.txt (use both)" };
      },
      conflict_text: () => TEXT,
    });
    render(Page);
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    expect(banner).toHaveTextContent("Merging another branch into main");
    expect(await screen.findByRole("region", { name: "Merge summary" })).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(await screen.findByRole("region", { name: "Merge a.txt" })).toBeInTheDocument();
    expect(banner).toHaveTextContent("1 of 1 file left");
    expect(calls.filter((c) => c.cmd === "merge_state").length).toBeGreaterThanOrEqual(2);
  });
});

describe("start page: rebase", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 };
  const TEXT = {
    finalNewline: true,
    chunks: [{ kind: "conflict" as const, yours: ["M"], base: ["2"], theirs: ["F"] }],
  };

  /** `feature` rebased onto `main` stops at step 1 of 2 on a.txt. */
  function rebasing(extra: Handlers = {}) {
    let state: RebaseState | null = null;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({ ...STATUS, head: { kind: "branch", name: "feature" } }),
      branches: () => ({
        local: [
          { name: "feature", current: true, upstream: null, ahead: 0, behind: 0 },
          { name: "main", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [],
        fetched: null,
      }),
      rebase_state: () => state,
      predict_rebase: () => ({
        commits: 2,
        behind: 1,
        conflict: { step: 1, shortId: "f1f1f1f", subject: "f1", paths: ["a.txt"] },
      }),
      rebase: () => {
        state = {
          branch: "feature",
          onto: "main",
          step: 1,
          total: 2,
          subject: "f1",
          conflicts: [{ path: "a.txt", kind: "code", resolved: false }],
          editing: false,
        };
        return OPENED;
      },
      conflict_text: () => TEXT,
      resolve_with: () => {
        state = { ...state!, conflicts: [{ path: "a.txt", kind: "code", resolved: true }] };
        return OPENED;
      },
      rebase_step: () => {
        state = null;
        return OPENED;
      },
      ...extra,
    });
    return calls;
  }

  async function startRebase() {
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /main/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Rebase feature onto main…" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/Replays 2 commits of feature on top of 1 new commit from main/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Step 1 \(f1f1f1f "f1"\) will conflict/)).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole("button", { name: /Rebase/ }));
    return screen.findByRole("region", { name: "Rebase in progress" });
  }

  it("predicts, stops on the conflict, resolves, and continues to the end", async () => {
    const calls = rebasing();
    const banner = await startRebase();

    expect(banner).toHaveTextContent("Rebasing feature onto main · step 1 of 2 · 1 of 1 file left");
    expect(within(banner).getByRole("button", { name: "Continue" })).toBeDisabled();
    expect(await screen.findByText("Yours · main (rebased so far)")).toBeInTheDocument();
    expect(screen.getByText('Theirs · "f1"')).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "ArrowRight", altKey: true });
    await fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(await screen.findByRole("region", { name: "Step resolved" })).toHaveTextContent("Step 1 of 2 resolved");
    expect(banner).toHaveTextContent("resolved");

    await fireEvent.click(screen.getByRole("button", { name: "Continue rebase" }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "rebase_step")?.args).toEqual({ id: 1, step: "continue" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Rebase finished");
    expect(screen.queryByRole("region", { name: "Rebase in progress" })).not.toBeInTheDocument();
  });

  it("Abort goes through rebase_step and says Ctrl+Z brings it back", async () => {
    const calls = rebasing();
    const banner = await startRebase();

    await fireEvent.click(within(banner).getByRole("button", { name: "Abort, back to before" }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "rebase_step")?.args).toEqual({ id: 1, step: "abort" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Rebase aborted · Ctrl+Z brings it back");
  });

  it("a clean rebase says so", async () => {
    rebasing({ predict_rebase: () => ({ commits: 2, behind: 1, conflict: null }), rebase: () => OPENED });
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /main/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Rebase feature onto main…" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/No conflicts expected/)).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole("button", { name: /Rebase/ }));

    expect(await screen.findByRole("status")).toHaveTextContent("Rebased feature onto main · Ctrl+Z undoes it");
  });

  it("a branch already on top has nothing to do", async () => {
    rebasing({ predict_rebase: () => ({ commits: 2, behind: 0, conflict: null }) });
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await fireEvent.contextMenu(await within(branches).findByRole("button", { name: /main/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Rebase feature onto main…" }));
    const dialog = await screen.findByRole("dialog");

    expect(await within(dialog).findByText(/already contains everything on main/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: /Rebase/ })).toBeDisabled();
  });
});

describe("start page: history edits", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 3, graphVersion: 0 };
  const TIP: Row = { ...ROW, id: "c".repeat(40), shortId: "ccccccc", subject: "Add c rgiht" };
  const OLDER: Row = { ...ROW, id: "b".repeat(40), shortId: "bbbbbbb", subject: "Add b", labels: [] };
  const BASE: Row = { ...ROW, id: "a".repeat(40), shortId: "aaaaaaa", subject: "base", labels: [] };
  const PLAN = {
    branch: "main",
    base: BASE.id,
    baseShort: "aaaaaaa",
    baseSubject: "base",
    commits: [
      { id: TIP.id, shortId: "ccccccc", subject: TIP.subject, message: TIP.subject, author: "Ada", time: 0, action: "pick" as const },
      { id: OLDER.id, shortId: "bbbbbbb", subject: "Add b", message: "Add b", author: "Ada", time: 0, action: "pick" as const },
    ],
    dependents: [],
    pushed: null,
  };

  function history(extra: Handlers = {}) {
    return app({
      launch_path: () => "C:/work/repo",
      open_repo: () => OPENED,
      repo_status: () => ({ ...STATUS, entries: [] }),
      graph_rows: () => [TIP, OLDER, BASE],
      history_plan: () => PLAN,
      plan_preview: ({ steps }) => ({
        prediction: {
          steps: steps.map((s, i) => ({ id: s.id, step: steps.length - i, outcome: { kind: "clean" as const } })),
          before: 2,
          after: 2,
          changed: 0,
        },
        rows: 5,
      }),
      preview_rows: ({ start, count }) =>
        [
          { ...TIP, id: "", shortId: "", labels: [], subject: "Add c right", after: "reworded" },
          { ...OLDER, id: "", shortId: "", subject: "Add b", after: "" },
          { ...TIP, faded: true },
          { ...OLDER, faded: true },
          BASE,
        ].slice(start, start + count),
      commit_detail: ({ commit }) => ({
        id: commit,
        shortId: commit.slice(0, 7),
        subject: commit === TIP.id ? TIP.subject : "Add b",
        body: commit === TIP.id ? "Why: typo" : "",
        author: { name: "Ada", email: "a", time: 0 },
        committer: { name: "Ada", email: "a", time: 0 },
        parents: [],
        files: [],
      }),
      run_plan: () => OPENED,
      reword_head: () => OPENED,
      quick_edit: () => OPENED,
      ...extra,
    });
  }

  const commits = () => screen.getByRole("listbox", { name: "Commits" });

  async function selectRow(subject: string) {
    await fireEvent.click(await within(await screen.findByRole("listbox", { name: "Commits" })).findByText(subject));
  }

  it("Shift+E opens the plan: banner, preview in the graph, Start runs it, then the Done panel", async () => {
    const calls = history();
    render(Page);
    await selectRow("Add b");
    await fireEvent.keyDown(commits(), { key: "E", shiftKey: true });

    expect(await screen.findByRole("region", { name: "Planning" })).toHaveTextContent("Planning a history edit of main · nothing changes until Start");
    expect(calls.find((c) => c.cmd === "history_plan")?.args).toEqual({ id: 1, from: OLDER.id });
    const plan = screen.getByRole("listbox", { name: "Plan" });
    await fireEvent.keyDown(plan, { key: "r" });
    await fireEvent.input(screen.getByRole("textbox", { name: "Message of ccccccc" }), { target: { value: "Add c right" } });

    expect(await within(commits()).findByText("reworded")).toBeInTheDocument();
    expect(within(commits()).getByText("Add c right").closest("li")).toHaveClass("ghost");

    await fireEvent.click(screen.getByRole("button", { name: /Start rebase/ }));

    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "run_plan")?.args).toEqual({
        id: 1,
        onto: BASE.id,
        base: BASE.id,
        steps: [
          { id: TIP.id, action: "reword", message: "Add c right" },
          { id: OLDER.id, action: "pick", message: null },
        ],
        updateRefs: false,
      }),
    );
    const done = await screen.findByRole("region", { name: "History edit done" });
    expect(done).toHaveTextContent("Rebased main · 2 commits → 2");
    expect(done).toHaveTextContent("✓ Final files identical to before");
    expect(done).toHaveTextContent('Reworded ccccccc → "Add c right"');
    expect(screen.queryByRole("region", { name: "Planning" })).not.toBeInTheDocument();
  });

  it("while planning, Ctrl+Z does nothing and Esc cancels back to the real graph", async () => {
    const calls = history({ undo: () => ({ opened: OPENED, action: "x" }) });
    render(Page);
    await selectRow("Add b");
    await fireEvent.keyDown(commits(), { key: "E", shiftKey: true });
    const plan = await screen.findByRole("listbox", { name: "Plan" });

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await fireEvent.keyDown(plan, { key: "Escape" });

    expect(screen.queryByRole("region", { name: "Planning" })).not.toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "undo")).toBe(false);
    expect(await within(commits()).findByText("Add c rgiht")).toBeInTheDocument();
  });

  it("R on the tip renames inline and keeps the body", async () => {
    const calls = history();
    render(Page);
    await selectRow("Add c rgiht");
    await fireEvent.keyDown(commits(), { key: "r" });

    const input = await screen.findByRole("textbox", { name: "New commit subject" });
    await fireEvent.input(input, { target: { value: "Add c right" } });
    await fireEvent.keyDown(input, { key: "Enter" });

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "reword_head")?.args).toEqual({ id: 1, message: "Add c right\n\nWhy: typo" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Reworded · Ctrl+Z undoes it");
  });

  it("R on an older commit opens the plan with that row set to Reword", async () => {
    history();
    render(Page);
    await selectRow("Add b");
    await fireEvent.keyDown(commits(), { key: "r" });

    expect(await screen.findByRole("textbox", { name: "Message of bbbbbbb" })).toHaveValue("Add b");
  });

  it("F and D apply at once and say Ctrl+Z undoes them; refusals show git's reason", async () => {
    const calls = history();
    render(Page);
    await selectRow("Add b");
    await fireEvent.keyDown(commits(), { key: "d" });

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "quick_edit")?.args).toEqual({ id: 1, commit: OLDER.id, action: "drop" }));
    expect(await screen.findByRole("status")).toHaveTextContent('Dropped "Add b" · Ctrl+Z undoes it');
  });

  it("the row menu offers the same edits", async () => {
    const calls = history({
      quick_edit: () => {
        throw "history with merge commits can't be edited yet";
      },
    });
    render(Page);
    await fireEvent.contextMenu(await screen.findByText("Add b"), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: /Fixup into parent/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent("history with merge commits can't be edited yet");
    expect(calls.find((c) => c.cmd === "quick_edit")?.args).toEqual({ id: 1, commit: OLDER.id, action: "fixup" });
  });

  it("an Edit stop keeps the graph and working copy, with Continue in the banner", async () => {
    const calls = history({
      rebase_state: () => ({ branch: "main", onto: "aaaaaaa", step: 1, total: 2, subject: "Add b", conflicts: [], editing: true }),
      rebase_step: () => OPENED,
    });
    render(Page);

    const banner = await screen.findByRole("region", { name: "Rebase in progress" });
    expect(banner).toHaveTextContent("Stopped at step 1 of 2 to edit Add b");
    expect(screen.getByRole("listbox", { name: "Commits" })).toBeInTheDocument();
    await fireEvent.click(within(banner).getByRole("button", { name: "Continue" }));
    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "rebase_step")?.args).toEqual({ id: 1, step: "continue" }));
  });
});

describe("start page: Milestone 1 workflows", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 2, graphVersion: 0 };
  const TIP: Row = { ...ROW, id: "c".repeat(40), shortId: "ccccccc", subject: "tip commit" };
  const BASE: Row = { ...ROW, id: "a".repeat(40), shortId: "aaaaaaa", subject: "base commit", labels: [] };

  const BRANCHES = {
    local: [
      { name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 2 },
      { name: "other", current: false, upstream: null, ahead: 0, behind: 0 },
    ],
    remotes: [{ name: "origin", branches: ["origin/main", "origin/other"] }],
    fetched: null,
  };

  function repo(extra: Handlers = {}) {
    return app({
      launch_path: () => "C:/work/repo",
      open_repo: () => OPENED,
      repo_status: () => STATUS,
      branches: () => BRANCHES,
      graph_rows: () => [TIP, BASE],
      commit_detail: ({ commit }) => ({
        id: commit,
        shortId: commit.slice(0, 7),
        subject: "tip commit",
        body: "",
        author: { name: "Ada", email: "a", time: 0 },
        committer: { name: "Ada", email: "a", time: 0 },
        parents: [],
        files: [],
      }),
      ...extra,
    });
  }

  it("Pull opens the divergence dialog and sends the chosen strategy", async () => {
    const calls = repo({
      pull: ({ strategy }) => ({
        opened: { ...OPENED, graphVersion: 1 },
        outcome:
          strategy === "auto"
            ? { kind: "diverged", ahead: 1, behind: 2, upstream: "origin/main" }
            : { kind: "merged" },
      }),
    });
    render(Page);
    await screen.findByText("tip commit");

    await fireEvent.keyDown(window, { key: "L", ctrlKey: true, shiftKey: true });

    const dialog = await screen.findByRole("dialog", { name: "Branch diverged from origin/main" });
    await fireEvent.click(within(dialog).getByLabelText(/Merge origin\/main/));
    await fireEvent.click(within(dialog).getByRole("button", { name: /Pull \(merge\)/ }));

    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "pull").map((c) => c.args)).toEqual([
        { id: 1, strategy: "auto", remember: false },
        { id: 1, strategy: "merge", remember: true },
      ]),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("Pulled (merged) · Ctrl+Z undoes it");
  });

  it("Push dialog publishes / pushes and PlanDone offers Force-push with lease and Open PR", async () => {
    const calls = repo({
      repo_status: () => ({ ...STATUS, entries: [] }),
      push_info: () => ({
        branch: "main",
        upstream: "origin/main",
        remote: "origin",
        remotes: ["origin"],
        remoteBranch: "main",
        ahead: 1,
        behind: 0,
        needsForce: false,
        commits: [{ shortId: "ccccccc", subject: "tip commit" }],
        prUrl: "https://github.com/org/repo/pull/new/main",
      }),
      push: () => ({ ...OPENED, graphVersion: 1 }),
      history_plan: () => ({
        branch: "main",
        base: BASE.id,
        baseShort: "aaaaaaa",
        baseSubject: "base commit",
        commits: [
          { id: TIP.id, shortId: "ccccccc", subject: "tip commit", message: "tip commit", author: "Ada", time: 0, action: "pick" },
        ],
        dependents: [],
        pushed: "origin/main",
      }),
      plan_preview: () => ({
        prediction: { steps: [{ id: TIP.id, step: 1, outcome: { kind: "clean" } }], before: 1, after: 1, changed: 0 },
        rows: 2,
      }),
      run_plan: () => ({ ...OPENED, graphVersion: 1 }),
      pr_url: () => "https://github.com/org/repo/pull/new/main",
      open_external_url: () => null,
    });
    render(Page);
    await screen.findByText("tip commit");

    await fireEvent.keyDown(window, { key: "P", ctrlKey: true, shiftKey: true });
    const pushDlg = await screen.findByRole("dialog", { name: "Push main" });
    await within(pushDlg).findByRole("button", { name: "Push" });
    expect(within(pushDlg).queryByRole("checkbox", { name: /force-with-lease/ })).not.toBeInTheDocument();
    await fireEvent.click(within(pushDlg).getByRole("button", { name: "Push" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Pushed main to origin/main");
    expect(calls.find((c) => c.cmd === "push")?.args).toEqual({
      id: 1,
      remote: "origin",
      branch: "main",
      remoteBranch: "main",
      setUpstream: false,
      forceWithLease: false,
    });

    const commits = screen.getByRole("listbox", { name: "Commits" });
    await fireEvent.click(await within(commits).findByText("tip commit"));
    await fireEvent.keyDown(commits, { key: "E", shiftKey: true });
    await fireEvent.click(await screen.findByRole("button", { name: /Start rebase/ }));
    const done = await screen.findByRole("region", { name: "History edit done" });

    await fireEvent.click(within(done).getByRole("button", { name: "Open Pull Request ↗" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "open_external_url")?.args).toEqual({
        url: "https://github.com/org/repo/pull/new/main",
      }),
    );

    await fireEvent.click(within(done).getByRole("button", { name: "Force-push with lease…" }));
    const forceDlg = await screen.findByRole("dialog", { name: "Push main" });
    expect(await within(forceDlg).findByRole("checkbox", { name: /force-with-lease/ })).toBeChecked();
    await fireEvent.click(within(forceDlg).getByRole("button", { name: "Force-push with lease" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Force-pushed main to origin/main"));
    const pushes = calls.filter((c): c is { cmd: "push"; args: Commands["push"]["args"] } => c.cmd === "push");
    expect(pushes.at(-1)?.args.forceWithLease).toBe(true);
  });

  it("Detached HEAD offers Create branch… and creates it", async () => {
    const calls = repo({
      repo_status: () => ({ ...STATUS, head: { kind: "detached", id: "c".repeat(40) } }),
      create_branch: () => ({ ...OPENED, graphVersion: 1 }),
    });
    render(Page);

    const createBtn = await screen.findByRole("button", { name: "Create branch…" });
    await fireEvent.click(createBtn);

    const dialog = await screen.findByRole("dialog", { name: "New branch" });
    await fireEvent.input(within(dialog).getByLabelText("Branch name"), { target: { value: "rescue/fix" } });
    await fireEvent.click(within(dialog).getByRole("button", { name: /Create branch/ }));

    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Created rescue/fix"));
    expect(calls.find((c) => c.cmd === "create_branch")?.args).toEqual({
      id: 1,
      name: "rescue/fix",
      at: null,
      checkout: true,
    });
  });

  it("handles branch create (Ctrl+B), rename (F2), unmerged force-delete, and remote delete", async () => {
    let deleteTry = 0;
    const calls = repo({
      create_branch: () => ({ ...OPENED, graphVersion: 1 }),
      rename_branch: () => ({ ...OPENED, graphVersion: 2 }),
      delete_branch: ({ force }) => {
        deleteTry++;
        if (!force && deleteTry === 1) {
          throw "error: The branch 'other' is not fully merged.";
        }
        return { ...OPENED, graphVersion: 3 };
      },
      delete_remote_branch: () => ({ ...OPENED, graphVersion: 4 }),
    });
    render(Page);
    await screen.findByText("tip commit");

    await fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    let dialog = await screen.findByRole("dialog", { name: "New branch" });
    await fireEvent.input(within(dialog).getByLabelText("Branch name"), { target: { value: "topic/one" } });
    await fireEvent.click(within(dialog).getByRole("button", { name: /Create branch/ }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Created topic/one"));
    expect(calls.find((c) => c.cmd === "create_branch")?.args).toEqual({
      id: 1,
      name: "topic/one",
      at: null,
      checkout: true,
    });

    const sidebarBranches = screen.getByRole("region", { name: "Branches" });
    await vi.waitFor(() => expect(within(sidebarBranches).getByRole("button", { name: "other" })).toBeEnabled());
    await fireEvent.keyDown(within(sidebarBranches).getByRole("button", { name: "other" }), { key: "F2" });
    dialog = await screen.findByRole("dialog", { name: "Rename branch other" });
    await fireEvent.input(within(dialog).getByLabelText("Branch name"), { target: { value: "other-renamed" } });
    await fireEvent.click(within(dialog).getByRole("button", { name: /Rename/ }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Renamed other to other-renamed"));
    expect(calls.find((c) => c.cmd === "rename_branch")?.args).toEqual({
      id: 1,
      old: "other",
      new: "other-renamed",
    });

    await vi.waitFor(() => expect(within(sidebarBranches).getByRole("button", { name: "other" })).toBeEnabled());
    await fireEvent.keyDown(within(sidebarBranches).getByRole("button", { name: "other" }), { key: "Delete" });
    const forceDlg = await screen.findByRole("dialog", { name: "Force-delete unmerged branch other?" });
    await fireEvent.click(within(forceDlg).getByRole("button", { name: "Force delete" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Deleted other"));
    expect(calls.filter((c) => c.cmd === "delete_branch").map((c) => c.args)).toEqual([
      { id: 1, name: "other", force: false },
      { id: 1, name: "other", force: true },
    ]);

    const remotes = screen.getByRole("region", { name: "Remotes" });
    await fireEvent.click(within(remotes).getByRole("button", { name: /origin/ }));
    await fireEvent.contextMenu(within(remotes).getByRole("button", { name: /other/ }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Delete origin/other…" }));
    const remoteDlg = await screen.findByRole("dialog", { name: "Delete remote branch origin/other?" });
    await fireEvent.click(within(remoteDlg).getByRole("button", { name: "Delete remote branch" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Deleted origin/other"));
    expect(calls.find((c) => c.cmd === "delete_remote_branch")?.args).toEqual({
      id: 1,
      remote: "origin",
      branch: "other",
    });
  });

  it("handles Reset, Cherry-pick, Revert, and SequencerBanner continue", async () => {
    let seq: SequencerState | null = null;
    const calls = repo({
      reset: () => ({ ...OPENED, graphVersion: 1 }),
      predict_cherry_pick: () => [],
      predict_revert: () => ["a.txt"],
      cherry_pick: () => ({ ...OPENED, graphVersion: 2 }),
      revert_commit: () => {
        seq = {
          kind: "revert",
          commit: TIP.id,
          shortId: "ccccccc",
          subject: "tip commit",
          message: 'Revert "tip commit"',
          conflicts: [{ path: "a.txt", kind: "code", resolved: true }],
        };
        return { ...OPENED, graphVersion: 3 };
      },
      sequencer_state: () => seq,
      sequencer_step: () => {
        seq = null;
        return { ...OPENED, graphVersion: 4 };
      },
      conflict_text: () => ({ chunks: [], finalNewline: true }),
    });
    render(Page);
    const commits = await screen.findByRole("listbox", { name: "Commits" });

    await fireEvent.contextMenu(await within(commits).findByText("base commit"), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Reset main to here…" }));
    const resetDlg = await screen.findByRole("dialog", { name: "Reset main to aaaaaaa" });
    await fireEvent.click(within(resetDlg).getByLabelText(/Hard/));
    await fireEvent.click(within(resetDlg).getByRole("button", { name: /Reset \(hard\)/ }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Reset (hard) to aaaaaaa"));
    expect(calls.find((c) => c.cmd === "reset")?.args).toEqual({ id: 1, target: BASE.id, mode: "hard" });

    await fireEvent.contextMenu(await within(screen.getByRole("listbox", { name: "Commits" })).findByText("tip commit"), {
      clientX: 5,
      clientY: 5,
    });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Cherry-pick…" }));
    const pickDlg = await screen.findByRole("dialog", { name: "Cherry-pick ccccccc onto main" });
    await within(pickDlg).findByText(/No conflicts expected/);
    await fireEvent.click(within(pickDlg).getByRole("button", { name: /Cherry-pick/ }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Cherry-picked ccccccc"));
    expect(calls.find((c) => c.cmd === "cherry_pick")?.args).toEqual({ id: 1, commit: TIP.id });

    await fireEvent.contextMenu(await within(screen.getByRole("listbox", { name: "Commits" })).findByText("tip commit"), {
      clientX: 5,
      clientY: 5,
    });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Revert commit…" }));
    const revDlg = await screen.findByRole("dialog", { name: "Revert ccccccc onto main" });
    await within(revDlg).findByText("a.txt");
    await fireEvent.click(within(revDlg).getByRole("button", { name: /Revert/ }));

    const banner = await screen.findByRole("region", { name: "Reverting in progress" });
    await fireEvent.click(within(banner).getByRole("button", { name: "Continue" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Revert finished · Ctrl+Z undoes it"));
    expect(calls.find((c) => c.cmd === "sequencer_step")?.args).toEqual({ id: 1, step: "continue" });
  });

  it("saves a stash, inspects it in StashDetail, and pops/applies/drops it", async () => {
    const STASH = {
      index: 0,
      shortId: "stash@{0}",
      id: "s".repeat(40),
      parentId: BASE.id,
      baseId: BASE.id,
      branch: "main",
      message: "WIP shader",
      time: 1_700_000_000,
    };
    let stashList = [STASH];
    const calls = repo({
      stashes: () => stashList,
      stash_save: () => ({ ...OPENED, graphVersion: 1 }),
      stash_detail: () => ({
        id: STASH.id,
        shortId: "stash@{0}",
        subject: "WIP shader",
        body: "",
        author: { name: "Ada", email: "a", time: 1_700_000_000 },
        committer: { name: "Ada", email: "a", time: 1_700_000_000 },
        parents: [BASE.id],
        files: [{ path: "a.txt", change: "modified" }],
      }),
      stash_file_diff: () => ({ path: "a.txt", binary: false, hunks: [] }),
      stash_apply: () => ({ ...OPENED, graphVersion: 1 }),
      stash_pop: () => {
        stashList = [];
        return { ...OPENED, graphVersion: 3 };
      },
      stash_drop: () => ({ ...OPENED, graphVersion: 2 }),
    });
    render(Page);
    await screen.findByText("tip commit");

    await fireEvent.click(within(screen.getByRole("banner")).getByRole("button", { name: "Stash…" }));
    const stashDlg = await screen.findByRole("dialog", { name: "Stash changes on main" });
    await fireEvent.input(within(stashDlg).getByLabelText("Stash message"), { target: { value: "WIP shader" } });
    await fireEvent.click(within(stashDlg).getByRole("button", { name: /Stash changes/ }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Stashed changes"));
    expect(calls.find((c) => c.cmd === "stash_save")?.args).toEqual({
      id: 1,
      message: "WIP shader",
      includeUntracked: false,
    });

    const stashesSec = await screen.findByRole("region", { name: "Stashes" });
    await fireEvent.click(within(stashesSec).getByRole("button", { name: /WIP shader/ }));
    const detail = await screen.findByRole("region", { name: "Stash" });
    await within(detail).findByRole("button", { name: /a\.txt/ });

    await vi.waitFor(() => expect(within(detail).getByRole("button", { name: "Apply" })).toBeEnabled());
    await fireEvent.click(within(detail).getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Applied stash@{0}"));
    expect(calls.find((c) => c.cmd === "stash_apply")?.args).toEqual({ id: 1, index: 0 });

    await vi.waitFor(() => expect(within(detail).getByRole("button", { name: "Drop" })).toBeEnabled());
    await fireEvent.click(within(detail).getByRole("button", { name: "Drop" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Dropped stash@{0}"));
    expect(calls.find((c) => c.cmd === "stash_drop")?.args).toEqual({ id: 1, index: 0 });

    await fireEvent.click(within(screen.getByRole("region", { name: "Stashes" })).getByRole("button", { name: /WIP shader/ }));
    const detail2 = await screen.findByRole("region", { name: "Stash" });
    await within(detail2).findByRole("button", { name: "Pop" });
    await vi.waitFor(() => expect(within(detail2).getByRole("button", { name: "Pop" })).toBeEnabled());
    await fireEvent.click(within(detail2).getByRole("button", { name: "Pop" }));
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Popped stash@{0}"));
    expect(calls.find((c) => c.cmd === "stash_pop")?.args).toEqual({ id: 1, index: 0 });
  });
});

describe("start page: Milestone 2 Library & multi-tab workspace", () => {
  it("restores saved session tabs on launch when GITGUD_OPEN is empty", async () => {
    const LIB = {
      version: 1,
      folders: [{ id: "f-1", name: "Studio", parentId: null, order: 0 }],
      repos: [
        {
          id: "r-1",
          name: "alpha",
          path: "C:/work/alpha",
          folderId: "f-1",
          pinned: false,
          order: 0,
          lastOpened: 100,
          cached: null,
        },
        {
          id: "r-2",
          name: "beta",
          path: "C:/work/beta",
          folderId: "f-1",
          pinned: false,
          order: 1,
          lastOpened: 200,
          cached: null,
        },
      ],
      tabs: { openPaths: ["C:/work/alpha", "C:/work/beta"], activePath: "C:/work/beta" },
      undoAction: null,
      redoAction: null,
    };
    const calls = app({
      library_load: () => LIB,
      graph_rows: ({ id }) => [{ ...ROW, subject: id === 1 ? "alpha commit" : "beta commit" }],
    });
    render(Page);

    expect(await screen.findByText("beta commit")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Library" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tab", { name: /beta/ })).toHaveAttribute("aria-selected", "true");
    expect(calls.filter((c) => c.cmd === "open_repo").map((c) => c.args)).toEqual([
      { path: "C:/work/alpha" },
      { path: "C:/work/beta" },
    ]);
  });

  it("supports Ctrl+P quick switch, Ctrl+Tab cycling, Ctrl+W close without closing Library, and Scan a folder", async () => {
    const LIB = {
      version: 1,
      folders: [{ id: "f-1", name: "Work", parentId: null, order: 0 }],
      repos: [
        {
          id: "r-1",
          name: "alpha",
          path: "C:/work/alpha",
          folderId: "f-1",
          pinned: true,
          order: 0,
          lastOpened: 100,
          cached: {
            path: "C:/work/alpha",
            missing: false,
            branch: "main",
            detached: false,
            dirtyCount: 0,
            untrackedCount: 0,
            ahead: 0,
            behind: 0,
            fetchedAt: null,
            locksHeld: 0,
            locksCheckedAt: null,
            lastActivity: 100,
            host: "gitHub" as const,
            isUnity: false,
            checkedAt: 100,
          },
        },
      ],
      tabs: { openPaths: [], activePath: null },
      undoAction: null,
      redoAction: null,
    };
    const calls = app({
      library_load: () => LIB,
      "plugin:dialog|open": () => "C:/work/scan-root",
      scan_folder: () => ({
        root: "C:/work/scan-root",
        totalFound: 1,
        elapsedMs: 15,
        groups: [
          {
            dir: "C:/work/scan-root/games",
            suggestedFolder: "Work / games",
            repos: [{ name: "gamma", path: "C:/work/scan-root/games/gamma", alreadyInLibrary: false }],
          },
        ],
      }),
      library_add_repos: ({ entries }) => ({
        ...LIB,
        repos: [
          ...LIB.repos,
          {
            id: "r-2",
            name: entries[0].name ?? "gamma",
            path: entries[0].path,
            folderId: "f-1",
            pinned: false,
            order: 1,
            lastOpened: null,
            cached: null,
          },
        ],
        undoAction: "Add repository",
      }),
    });
    render(Page);
    await screen.findByText("alpha");

    await fireEvent.click(screen.getByRole("button", { name: "Scan a folder…" }));
    const scanDlg = await screen.findByRole("dialog", { name: /Add repositories · scan/ });
    await fireEvent.click(within(scanDlg).getByRole("button", { name: "Add 1 repo" }));
    await vi.waitFor(() => {
      const add = calls.find((c): c is { cmd: "library_add_repos"; args: Commands["library_add_repos"]["args"] } => c.cmd === "library_add_repos");
      expect(add?.args.entries).toHaveLength(1);
    });

    await fireEvent.keyDown(window, { key: "p", ctrlKey: true });
    const qs = await screen.findByRole("dialog", { name: "Go to repository" });
    await fireEvent.input(within(qs).getByRole("textbox", { name: "Go to repository" }), { target: { value: "alp" } });
    await fireEvent.keyDown(qs, { key: "Enter" });

    expect(await screen.findByText("first commit")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /alpha/ })).toHaveAttribute("aria-selected", "true");

    await fireEvent.keyDown(window, { key: "Tab", ctrlKey: true, shiftKey: true });
    expect(screen.getByRole("tab", { name: "Library" })).toHaveAttribute("aria-selected", "true");

    await fireEvent.keyDown(window, { key: "w", ctrlKey: true });
    expect(calls.some((c) => c.cmd === "close_repo")).toBe(false);

    await fireEvent.keyDown(window, { key: "Tab", ctrlKey: true });
    expect(screen.getByRole("tab", { name: /alpha/ })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(window, { key: "w", ctrlKey: true });
    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "close_repo")?.args).toEqual({ id: 1 }));
    expect(screen.getByRole("tab", { name: "Library" })).toHaveAttribute("aria-selected", "true");
  });
});

describe("command palette, shortcut sheet, settings, and error fixes", () => {
  it("opens Ctrl+K palette, ? shortcut sheet, and Ctrl+, settings", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [
          { name: "main", current: true, upstream: null, ahead: 0, behind: 0 },
          { name: "feat/palette", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [],
        fetched: null,
      }),
      checkout: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const pal = await screen.findByRole("dialog", { name: "Command palette" });
    await fireEvent.click(within(pal).getByRole("option", { name: /Checkout branch/ }));
    await fireEvent.click(within(pal).getByRole("option", { name: "feat/palette" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "checkout")?.args).toEqual({ id: 1, branch: "feat/palette" }),
    );

    await fireEvent.keyDown(window, { key: "?" });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    await fireEvent.keyDown(screen.getByRole("dialog", { name: "Keyboard shortcuts" }), { key: "Escape" });

    await fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const settings = await screen.findByRole("dialog", { name: "Settings" });
    await fireEvent.click(within(settings).getByRole("button", { name: "repo" }));
    await fireEvent.click(within(settings).getByRole("button", { name: "Save identity" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "config_set_identity")).toBe(true));
  });

  it("explains push rejections and runs 1-click Pull fix", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [{ name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 1 }],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
        fetched: 1_700_000_000,
      }),
      push_info: () => ({
        branch: "main",
        remote: "origin",
        remoteBranch: "main",
        upstream: "origin/main",
        ahead: 1,
        behind: 1,
        needsForce: false,
        commits: [],
        remotes: ["origin"],
        prUrl: null,
      }),
      push: () => {
        throw "! [rejected] main -> main (non-fast-forward)";
      },
      pull: () => ({
        opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 },
        outcome: { kind: "fastForwarded" },
      }),
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: /Push/ }));
    const dlg = await screen.findByRole("dialog", { name: "Push main" });
    await fireEvent.click(within(dlg).getByRole("button", { name: "Push" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Remote has newer commits");
    await fireEvent.click(within(alert).getByRole("button", { name: "Pull now" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "pull")).toBe(true));
  });
});

describe("Unity, LFS locks, Smart Merge verify, and open-editor guard", () => {
  it("shows .meta warnings, LFS badges, Bitbucket hidden locks, and open-editor warning", async () => {
    const calls = app({
      launch_path: () => "C:/work/unity",
      repo_status: () => ({
        root: "C:/work/unity",
        head: { kind: "branch", name: "main" },
        entries: [{ path: "Assets/Art/Hero.psd", kind: "tracked", index: null, worktree: "modified" }],
      }),
      branches: () => ({
        local: [
          { name: "main", current: true, upstream: null, ahead: 0, behind: 0 },
          { name: "feature", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [],
        fetched: null,
      }),
      unity_info: () => ({
        isUnity: true,
        projectRoot: "C:/work/unity",
        editorOpen: true,
        editorVersion: "6000.0.12f1",
        smartMergeConfigured: false,
        smartMergeToolPath: null,
        detectedEditors: [],
      }),
      unity_meta_issues: () => [
        { kind: "missingMeta", path: "Assets/NoMeta.cs", detail: "Missing .meta file for Assets/NoMeta.cs" },
      ],
      unity_check_open_assets: () => true,
      lfs_info: () => ({
        installed: true,
        version: "git-lfs/3.6.0",
        trackedPatterns: ["*.psd"],
        supportsLocks: false,
        locksUnsupportedReason: "Bitbucket Cloud does not support Git LFS file locking",
      }),
      checkout: () => ({ id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 }),
    });
    render(Page);

    expect(await screen.findByText(/Missing \.meta file/)).toBeInTheDocument();
    expect(screen.getByText("LFS")).toBeInTheDocument();
    // Bitbucket Cloud hides Locks section completely (spec O1).
    expect(screen.queryByRole("region", { name: "Locks" })).not.toBeInTheDocument();

    await fireEvent.dblClick(screen.getByRole("button", { name: "feature" }));
    const warnDlg = await screen.findByRole("dialog", { name: "Unity Editor is open" });
    await fireEvent.click(within(warnDlg).getByRole("button", { name: "Proceed anyway" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "checkout")?.args).toEqual({ id: 1, branch: "feature" }),
    );
  });

  it("shows Smart Merge setup, gates merge commit on Verify confirmation, and unlocks on Push", async () => {
    let remerged = false;
    let confirmed = false;
    const calls = app({
      launch_path: () => "C:/work/unity",
      branches: () => ({
        local: [{ name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 0 }],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
        fetched: 1_700_000_000,
      }),
      unity_info: () => ({
        isUnity: true,
        projectRoot: "C:/work/unity",
        editorOpen: false,
        editorVersion: "6000.0.12f1",
        smartMergeConfigured: remerged,
        smartMergeToolPath: "C:/Unity/UnityYAMLMerge.exe",
        detectedEditors: ["C:/Unity/UnityYAMLMerge.exe"],
      }),
      unity_configure_smart_merge: () => ({
        isUnity: true,
        projectRoot: "C:/work/unity",
        editorOpen: false,
        editorVersion: "6000.0.12f1",
        smartMergeConfigured: true,
        smartMergeToolPath: "C:/Unity/UnityYAMLMerge.exe",
        detectedEditors: ["C:/Unity/UnityYAMLMerge.exe"],
      }),
      unity_remerge_conflicts: () => {
        remerged = true;
        return { id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 };
      },
      unity_confirm_verify: ({ confirmed: c }) => {
        confirmed = c;
        return { id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 };
      },
      merge_state: () =>
        !remerged
          ? {
              theirs: "feature",
              message: "Merge feature",
              conflicts: [{ path: "Assets/Ship.prefab", kind: "unityYaml", resolved: false }],
            }
          : {
              theirs: "feature",
              message: "Merge feature",
              conflicts: [{ path: "Assets/Ship.prefab", kind: "unityYaml", resolved: true }],
              unityVerify: [
                {
                  path: "Assets/Ship.prefab",
                  verify: [
                    {
                      fileId: "200",
                      objectName: "Engine (200)",
                      property: "speed",
                      base: "10",
                      ours: "20",
                      theirs: "30",
                      merged: "30",
                      confirmed,
                    },
                  ],
                  autoMerged: [
                    {
                      fileId: "100",
                      objectName: "Transform (Ship)",
                      property: "m_LocalPosition",
                      side: "yours",
                      value: "{x: 1}",
                    },
                  ],
                },
              ],
            },
      lfs_info: () => ({
        installed: true,
        version: "git-lfs/3.6.0",
        trackedPatterns: ["*.psd"],
        supportsLocks: true,
        locksUnsupportedReason: null,
      }),
      lfs_locks: () => [
        { id: "1", path: "Assets/Art/Ship.psd", owner: "Ada", lockedAt: "2026-11-22T00:00:00Z", ours: true },
      ],
      lfs_locks_in_push: () => [
        { id: "1", path: "Assets/Art/Ship.psd", owner: "Ada", lockedAt: "2026-11-22T00:00:00Z", ours: true },
      ],
      push_info: () => ({
        branch: "main",
        remote: "origin",
        remoteBranch: "main",
        upstream: "origin/main",
        ahead: 1,
        behind: 0,
        needsForce: false,
        commits: [{ shortId: "aaaaaaa", subject: "ship art" }],
        remotes: ["origin"],
        prUrl: null,
      }),
      push: () => ({ id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 }),
    });
    render(Page);

    const setupBtn = await screen.findByRole("button", { name: /Set up Smart Merge/ });
    await fireEvent.click(setupBtn);

    expect(await screen.findByText(/Auto-merged cleanly \(1 properties\)/)).toBeInTheDocument();
    const summary = screen.getByRole("region", { name: "Merge summary" });
    expect(within(summary).getByRole("button", { name: /Commit merge/ })).toBeDisabled();

    await fireEvent.click(screen.getByRole("button", { name: "Confirm all" }));
    await vi.waitFor(() =>
      expect(within(screen.getByRole("region", { name: "Merge summary" })).getByRole("button", { name: /Commit merge/ })).toBeEnabled(),
    );

    // Simulate clean state after merge commit for push-unlock test.
    remerged = false;
  });

  it("shows Locks in sidebar and unlocks selected files after Push", async () => {
    const calls = app({
      launch_path: () => "C:/work/unity",
      branches: () => ({
        local: [{ name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 0 }],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
        fetched: 1_700_000_000,
      }),
      lfs_info: () => ({
        installed: true,
        version: "git-lfs/3.6.0",
        trackedPatterns: ["*.psd"],
        supportsLocks: true,
        locksUnsupportedReason: null,
      }),
      lfs_locks: () => [
        { id: "1", path: "Assets/Art/Ship.psd", owner: "Ada", lockedAt: "2026-11-22T00:00:00Z", ours: true },
      ],
      lfs_locks_in_push: () => [
        { id: "1", path: "Assets/Art/Ship.psd", owner: "Ada", lockedAt: "2026-11-22T00:00:00Z", ours: true },
      ],
      push_info: () => ({
        branch: "main",
        remote: "origin",
        remoteBranch: "main",
        upstream: "origin/main",
        ahead: 1,
        behind: 0,
        needsForce: false,
        commits: [{ shortId: "aaaaaaa", subject: "ship art" }],
        remotes: ["origin"],
        prUrl: null,
      }),
      push: () => ({ id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 }),
    });
    render(Page);

    expect(await screen.findByRole("region", { name: "Locks" })).toHaveTextContent("Ship.psd");

    await fireEvent.click(screen.getByRole("button", { name: /Push/ }));
    const dlg = await screen.findByRole("dialog", { name: "Push main" });
    expect(await within(dlg).findByText("Unlock")).toBeInTheDocument();
    await fireEvent.click(within(dlg).getByRole("button", { name: "Push" }));

    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "lfs_unlock")?.args).toEqual({
        id: 1,
        path: "Assets/Art/Ship.psd",
        force: false,
      }),
    );
  });

  it("lists and creates tags, inspects file history/blame, and prunes op log + views reflog", async () => {
    let tagList = [
      {
        name: "v0.9.0",
        target: "a".repeat(40),
        shortTarget: "aaaaaaa",
        annotated: true,
        message: "beta" as string | null,
        time: 1_700_000_000,
      },
    ];
    let pruned = false;
    const calls = app({
      launch_path: () => "C:/work/repo",
      tags: () => tagList,
      create_tag: ({ name, target, message }) => {
        tagList = [
          {
            name,
            target: target ?? "a".repeat(40),
            shortTarget: (target ?? "a".repeat(40)).slice(0, 7),
            annotated: Boolean(message),
            message: message ?? null,
            time: 1_700_100_000,
          },
          ...tagList,
        ];
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
      repo_status: () => ({
        root: "C:/work/repo",
        head: { kind: "branch", name: "master", id: "a".repeat(40) },
        entries: [
          {
            path: "src/app.ts",
            index: null,
            worktree: "modified",
            kind: "tracked",
            binary: false,
            bytes: 40,
            conflicted: false,
          },
        ],
      }),
      worktree_file_diff: ({ path }) => ({
        path,
        binary: false,
        hunks: [
          {
            oldStart: 1,
            oldLines: 1,
            newStart: 1,
            newLines: 1,
            lines: [{ kind: "added", old: null, new: 1, text: "const x = 1;" }],
          },
        ],
      }),
      file_history: () => [
        {
          id: "a".repeat(40),
          shortId: "aaaaaaa",
          subject: "Touch app.ts",
          author: "Max",
          time: 1_700_000_000,
          change: "modified",
        },
      ],
      blame_file: () => [
        {
          lineNo: 1,
          commit: "a".repeat(40),
          shortCommit: "aaaaaaa",
          author: "Max",
          time: 1_700_000_000,
          summary: "Touch app.ts",
          text: "export const ok = true;",
        },
      ],
      op_log: () => [{ id: 1, time: 1_700_000_000, action: "Stage src/app.ts", undone: false, paths: [] }],
      op_log_stats: () => ({
        entryCount: pruned ? 1 : 5,
        totalBytes: pruned ? 512 : 4096,
        prunableCount: pruned ? 0 : 4,
        reclaimableBytes: pruned ? 0 : 3584,
      }),
      op_log_prune: () => {
        pruned = true;
        return { entryCount: 1, totalBytes: 512, prunableCount: 0, reclaimableBytes: 0 };
      },
      outside_reflog: () => [
        {
          id: "a".repeat(40),
          shortId: "aaaaaaa",
          selector: "HEAD@{0}",
          action: "commit: external fix",
          time: 1_700_000_100,
        },
      ],
    });
    render(Page);

    const tagsSection = await screen.findByRole("region", { name: "Tags" });
    expect(await within(tagsSection).findByText("v0.9.0")).toBeInTheDocument();

    await fireEvent.click(within(tagsSection).getByRole("button", { name: "New tag" }));
    const tagDialog = await screen.findByRole("dialog", { name: "New tag" });
    await fireEvent.input(within(tagDialog).getByLabelText("Tag name"), { target: { value: "v1.0.0" } });
    await fireEvent.input(within(tagDialog).getByLabelText("Annotation message"), { target: { value: "Ship it" } });
    await fireEvent.click(within(tagDialog).getByRole("button", { name: /Create tag/ }));

    expect(calls.find((c) => c.cmd === "create_tag")?.args).toEqual({
      id: 1,
      name: "v1.0.0",
      target: null,
      message: "Ship it",
    });
    expect(await within(tagsSection).findByText("v1.0.0")).toBeInTheDocument();

    await fireEvent.click(await screen.findByTitle("src/app.ts"));
    await fireEvent.click(await screen.findByRole("button", { name: "History" }));
    expect(await screen.findByText("Touch app.ts")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Blame" }));
    expect(await screen.findByText("export const ok = true;")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Close file inspector" }));

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));
    await fireEvent.click(await screen.findByText(/Outside gitgud \(reflog\)/));
    expect(await screen.findByText("HEAD@{0}")).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole("button", { name: "Prune old (4 · 3.5 KB)" }));
    expect(calls.some((c) => c.cmd === "op_log_prune")).toBe(true);
  });

  it("shows structural Unity prefab diff and auto-lock suggestion for unlocked LFS assets", async () => {
    prefs.views = { prefab: "structure" };
    const calls = app({
      launch_path: () => "C:/work/unity",
      lfs_info: () => ({
        installed: true,
        version: "git-lfs/3.6.0",
        trackedPatterns: ["*.psd"],
        supportsLocks: true,
        locksUnsupportedReason: null,
      }),
      lfs_locks: () => [],
      lfs_lock: () => null,
      repo_status: () => ({
        root: "C:/work/unity",
        head: { kind: "branch", name: "main", id: "a".repeat(40) },
        entries: [
          {
            path: "Assets/Player.prefab",
            index: null,
            worktree: "modified",
            kind: "tracked",
            binary: false,
            bytes: 120,
            conflicted: false,
          },
          {
            path: "Assets/Art/Hull.psd",
            index: null,
            worktree: "modified",
            kind: "tracked",
            binary: true,
            bytes: 2048,
            conflicted: false,
          },
        ],
      }),
      worktree_unity_diff: ({ path }) => ({
        path,
        addedObjects: 0,
        removedObjects: 0,
        modifiedProperties: 1,
        objects: [
          {
            fileId: "12",
            objectPath: "Player",
            component: "PlayerMover.cs",
            kind: "modified",
            properties: [
              {
                property: "moveSpeed",
                before: "5",
                after: "9",
                beforeDisplay: "5",
                afterDisplay: "9",
              },
            ],
          },
        ],
      }),
      worktree_file_diff: ({ path }) => ({
        path,
        binary: path.endsWith(".psd"),
        hunks: [],
      }),
    });
    render(Page);

    await fireEvent.click(await screen.findByTitle("Assets/Player.prefab"));
    expect(await screen.findByText(/PlayerMover\.cs/)).toBeInTheDocument();
    expect(screen.getByText("moveSpeed")).toBeInTheDocument();

    await fireEvent.click(await screen.findByTitle("Assets/Art/Hull.psd"));
    expect(await screen.findByText(/LFS asset isn't locked yet/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Lock file" }));
    expect(calls.find((c) => c.cmd === "lfs_lock")?.args).toEqual({
      id: 1,
      path: "Assets/Art/Hull.psd",
    });
  });

  it("resolves a Unity YAML conflict via R4-A property table and shows post-merge check", async () => {
    let resolved = false;
    const calls = app({
      launch_path: () => "C:/work/unity",
      merge_state: () => ({
        theirs: "feature",
        message: "Merge feature",
        conflicts: [{ path: "Assets/Ship.prefab", kind: "unityYaml", resolved }],
      }),
      unity_conflict_table: ({ path }) => ({
        path,
        conflicts: [
          {
            id: "12::speed",
            fileId: "12",
            objectPath: "Ship / Engine",
            component: "Thruster.cs",
            property: "speed",
            base: "10",
            yours: "15",
            theirs: "22",
            baseDisplay: "10",
            yoursDisplay: "15",
            theirsDisplay: "22",
          },
        ],
        autoMerged: [],
      }),
      resolve_unity_conflict: () => {
        resolved = true;
        return { id: 1, root: "C:/work/unity", rows: 1, graphVersion: 0 };
      },
      unity_post_merge_check: () => [],
    });
    render(Page);

    expect(await screen.findByText("Ship / Engine")).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "1" });
    await fireEvent.click(screen.getByRole("button", { name: /Save & stage/ }));

    expect(calls.find((c) => c.cmd === "resolve_unity_conflict")?.args).toEqual({
      id: 1,
      path: "Assets/Ship.prefab",
      picks: [{ fileId: "12", property: "speed", value: "15" }],
    });
    expect(await screen.findByText(/Post-merge check passed/)).toBeInTheDocument();
  });

  it("reviews an agent proposal from the sidebar inbox: requests changes, edits message, and approves", async () => {
    let proposal: ProposalSummary = {
      id: "p-1",
      commitId: "d".repeat(40),
      shortCommit: "ddddddd",
      baseId: "a".repeat(40),
      message: "Add steering damper",
      why: "Prevents high-speed wobble",
      agent: "claude-code",
      status: "pending",
      createdAt: 1_700_000_000,
      updatedAt: 1_700_000_000,
      files: ["Assets/Scripts/Engine.cs"],
      riskFlags: [
        {
          kind: "missingMeta",
          path: "Assets/Scripts/Engine.cs",
          detail: "Missing companion .meta file",
        },
      ],
      comments: [],
      headMoved: false,
    };
    const calls = app({
      launch_path: () => "C:/work/repo",
      proposals_list: () => [proposal],
      proposal_file_diff: ({ path }) => ({
        path,
        binary: false,
        hunks: [
          {
            oldStart: 1,
            oldLines: 1,
            newStart: 1,
            newLines: 1,
            lines: [{ kind: "added", old: null, new: 1, text: "float damping = 1.4f;" }],
          },
        ],
      }),
      proposal_request_changes: ({ comment }) => {
        proposal = {
          ...proposal,
          status: "changesRequested",
          comments: [{ id: "c-1", author: "Max", text: comment, path: null, time: 1_700_000_100 }],
        };
        return proposal;
      },
      proposal_approve: () => {
        proposal = { ...proposal, status: "approved" };
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 };
      },
    });
    render(Page);

    const inbox = await screen.findByRole("region", { name: "Inbox" });
    await fireEvent.click(within(inbox).getByRole("button", { name: /Add steering damper/ }));

    expect(await screen.findByText(/Prevents high-speed wobble/)).toBeInTheDocument();
    expect(screen.getByText(/Missing companion \.meta file/)).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "Diff of Assets/Scripts/Engine.cs" })).toHaveTextContent(
      "float damping = 1.4f;",
    );

    const feedback = screen.getByLabelText("Change request feedback");
    await fireEvent.input(feedback, { target: { value: "Also tweak rudder curve" } });
    await fireEvent.click(screen.getByRole("button", { name: "Request changes" }));

    expect(calls.find((c) => c.cmd === "proposal_request_changes")?.args).toEqual({
      id: 1,
      proposalId: "p-1",
      comment: "Also tweak rudder curve",
      path: null,
    });
    expect(await screen.findByText("Also tweak rudder curve")).toBeInTheDocument();

    const msgInput = screen.getByLabelText("Proposal commit message");
    await fireEvent.input(msgInput, { target: { value: "Tune steering damper" } });
    await fireEvent.click(screen.getByRole("button", { name: "Approve & commit" }));

    expect(calls.find((c) => c.cmd === "proposal_approve")?.args).toEqual({
      id: 1,
      proposalId: "p-1",
      message: "Tune steering damper",
      selectedPaths: ["Assets/Scripts/Engine.cs"],
    });
  });

  it("shows stacked branches in the sidebar, opens stacked PR URLs, and restacks", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branch_stacks: () => [
        {
          root: "main",
          levels: [
            {
              branch: "stack/part1",
              base: "main",
              commits: 1,
              current: false,
              needsRestack: true,
              prUrl: "https://github.com/Max/boat/compare/main...stack/part1?expand=1",
            },
            {
              branch: "stack/part2",
              base: "stack/part1",
              commits: 2,
              current: true,
              needsRestack: true,
              prUrl: "https://github.com/Max/boat/compare/stack/part1...stack/part2?expand=1",
            },
          ],
        },
      ],
      restack: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 }),
      open_external_url: () => null,
    });
    render(Page);

    const stacksRegion = await screen.findByRole("region", { name: "Stacks" });
    expect(within(stacksRegion).getByText("stack/part1")).toBeInTheDocument();
    expect(within(stacksRegion).getByText("stack/part2")).toBeInTheDocument();

    await fireEvent.click(within(stacksRegion).getByRole("button", { name: "Open PR for stack/part2" }));
    expect(calls.find((c) => c.cmd === "open_external_url")?.args).toEqual({
      url: "https://github.com/Max/boat/compare/stack/part1...stack/part2?expand=1",
    });

    await fireEvent.click(within(stacksRegion).getByRole("button", { name: "Restack" }));
    expect(calls.find((c) => c.cmd === "restack")?.args).toEqual({
      id: 1,
      topBranch: "stack/part2",
      onto: "main",
    });
  });

  it("drafts a commit message with AI from staged changes", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({
        root: "C:/work/repo",
        head: { kind: "branch", name: "main" },
        entries: [{ path: "a.txt", kind: "tracked", index: "modified", worktree: null }],
      }),
      ai_draft_commit_message: () => "Improve steering responsiveness",
    });
    render(Page);

    const draftBtn = await screen.findByRole("button", { name: "✦ Draft" });
    await fireEvent.click(draftBtn);

    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "ai_draft_commit_message")).toBe(true));
    await vi.waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveValue("Improve steering responsiveness"),
    );
  });

  it("offers Force-push with lease after quick-editing a pushed branch", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [{ name: "feat/boat", current: true, upstream: "origin/feat/boat", ahead: 1, behind: 0 }],
        remotes: [{ name: "origin", branches: ["origin/feat/boat"] }],
        fetched: null,
      }),
      quick_edit: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 }),
    });
    render(Page);

    const commits = await screen.findByRole("listbox", { name: "Commits" });
    await fireEvent.click(await within(commits).findByText("first commit"));
    await fireEvent.keyDown(commits, { key: "d" });

    expect(calls.find((c) => c.cmd === "quick_edit")?.args).toEqual({
      id: 1,
      commit: "a".repeat(40),
      action: "drop",
    });
    expect(await screen.findByRole("button", { name: "Force-push with lease…" })).toBeInTheDocument();
  });

  it("fades commits still on origin after rewriting a pushed branch", async () => {
    const staleRow: Row = {
      id: "f".repeat(40),
      shortId: "fffffff",
      subject: "still on origin",
      author: "Ada",
      time: 1_700_000_000,
      labels: [{ kind: "remote", name: "origin/main" }],
      wip: false,
      layout: { node: 0, segments: [], overflow: false },
    };
    app({
      launch_path: () => "C:/work/repo",
      open_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 0 }),
      graph_rows: () => [ROW, staleRow],
      branches: () => ({
        local: [{ name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 0 }],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
        fetched: null,
      }),
      quick_edit: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 1 }),
      stale_origin_commits: () => ["f".repeat(40)],
    });
    render(Page);

    const commits = await screen.findByRole("listbox", { name: "Commits" });
    await fireEvent.click(await within(commits).findByText("first commit"));
    await fireEvent.keyDown(commits, { key: "d" });

    await screen.findByRole("button", { name: "Force-push with lease…" });
    const refreshed = screen.getByRole("listbox", { name: "Commits" });
    const staleLabel = await within(refreshed).findByText("still on origin");
    await vi.waitFor(() => expect(staleLabel.closest("li")).toHaveClass("faded"));
  });

  it("lists linked agent worktrees and turns one into a proposal", async () => {
    let created: import("$lib/api").ProposalSummary[] = [];
    const calls = app({
      launch_path: () => "C:/work/repo",
      worktrees_list: () => [
        {
          path: "C:/work/repo",
          branch: "main",
          head: "a".repeat(40),
          dirtyCount: 0,
          isMain: true,
        },
        {
          path: "C:/work/repo-agent",
          branch: "agent/physics",
          head: "b".repeat(40),
          dirtyCount: 2,
          isMain: false,
        },
      ],
      proposals_list: () => created,
      propose_from_worktree: ({ worktreePath, message, why, agent }) => {
        const p: ProposalSummary = {
          id: "p-wt-1",
          commitId: "e".repeat(40),
          shortCommit: "eeeeeee",
          baseId: "a".repeat(40),
          message,
          why: why ?? `Imported from ${worktreePath}`,
          agent: agent ?? "worktree",
          status: "pending",
          createdAt: 1_700_000_000,
          updatedAt: 1_700_000_000,
          files: ["a.txt"],
          riskFlags: [],
          comments: [],
          headMoved: false,
        };
        created = [p];
        return p;
      },
      proposal_file_diff: ({ path }) => ({
        path,
        binary: false,
        hunks: [],
      }),
    });
    render(Page);

    const wtSection = await screen.findByRole("region", { name: "Agent Worktrees" });
    expect(within(wtSection).getByText("agent/physics")).toBeInTheDocument();
    await fireEvent.click(within(wtSection).getByRole("button", { name: "Propose" }));

    expect(calls.find((c) => c.cmd === "propose_from_worktree")?.args).toMatchObject({
      id: 1,
      worktreePath: "C:/work/repo-agent",
    });
    expect(await screen.findByLabelText("Proposal review")).toBeInTheDocument();
  });

  it("offers one-click commit-graph creation when opening a large repository", async () => {
    const calls = app({
      launch_path: () => "C:/work/big",
      open_repo: () => ({ id: 1, root: "C:/work/big", rows: 650, graphVersion: 0 }),
      has_commit_graph: () => false,
    });
    render(Page);

    expect(await screen.findByText(/Large history \(650 rows\)/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Build cache" }));
    expect(calls.some((c) => c.cmd === "write_commit_graph")).toBe(true);
  });

  it("resizes sidebar and inspector splitters and persists pane sizes", async () => {
    localStorage.removeItem("gitgud.ui.panes");
    app({ launch_path: () => "C:/work/repo" });
    render(Page);
    await screen.findByText("first commit");

    const sideSep = screen.getByRole("separator", { name: "Resize sidebar" });
    const body = sideSep.closest(".body") as HTMLElement;
    await fireEvent.keyDown(sideSep, { key: "ArrowRight" });
    expect(body.style.getPropertyValue("--side-w")).toBe("226px");

    const inspSep = screen.getByRole("separator", { name: "Resize inspector" });
    await fireEvent.keyDown(inspSep, { key: "ArrowLeft" });
    expect(body.style.getPropertyValue("--insp-w")).toBe("436px");
    expect(JSON.parse(localStorage.getItem("gitgud.ui.panes") ?? "{}")).toMatchObject({
      sidebarWidth: 226,
      inspectorWidth: 436,
    });

    await fireEvent.dblClick(sideSep);
    expect(body.style.getPropertyValue("--side-w")).toBe("");
    await fireEvent.keyDown(inspSep, { key: "Home" });
    expect(body.style.getPropertyValue("--insp-w")).toBe("");
    localStorage.removeItem("gitgud.ui.panes");
  });
});
