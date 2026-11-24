import { describe, expect, it } from "vitest";
import type { ClipCurves, CurveKey, CurveTrack } from "$lib/api";
import { byObject, curveCaption, curvePath, evaluate, initialSelection, range, trackId } from "./curves";

const k = (time: number, value: number, slope: number | null = 0): CurveKey => ({ time, value, inSlope: slope, outSlope: slope });
const track = (path: string, label: string, state: CurveTrack["state"] = "same"): CurveTrack => ({
  path,
  label,
  before: [k(0, 0)],
  after: [k(0, 1)],
  state,
  approximated: false,
});

describe("evaluate", () => {
  it("follows Unity's Hermite curve between keys and holds outside them", () => {
    const keys = [k(0, 0), k(1, 1)];
    expect(evaluate(keys, -1)).toBe(0);
    expect(evaluate(keys, 0.5)).toBeCloseTo(0.5);
    expect(evaluate(keys, 0.25)).toBeCloseTo(0.15625);
    expect(evaluate(keys, 2)).toBe(1);
    expect(evaluate([k(0, 0, 1), k(1, 1, 1)], 0.25)).toBeCloseTo(0.25, 5);
  });

  it("steps on an infinite slope and draws lines when asked", () => {
    const keys = [k(0, 0, null), k(1, 1, null)];
    expect(evaluate(keys, 0.9)).toBe(0);
    expect(evaluate([k(0, 0), k(1, 1)], 0.25, true)).toBe(0.25);
  });

  it("finds the range including overshoot and draws a path", () => {
    const [lo, hi] = range([k(0, 0, 4), k(1, 0, 4)], 1, false);
    expect(lo).toBeLessThan(0);
    expect(hi).toBeGreaterThan(0);
    expect(curvePath([k(0, 0), k(1, 1)], 1, 4, (v) => 10 - v * 10, false, 2)).toBe("M0.0,10.0L2.0,5.0L4.0,0.0");
  });
});

it("groups by object, plots changed tracks first and writes the caption", () => {
  const tracks = [track("Hull", "Position.y", "changed"), track("Hull", "Rotation.x"), track("Flag", "Rotation.y", "added")];
  expect(byObject(tracks).map(([o, t]) => [o, t.length])).toEqual([
    ["Hull", 2],
    ["Flag", 1],
  ]);
  expect([...initialSelection(tracks)]).toEqual(["Hull|Position.y", "Flag|Rotation.y"]);
  expect([...initialSelection([track("A", "x")])]).toEqual([trackId({ path: "A", label: "x" })]);
  const clip: ClipCurves = { name: "Bob", duration: 2, looping: true, sampleRate: 60, tracks, flipbooks: [], keysBefore: 8, keysAfter: 15 };
  expect(curveCaption(clip, true).map((p) => p.text)).toEqual(["1 track changed", "1 added", "keys 8 → 15"]);
  expect(curveCaption(clip, false).map((p) => p.text)).toEqual(["3 tracks · 15 keys"]);
});
