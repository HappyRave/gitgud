import { describe, expect, it } from "vitest";
import type { Overlap, OverlapFile } from "$lib/api";
import { advice, freshHeadsUps, headline, headsUpKey, initials, overlapFor, personColor, stripLabel } from "./radar";

const NOW = 1_800_000_000;

function file(path: string, over: Partial<OverlapFile> = {}): OverlapFile {
  return {
    path,
    risk: "wontMerge",
    on: "origin/PartyGame",
    authors: ["JG"],
    commits: 1,
    newest: NOW - 40 * 60,
    newestCommit: `c-${path}`,
    dirty: true,
    ...over,
  };
}

const lobby = file("Assets/Scenes/Playfield 1 - Lobby.unity");
const code = file("Assets/Code.cs", { risk: "mayConflict", authors: ["Jean Paul"] });
const other = file("Assets/Other.cs", { risk: "merges" });
const radar: Overlap = { against: ["origin/PartyGame"], files: [lobby, code, other] };

describe("overlapFor", () => {
  it("finds a row's file and folds a .meta into its asset", () => {
    expect(overlapFor(radar, "Assets/Code.cs")).toBe(code);
    expect(overlapFor(radar, "Assets/Scenes/Playfield 1 - Lobby.unity.meta")).toBe(lobby);
    expect(overlapFor(radar, "Assets/Mine.cs")).toBeNull();
    expect(overlapFor(null, "Assets/Code.cs")).toBeNull();
  });
});

describe("freshHeadsUps", () => {
  it("announces only won't-merge files, once per origin commit", () => {
    expect(freshHeadsUps(radar, new Set())).toEqual([lobby]);
    expect(freshHeadsUps(radar, new Set([headsUpKey(lobby)]))).toEqual([]);
    const newer = { ...lobby, newestCommit: "c-next" };
    expect(freshHeadsUps({ ...radar, files: [newer] }, new Set([headsUpKey(lobby)]))).toEqual([newer]);
  });
});

describe("words", () => {
  it("names people by initials the way the team writes them", () => {
    expect(initials("Jean Paul")).toBe("JP");
    expect(initials("MD")).toBe("MD");
    expect(initials("max.demol")).toBe("MD");
    expect(initials("sam")).toBe("S");
    expect(personColor("JG")).toBe(personColor("JG"));
  });

  it("says what changed, when, and what to do", () => {
    expect(headline(lobby, NOW)).toBe("Playfield 1 - Lobby.unity changed on origin/PartyGame 40 min ago");
    expect(headline({ ...lobby, newest: NOW - 20 }, NOW)).toContain("just now");
    expect(advice(lobby)).toMatch(/^JG changed it and you're editing it/);
    expect(advice({ ...lobby, dirty: false })).toContain("a commit you haven't pushed");
  });

  it("counts for the strip, won't-merge first", () => {
    expect(stripLabel(radar)).toEqual({ text: "1 file won't merge with origin/PartyGame", risk: "wontMerge" });
    expect(stripLabel({ ...radar, files: [code, other] })).toEqual({
      text: "2 files also changed on origin/PartyGame",
      risk: "mayConflict",
    });
    expect(stripLabel({ ...radar, files: [] })).toBeNull();
    expect(stripLabel(null)).toBeNull();
  });
});
