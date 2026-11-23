import { describe, expect, it } from "vitest";
import type { LibraryFolder, LibraryRepo } from "$lib/api";
import {
  buildFolderTree,
  changesTitle,
  descendantFolderIds,
  filterAndGroupRepos,
  folderPathLabel,
  scoreRepoMatch,
  smartCounts,
} from "./filter";

const FOLDERS: LibraryFolder[] = [
  { id: "f-studio", name: "Studio", parentId: null, order: 0 },
  { id: "f-vr", name: "VR", parentId: "f-studio", order: 0 },
  { id: "f-tools", name: "Tools", parentId: null, order: 1 },
];

const mkRepo = (id: string, name: string, folderId: string | null, over: Partial<LibraryRepo> = {}): LibraryRepo => ({
  id,
  name,
  path: `D:/Work/${name}`,
  folderId,
  pinned: false,
  order: 0,
  lastOpened: null,
  cached: {
    path: `D:/Work/${name}`,
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
    host: "gitHub",
    isUnity: false,
    checkedAt: 100,
    ...(over.cached ?? {}),
  },
  ...over,
});

const REPOS: LibraryRepo[] = [
  mkRepo("r-1", "quest-racer", "f-vr", {
    pinned: true,
    lastOpened: 500,
    cached: {
      path: "D:/Work/quest-racer",
      missing: false,
      branch: "develop",
      detached: false,
      dirtyCount: 2,
      untrackedCount: 0,
      ahead: 1,
      behind: 0,
      fetchedAt: null,
      locksHeld: 2,
      locksCheckedAt: 400,
      lastActivity: 500,
      host: "bitbucket",
      isUnity: true,
      checkedAt: 500,
    },
  }),
  mkRepo("r-2", "gitgud", "f-tools", { lastOpened: 300 }),
  mkRepo("r-3", "old-jam", null, {
    cached: {
      path: "D:/Work/old-jam",
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
      lastActivity: 50,
      host: "none",
      isUnity: true,
      checkedAt: 100,
    },
  }),
];

describe("library filter & tree helpers", () => {
  it("computes recursive folder counts and respects collapsed folders", () => {
    const expanded = buildFolderTree(FOLDERS, REPOS, new Set());
    expect(expanded.map((n) => [n.folder.name, n.depth, n.totalCount])).toEqual([
      ["Studio", 0, 1],
      ["VR", 1, 1],
      ["Tools", 0, 1],
    ]);

    const collapsed = buildFolderTree(FOLDERS, REPOS, new Set(["f-studio"]));
    expect(collapsed.map((n) => n.folder.name)).toEqual(["Studio", "Tools"]);
    expect([...descendantFolderIds(FOLDERS, "f-studio")]).toEqual(["f-studio", "f-vr"]);
    expect(folderPathLabel(FOLDERS, "f-vr")).toBe("Studio / VR");
  });

  it("computes smart filter counts accurately", () => {
    expect(smartCounts(REPOS)).toEqual({
      all: 3,
      recent: 3,
      pinned: 1,
      uncommitted: 1,
      notPushed: 1,
      locks: 1,
      unity: 2,
      missing: 1,
    });
  });

  it("filters by folder, smart filter, and search query", () => {
    const vr = filterAndGroupRepos(FOLDERS, REPOS, { kind: "folder", folderId: "f-studio" }, "", new Set());
    expect(vr.flat.map((r) => r.name)).toEqual(["quest-racer"]);

    const searched = filterAndGroupRepos(FOLDERS, REPOS, { kind: "smart", filter: "all" }, "bitbucket", new Set());
    expect(searched.flat.map((r) => r.name)).toEqual(["quest-racer"]);

    const missing = filterAndGroupRepos(FOLDERS, REPOS, { kind: "smart", filter: "missing" }, "", new Set());
    expect(missing.flat.map((r) => r.name)).toEqual(["old-jam"]);
  });

  it("ranks QuickSwitch matches prioritizing name and open tabs", () => {
    const s1 = scoreRepoMatch(REPOS[0], "Studio / VR", "qr", true);
    const s2 = scoreRepoMatch(REPOS[1], "Tools", "qr", false);
    expect(s1).not.toBeNull();
    expect(s2).toBeNull();
    const openBonus = scoreRepoMatch(REPOS[1], "Tools", "git", true)!;
    const closedScore = scoreRepoMatch(REPOS[1], "Tools", "git", false)!;
    expect(openBonus).toBeGreaterThan(closedScore);
  });
});

describe("changesTitle", () => {
  const summary = (dirtyCount: number, untrackedCount: number) =>
    mkRepo("r", "r", null, { cached: { ...mkRepo("r", "r", null).cached!, dirtyCount, untrackedCount } }).cached!;

  it("splits modified from untracked", () => {
    expect(changesTitle(summary(3, 1))).toBe("2 modified · 1 untracked");
    expect(changesTitle(summary(3, 0))).toBe("3 modified");
    expect(changesTitle(summary(2, 2))).toBe("2 untracked");
  });
});
