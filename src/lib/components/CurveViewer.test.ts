import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClipCurves, CurveKey } from "$lib/api";
import type { ModelSide } from "$lib/model/open";
import { prefs, viewerChoice } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { CURVE_MODES } from "$lib/viewers/curves";
import CurveViewer from "./CurveViewer.svelte";

vi.mock("./ModelViewer.svelte", async () => ({ default: (await import("$lib/test/Stub.svelte")).default }));

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };
const k = (time: number, value: number): CurveKey => ({ time, value, inSlope: 0, outSlope: 0 });

const clip: ClipCurves = {
  name: "Bob",
  duration: 2,
  looping: true,
  sampleRate: 60,
  tracks: [
    { path: "Hull", label: "Position.y", before: [k(0, 0), k(1, 0.25)], after: [k(0, 0), k(1, 0.4)], state: "changed", approximated: false },
    { path: "Hull", label: "Rotation.x", before: [k(0, 0)], after: [k(0, 0)], state: "same", approximated: false },
    { path: "Flag", label: "Rotation.y", before: null, after: [k(0, 0), k(2, 30)], state: "added", approximated: false },
  ],
  flipbooks: [{ path: "Icon", label: "Sprite", before: [[0, "fuel_0"]], after: [[0, "fuel_0"], [1, "fuel_1"]], state: "changed" }],
  keysBefore: 5,
  keysAfter: 8,
};

beforeEach(() => {
  prefs.viewer = {};
});

describe("CurveViewer", () => {
  it("plots the changed tracks, HEAD dashed under the working copy, with keys as dots", async () => {
    mockCommands({ clip_curves: () => clip, clip_users: () => [] });
    render(CurveViewer, { repoId: 1, path: "Anims/Bob.anim", before: HEAD, after: DISK });
    expect(await screen.findByText("Clip · 2.00 s · loop")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /Position\.y/ })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Rotation\.x/ })).not.toBeChecked();
    const plot = screen.getByLabelText("Curves");
    expect(plot.querySelectorAll("path.before")).toHaveLength(1);
    expect(plot.querySelectorAll("path.after")).toHaveLength(2);
    expect(document.querySelectorAll(".key")).toHaveLength(4);
    expect(screen.getByText("fuel_1")).toBeInTheDocument();
    expect(screen.getByText("keys 5 → 8")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("checkbox", { name: /Rotation\.x/ }));
    expect(screen.getByLabelText("Curves").querySelectorAll("path.after")).toHaveLength(3);
    expect(screen.getByRole("radio", { name: "3D" })).toBeDisabled();
  });

  it("plays the clip in 3D on a prefab whose Animator uses it", async () => {
    mockCommands({ clip_curves: () => clip, clip_users: () => ["Prefabs/Skiff.prefab", "Prefabs/Tug.prefab"] });
    render(CurveViewer, { repoId: 1, path: "Anims/Bob.anim", before: HEAD, after: DISK });
    await fireEvent.click(await screen.findByRole("radio", { name: "3D" }));
    expect(screen.getByTestId("stub")).toHaveAttribute("data-path", "Prefabs/Skiff.prefab");
    expect(viewerChoice("Other.anim", "curves", CURVE_MODES)).toBe("3d");
    await fireEvent.change(screen.getByLabelText("Prefab that plays this clip"), { target: { value: "Prefabs/Tug.prefab" } });
    expect(screen.getByTestId("stub")).toHaveAttribute("data-path", "Prefabs/Tug.prefab");
  });
});
