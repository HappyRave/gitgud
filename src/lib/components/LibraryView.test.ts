import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { LibrarySnapshot } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import LibraryView from "./LibraryView.svelte";

const GIT = Promise.resolve({ path: "C:/git/git.exe", version: "2.56.1" });

const SNAP: LibrarySnapshot = {
  version: 1,
  folders: [
    { id: "f-studio", name: "Studio", parentId: null, order: 0 },
    { id: "f-vr", name: "VR", parentId: "f-studio", order: 0 },
  ],
  repos: [
    {
      id: "r-vr",
      name: "quest-racer",
      path: "D:/Work/quest-racer",
      folderId: "f-vr",
      pinned: true,
      order: 0,
      lastOpened: 1000,
      cached: {
        path: "D:/Work/quest-racer",
        missing: false,
        branch: "develop",
        detached: false,
        dirtyCount: 4,
        untrackedCount: 1,
        ahead: 1,
        behind: 2,
        fetchedAt: 900,
        locksHeld: 2,
        locksCheckedAt: 900,
        lastActivity: 1000,
        host: "gitHub",
        isUnity: true,
        checkedAt: 1000,
      },
    },
    {
      id: "r-miss",
      name: "old-jam",
      path: "D:/Gone/old-jam",
      folderId: null,
      pinned: false,
      order: 1,
      lastOpened: 500,
      cached: {
        path: "D:/Gone/old-jam",
        missing: true,
        branch: null,
        detached: false,
        dirtyCount: 0,
        untrackedCount: 0,
        ahead: 0,
        behind: 0,
        fetchedAt: null,
        locksHeld: 0,
        locksCheckedAt: null,
        lastActivity: 500,
        host: "bitbucket",
        isUnity: false,
        checkedAt: 1000,
      },
    },
  ],
  tabs: { openPaths: [], activePath: null },
  undoAction: "Move folder",
  redoAction: null,
};

function setup(snap = SNAP) {
  const calls = mockCommands({
    library_create_folder: ({ name, parentId }) => ({
      ...snap,
      folders: [...snap.folders, { id: "f-new", name, parentId, order: 1 }],
    }),
    library_pin_repo: ({ id, pinned }) => ({
      ...snap,
      repos: snap.repos.map((r) => (r.id === id ? { ...r, pinned } : r)),
    }),
    library_remove_repo: ({ id }) => ({
      ...snap,
      repos: snap.repos.filter((r) => r.id !== id),
    }),
    library_undo: () => snap,
  });
  const onopen = vi.fn();
  const onlocate = vi.fn();
  const onfetchvisible = vi.fn();
  const onlibrarychange = vi.fn();
  render(LibraryView, {
    active: true,
    library: snap,
    openPaths: new Set<string>(),
    git: GIT,
    onopen,
    onadd: vi.fn(),
    onscan: vi.fn(),
    onclone: vi.fn(),
    onlocate,
    onfetchvisible,
    onlibrarychange,
    onrefreshsummaries: vi.fn(),
    onerror: vi.fn(),
  });
  return { calls, onopen, onlocate, onfetchvisible, onlibrarychange };
}

describe("LibraryView", () => {
  it("draws a hollow dot when a repo only has untracked files", () => {
    const repo = SNAP.repos[0];
    setup({
      ...SNAP,
      repos: [{ ...repo, cached: { ...repo.cached!, dirtyCount: 2, untrackedCount: 2 } }],
    });
    const [dot, cell] = screen.getAllByTitle("2 untracked");
    expect(dot).toHaveClass("dot", "hollow");
    expect(cell).toHaveTextContent("2 untracked");
  });

  it("marks Unity projects with the Unity logo", () => {
    setup();
    expect(screen.getByRole("img", { name: "Unity project" }).tagName).toBe("svg");
  });

  it("draws a solid dot when tracked files changed", () => {
    setup();
    const [dot, cell] = screen.getAllByTitle("3 modified · 1 untracked");
    expect(dot).not.toHaveClass("hollow");
    expect(cell).toHaveTextContent("4 changed");
  });

  it("filters by smart filter, creates folders, handles missing Locate, fetch visible, and Ctrl+Z undo", async () => {
    const { calls, onopen, onlocate, onfetchvisible, onlibrarychange } = setup();

    expect(screen.getByText("quest-racer")).toBeInTheDocument();
    expect(screen.getByText("old-jam")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /Missing on disk/ }));
    expect(screen.queryByText("quest-racer")).not.toBeInTheDocument();
    expect(screen.getByText("old-jam")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Locate…" }));
    expect(onlocate).toHaveBeenCalledWith("r-miss");

    await fireEvent.click(screen.getByRole("button", { name: /All repositories/ }));
    await fireEvent.click(screen.getByRole("button", { name: "↓ Fetch visible" }));
    expect(onfetchvisible).toHaveBeenCalledWith(["D:/Work/quest-racer"]);

    await fireEvent.input(screen.getByRole("textbox", { name: "Filter repositories" }), { target: { value: "quest" } });
    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Filter repositories" }), { key: "Enter" });
    expect(onopen).toHaveBeenCalledWith("D:/Work/quest-racer");

    await fireEvent.click(screen.getByRole("button", { name: "+ new" }));
    const folderBox = screen.getByRole("textbox", { name: "New folder name" });
    await fireEvent.input(folderBox, { target: { value: "Jams" } });
    await fireEvent.keyDown(folderBox, { key: "Enter" });
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "library_create_folder")?.args).toEqual({ name: "Jams", parentId: null }),
    );
    expect(onlibrarychange).toHaveBeenCalled();

    await fireEvent.click(within(screen.getByRole("region", { name: "Repositories" })).getByRole("button", { name: "Unpin quest-racer" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "library_pin_repo")?.args).toEqual({ id: "r-vr", pinned: false }),
    );

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "library_undo")).toBe(true));
  });
});
