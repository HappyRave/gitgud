import { describe, expect, it } from "vitest";
import { isModel, meshFileKind, modelKind, modelUrl, refUrl, toVersion } from "./source";

describe("model sources", () => {
  it("recognises model files by extension", () => {
    expect(isModel("Art/Boat.FBX")).toBe(true);
    expect(isModel("a.glb")).toBe(true);
    expect(isModel("Prefabs/Skiff.prefab")).toBe(true);
    expect(isModel("a.png")).toBe(false);
    expect(modelKind("x.GLTF")).toBe("gltf");
    expect(modelKind("x.mat")).toBeNull();
    expect(modelKind("Scenes/Harbour.unity")).toBe("prefab");
    expect(meshFileKind("Meshes/Hull.asset")).toBe("asset");
  });

  it("turns image versions into backend versions", () => {
    expect(toVersion({ at: "head" })).toBe("head");
    expect(toVersion({ at: "commit", commit: "abc" })).toEqual({ commit: "abc" });
    expect(toVersion({ at: "parent", commit: "abc" })).toEqual({ parent: "abc" });
  });

  it("builds protocol URLs for the model and what it refers to", () => {
    expect(modelUrl(3, "Art/a b.fbx", { at: "commit", commit: "c1" }, 7, true)).toBe(
      "http://gitgud.localhost/model?repo=3&path=Art%2Fa+b.fbx&at=commit&commit=c1&v=7",
    );
    expect(refUrl(3, "Art/a.fbx", "C:\\tex\\hull.png", { at: "worktree" }, 0, false)).toBe(
      "gitgud://localhost/ref?repo=3&path=Art%2Fa.fbx&at=worktree&v=0&ref=C%3A%5Ctex%5Chull.png",
    );
  });
});
