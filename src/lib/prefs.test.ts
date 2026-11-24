import { beforeEach, describe, expect, it, vi } from "vitest";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});

describe("remembered views", () => {
  it("opens each format in its first view, then the one last chosen for that format", async () => {
    const { setViewMode, viewModeFor } = await import("./prefs.svelte");
    expect(viewModeFor("A/Skiff.prefab")).toBe("visual");
    setViewMode("A/Skiff.prefab", "structure");
    expect(viewModeFor("B/Other.prefab")).toBe("structure");
    expect(viewModeFor("Meshes/Hull.asset")).toBe("visual");
    expect(JSON.parse(localStorage.getItem("gitgud.views")!)).toEqual({ prefab: "structure" });
  });

  it("ignores a remembered view the file doesn't offer", async () => {
    localStorage.setItem("gitgud.views", JSON.stringify({ fbx: "source", gltf: "nonsense" }));
    const { viewModeFor } = await import("./prefs.svelte");
    expect(viewModeFor("Boat.fbx")).toBe("visual");
    expect(viewModeFor("Boat.gltf")).toBe("visual");
  });

  it("carries R6's prefab Text choice over as Structure", async () => {
    localStorage.setItem("gitgud.model.view", "source");
    const { viewModeFor } = await import("./prefs.svelte");
    expect(viewModeFor("Skiff.prefab")).toBe("structure");
  });
});
