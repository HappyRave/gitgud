import { describe, expect, it } from "vitest";
import type { ModelInfo } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import { sceneInfo, warmSceneInfo } from "./sceneInfo";

const info = (scale: number): ModelInfo => ({ kind: "prefab", key: null, materials: [], remaps: {}, scale, scene: null, fromDisk: 0 });

describe("sceneInfo", () => {
  it("hands a warmed scene info to the opening once, then asks again", async () => {
    let scale = 1;
    const calls = mockCommands({ model_info: () => info(scale++) });
    const version = { commit: "abc" };
    const warm = warmSceneInfo(1, "Scenes/Room.unity", version, 0);
    expect(warmSceneInfo(1, "Scenes/Room.unity", version, 0)).toBe(warm);
    expect(await sceneInfo(1, "Scenes/Room.unity", version, 0)).toEqual(info(1));
    expect(await sceneInfo(1, "Scenes/Room.unity", version, 0)).toEqual(info(2));
    expect(await sceneInfo(1, "Scenes/Room.unity", "worktree", 0)).toEqual(info(3));
    expect(calls.filter((c) => c.cmd === "model_info")).toHaveLength(3);
  });

  it("doesn't hand out a warm-up read before the files changed", async () => {
    let scale = 1;
    mockCommands({ model_info: () => info(scale++) });
    await warmSceneInfo(1, "Scenes/Room.unity", "worktree", 3);
    expect(await sceneInfo(1, "Scenes/Room.unity", "worktree", 4)).toEqual(info(2));
  });

  it("doesn't keep a warm-up that failed", async () => {
    let fail = true;
    mockCommands({
      model_info: () => {
        if (fail) {
          throw new Error("not in that version");
        }
        return info(5);
      },
    });
    await expect(warmSceneInfo(1, "Scenes/Gone.unity", "head", 0)).rejects.toThrow("not in that version");
    fail = false;
    expect(await sceneInfo(1, "Scenes/Gone.unity", "head", 0)).toEqual(info(5));
  });
});
