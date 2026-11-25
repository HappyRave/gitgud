import { afterEach, describe, expect, it } from "vitest";
import type { ModelInfo, ModelMaterial } from "$lib/api";
import { prefs, setTextureMax, TEXTURE_MAX_DEFAULT } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { warmScene } from "./warm";

afterEach(() => setTextureMax(TEXTURE_MAX_DEFAULT));

const mat = (baseMap: string | null, normalMap: string | null) => ({ baseMap, normalMap }) as ModelMaterial;
const scene = (materials: ModelMaterial[]): ModelInfo => ({ kind: "prefab", key: null, materials, remaps: {}, scale: 1, scene: null, fromDisk: 0 });

describe("warmScene", () => {
  it("asks for each side's scene info, then for the textures its materials use", async () => {
    const calls = mockCommands({
      model_info: () => scene([mat("Art/wall.png", "Art/wall_n.png"), mat("Art/wall.png", null), mat(null, null)]),
      warm_textures: () => null,
    });
    warmScene(4, "Scenes/Room.unity", [
      { label: "HEAD", at: { at: "head" } },
      { label: "On disk", at: { at: "worktree" } },
    ]);
    await new Promise((r) => setTimeout(r, 0));
    expect(calls.filter((c) => c.cmd === "model_info").map((c) => (c.args as { version: unknown }).version)).toEqual(["head", "worktree"]);
    const warmed = calls.filter((c) => c.cmd === "warm_textures").map((c) => c.args);
    expect(warmed).toEqual([
      { id: 4, version: "head", paths: ["Art/wall.png", "Art/wall_n.png"], max: prefs.textureMax, bc: false },
      { id: 4, version: "worktree", paths: ["Art/wall.png", "Art/wall_n.png"], max: prefs.textureMax, bc: false },
    ]);
  });

  it("leaves native-size textures alone: those aren't prepared", async () => {
    setTextureMax(0);
    const calls = mockCommands({ model_info: () => scene([mat("Art/wall.png", null)]) });
    warmScene(4, "Scenes/Room.unity", [{ label: "HEAD", at: { at: "head" } }]);
    await new Promise((r) => setTimeout(r, 0));
    expect(calls.some((c) => c.cmd === "warm_textures")).toBe(false);
  });
});
