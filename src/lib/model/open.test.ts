import { Group } from "three";
import { describe, expect, it, vi } from "vitest";
import type { ModelInfo } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import { openSide } from "./open";
import { buildPrefab, type PrefabContext } from "./prefab";

vi.mock("./prefab", () => ({
  buildPrefab: vi.fn(() => ({ root: new Group(), clips: [], approximated: [], missing: [], load: vi.fn() })),
}));

const info = (over: Partial<ModelInfo>): ModelInfo => ({ kind: "fbx", key: null, materials: [], remaps: {}, scale: 1, scene: null, fromDisk: 0, ...over });

describe("openSide", () => {
  it("takes a scene's model infos from the scene, and asks only for the ones it lacks", async () => {
    const calls = mockCommands({
      model_info: ({ path }) =>
        path.endsWith(".unity")
          ? info({ kind: "prefab", scene: { nodes: [], missing: [], clips: [] }, models: { "Assets/boat.fbx": info({ scale: 2 }) } })
          : info({ scale: 3 }),
    });
    const opened = await openSide(1, "Assets/Room.unity", { label: "HEAD", at: { at: "head" } }, 0);
    expect(opened.error).toBeNull();
    const ctx = vi.mocked(buildPrefab).mock.calls[0][2] as PrefabContext;
    expect((await ctx.info("Assets/boat.fbx")).scale).toBe(2);
    expect((await ctx.info("Assets/raft.fbx")).scale).toBe(3);
    expect(calls.filter((c) => c.cmd === "model_info").map((c) => (c.args as { path: string }).path)).toEqual([
      "Assets/Room.unity",
      "Assets/raft.fbx",
    ]);
    expect(ctx.meshesUrl("Assets/Room.unity", ["1", "-2"])).toMatch(/&meshes=1,-2$/);
  });
});
