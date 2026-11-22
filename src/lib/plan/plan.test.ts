import { describe as group, expect, it } from "vitest";
import type { HistoryPlan, PlanCommit } from "$lib/api";
import { describe, editsSquashMessage, firstConflict, move, planRows, setAction, setMessage, steps, target, valid } from "./plan";

const commit = (id: string, subject: string, action: PlanCommit["action"] = "pick"): PlanCommit => ({
  id: id.padEnd(40, "0"),
  shortId: id,
  subject,
  message: subject,
  author: "Max",
  time: 0,
  action,
});

const PLAN: HistoryPlan = {
  branch: "feature/stack",
  base: "6acf5f7".padEnd(40, "0"),
  baseShort: "6acf5f7",
  baseSubject: "Remove EmojiOne assets",
  commits: [commit("3493748", "Move blimp camera rgiht"), commit("6bb2f7d", "fixup! WIP engine", "fixup"), commit("4592a21", "WIP engine")],
  dependents: [],
  pushed: "origin/feature/stack",
};

const shape = (rows: ReturnType<typeof planRows>) => rows.map((r) => `${r.commit.shortId}:${r.action}`);

group("plan rows", () => {
  it("open with the plan's actions, fixups already applied", () => {
    expect(shape(planRows(PLAN))).toEqual(["3493748:pick", "6bb2f7d:fixup", "4592a21:pick"]);
  });

  it("can open with one row preset, as R on an older commit does", () => {
    const rows = planRows(PLAN, { id: PLAN.commits[0].id, action: "reword" });
    expect(rows[0]).toMatchObject({ action: "reword", message: "Move blimp camera rgiht" });
  });

  it("squash prefills both messages, the target's first, and keeps edits when pressed again", () => {
    let rows = setAction(planRows(PLAN), 1, "squash")!;
    expect(rows[1].message).toBe("WIP engine\n\nfixup! WIP engine");
    rows = setMessage(rows, 1, "WIP engine, smoothed");
    expect(setAction(rows, 1, "squash")![1].message).toBe("WIP engine, smoothed");
  });

  it("squash uses the target's new message when it is reworded", () => {
    let rows = setAction(planRows(PLAN), 2, "reword")!;
    rows = setMessage(rows, 2, "Engine thrust");
    expect(setAction(rows, 1, "squash")![1].message).toBe("Engine thrust\n\nfixup! WIP engine");
  });

  it("refuses a fold with nothing older, and a drop that leaves a fold at the bottom", () => {
    const rows = planRows(PLAN);
    expect(setAction(rows, 2, "fixup")).toBeNull();
    expect(setAction(rows, 2, "drop")).toBeNull();
    expect(target(rows, 1)).toBe(2);
  });

  it("fixup and pick clear the message; reword starts from the commit's", () => {
    let rows = setAction(planRows(PLAN), 0, "reword")!;
    rows = setMessage(rows, 0, "Move blimp camera right");
    expect(setAction(rows, 0, "pick")![0].message).toBeNull();
    expect(setAction(rows, 0, "reword")![0].message).toBe("Move blimp camera right");
  });

  it("moves rows, refusing to put a fold at the bottom", () => {
    const rows = planRows(PLAN);
    expect(shape(move(rows, 0, 1)!)).toEqual(["6bb2f7d:fixup", "3493748:pick", "4592a21:pick"]);
    expect(move(rows, 1, 2)).toBeNull();
    expect(move(rows, 0, -1)).toBeNull();
    expect(valid(rows)).toBe(true);
  });

  it("the newest squash of a chain edits its message", () => {
    let rows = setAction(planRows(PLAN), 1, "squash")!;
    rows = setAction(rows, 0, "squash")!;
    expect([0, 1, 2].map((i) => editsSquashMessage(rows, i))).toEqual([true, false, false]);
  });

  it("turns into steps with messages only where they count", () => {
    let rows = setAction(planRows(PLAN), 0, "reword")!;
    rows = setMessage(rows, 0, "Move blimp camera right");
    expect(steps(rows)).toEqual([
      { id: PLAN.commits[0].id, action: "reword", message: "Move blimp camera right" },
      { id: PLAN.commits[1].id, action: "fixup", message: null },
      { id: PLAN.commits[2].id, action: "pick", message: null },
    ]);
    expect(describe(rows)).toBe('Reworded 3493748 → "Move blimp camera right" · folded 6bb2f7d into 4592a21');
  });

  it("finds the oldest conflicting step", () => {
    const conflict = (id: string, step: number) => ({ id, step, outcome: { kind: "conflict" as const, paths: ["a"], tree: "t" } });
    expect(
      firstConflict({ steps: [conflict("b", 2), conflict("a", 1)], before: 2, after: 2, changed: null })?.id,
    ).toBe("a");
    expect(firstConflict({ steps: [], before: 0, after: 0, changed: 0 })).toBeNull();
  });
});
