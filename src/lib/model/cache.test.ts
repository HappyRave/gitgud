import { BufferAttribute, BufferGeometry, Group, Mesh, MeshStandardMaterial, Texture } from "three";
import { describe, expect, it, vi } from "vitest";
import type { ModelInfo, ModelMaterial } from "$lib/api";
import { ModelCache } from "./cache";
import { loadShared, materialKey } from "./shared";
import type { LoadedModel } from "./load";

vi.mock("./load", async (orig) => ({
  ...(await orig<typeof import("./load")>()),
  loadModel: vi.fn(async () => model(12)),
}));

function image(width: number, height: number): Texture {
  const t = new Texture();
  t.image = { width, height };
  return t;
}

function model(vertices: number): LoadedModel {
  const geo = new BufferGeometry();
  geo.setAttribute("position", new BufferAttribute(new Float32Array(vertices * 3), 3));
  const map = image(4, 2);
  const root = new Group();
  root.add(new Mesh(geo, new MeshStandardMaterial({ map })));
  return { root, clips: [], approximated: [], missing: [] };
}

const meshOf = (m: LoadedModel) => m.root.children[0] as Mesh<BufferGeometry, MeshStandardMaterial>;

describe("ModelCache", () => {
  it("loads a key once, marks what it holds shared and counts its memory", async () => {
    const cache = new ModelCache();
    const load = vi.fn(async () => model(10));
    const a = await cache.model("k", load);
    const b = await cache.model("k", load);
    expect(load).toHaveBeenCalledTimes(1);
    expect(a).toBe(b);
    const mesh = meshOf(a);
    expect(mesh.geometry.userData.shared).toBe(true);
    expect(mesh.material.userData.shared).toBe(true);
    expect(mesh.material.map?.userData.shared).toBe(true);
    expect(cache.stats()).toEqual({ models: 1, materials: 0, bytes: 10 * 12 + 4 * 2 * 4 });
  });

  it("doesn't keep a failed load", async () => {
    const cache = new ModelCache();
    const load = vi.fn().mockRejectedValueOnce(new Error("not in that version")).mockResolvedValueOnce(model(1));
    await expect(cache.model("k", load)).rejects.toThrow("not in that version");
    await expect(cache.model("k", load)).resolves.toBeTruthy();
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("drops the least recently used entry when over budget, and frees it", async () => {
    const cache = new ModelCache();
    cache.budget = 3 * 1200;
    const old = await cache.model("old", async () => model(100));
    const kept = await cache.model("kept", async () => model(100));
    await cache.model("old", async () => model(100));
    await cache.model("kept", async () => model(100));
    const dispose = vi.spyOn(meshOf(old).geometry, "dispose");
    await cache.model("new", async () => model(100));
    await Promise.resolve();
    expect(dispose).toHaveBeenCalled();
    expect(cache.stats().models).toBe(2);
    expect(await cache.model("kept", async () => model(1))).toBe(kept);
  });

  it("shares materials by key and frees everything on clear", async () => {
    const cache = new ModelCache();
    const make = vi.fn(() => new MeshStandardMaterial({ map: image(2, 2) }));
    const m = cache.material("mat", make);
    expect(cache.material("mat", make)).toBe(m);
    expect(make).toHaveBeenCalledTimes(1);
    const loaded = await cache.model("k", async () => model(5));
    const disposeMat = vi.spyOn(m, "dispose");
    const disposeGeo = vi.spyOn(meshOf(loaded).geometry, "dispose");
    expect(cache.stats()).toEqual({ models: 1, materials: 1, bytes: 5 * 12 + 32 + 16 });

    cache.clear();
    expect(disposeMat).toHaveBeenCalled();
    expect(disposeGeo).toHaveBeenCalled();
    expect(cache.stats()).toEqual({ models: 0, materials: 0, bytes: 0 });
  });
});

describe("loadShared", () => {
  const info = (key: string | null): ModelInfo => ({ kind: "fbx", key, materials: [], remaps: {}, scale: 1, scene: null, fromDisk: 0 });
  const src = (key: string | null, onProgress = vi.fn()) => ({
    kind: "fbx" as const,
    url: "m://a.fbx",
    refUrl: (r: string) => r,
    imageUrl: (p: string) => p,
    info: info(key),
    onProgress,
  });

  it("loads a named file once and reports it done each time", async () => {
    const { loadModel } = await import("./load");
    vi.mocked(loadModel).mockClear();
    const cache = new ModelCache();
    const progress = vi.fn();
    const a = await loadShared(src("blob1"), cache);
    expect(await loadShared(src("blob1", progress), cache)).toBe(a);
    expect(progress).toHaveBeenCalledWith({ phase: "done" });
    await loadShared(src("blob2"), cache);
    await loadShared(src(null), cache);
    await loadShared(src(null), cache);
    expect(loadModel).toHaveBeenCalledTimes(4);
  });
});

describe("materialKey", () => {
  const desc = (over: Partial<ModelMaterial>): ModelMaterial => ({
    name: "Hull",
    path: null,
    shader: "Standard",
    approximated: false,
    unlit: false,
    color: [1, 1, 1, 1],
    baseMap: null,
    normalMap: null,
    baseMapKey: null,
    normalMapKey: null,
    tiling: [1, 1],
    offset: [0, 0],
    metallic: 0,
    smoothness: 0.5,
    missing: false,
    textures: [],
    baseProperty: null,
    baseCandidates: [],
    ...over,
  });

  it("names a material only when its textures are named by content", () => {
    expect(materialKey(desc({}))).toMatch(/^mat\|/);
    expect(materialKey(desc({ baseMap: "a.png" }))).toBeNull();
    expect(materialKey(desc({ baseMap: "a.png", baseMapKey: "b1" }))).not.toBe(materialKey(desc({ baseMap: "a.png", baseMapKey: "b2" })));
  });
});
