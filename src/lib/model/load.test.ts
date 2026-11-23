import { type Mesh, MeshBasicMaterial, MeshStandardMaterial, Texture } from "three";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ModelInfo, ModelMaterial } from "$lib/api";
import { loadModel, ModelLoadError } from "./load";
import { materialFrom } from "./materials";

const realFetch = globalThis.fetch;

function serve(files: Record<string, string | ArrayBuffer>) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string | Request) => {
      const url = typeof input === "string" ? input : input.url;
      if (url.startsWith("data:")) {
        return realFetch(url);
      }
      return url in files ? new Response(files[url]) : new Response("x.fbx is not in that version", { status: 404 });
    }),
  );
}

afterEach(() => vi.unstubAllGlobals());

const b64 = (values: number[]) => Buffer.from(new Float32Array(values).buffer).toString("base64");

const gltf = JSON.stringify({
  asset: { version: "2.0" },
  scene: 0,
  scenes: [{ nodes: [0] }],
  nodes: [{ name: "Tri", mesh: 0 }],
  meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }],
  materials: [{ name: "HullMat" }],
  buffers: [
    { byteLength: 36, uri: `data:application/octet-stream;base64,${b64([0, 0, 0, 1, 0, 0, 0, 1, 0])}` },
    { byteLength: 32, uri: `data:application/octet-stream;base64,${b64([0, 1, 0, 0, 0, 2, 0, 0])}` },
  ],
  bufferViews: [
    { buffer: 0, byteLength: 36 },
    { buffer: 1, byteLength: 8 },
    { buffer: 1, byteOffset: 8, byteLength: 24 },
  ],
  accessors: [
    { bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] },
    { bufferView: 1, componentType: 5126, count: 2, type: "SCALAR", min: [0], max: [1] },
    { bufferView: 2, componentType: 5126, count: 2, type: "VEC3" },
  ],
  animations: [
    {
      name: "Slide",
      samplers: [{ input: 1, output: 2 }],
      channels: [{ sampler: 0, target: { node: 0, path: "translation" } }],
    },
  ],
});

const desc = (over: Partial<ModelMaterial> = {}): ModelMaterial => ({
  name: "Hull",
  path: "Assets/M/hull.mat",
  shader: "BoatAttack/Hull",
  approximated: true,
  unlit: false,
  color: [0.5, 0.25, 1, 1],
  baseMap: "Assets/T/hull.png",
  normalMap: null,
  tiling: [2, 3],
  offset: [0.5, 0],
  metallic: 0.25,
  smoothness: 0.75,
  missing: false,
  ...over,
});

describe("loadModel", () => {
  it("loads glTF with its clips and swaps remapped Unity materials", async () => {
    serve({ "m://boat.gltf": gltf });
    const urls: string[] = [];
    const info: ModelInfo = { kind: "gltf", materials: [desc()], remaps: { HullMat: 0 }, scale: 1, scene: null };
    const model = await loadModel({
      kind: "gltf",
      url: "m://boat.gltf",
      refUrl: (r) => `ref://${r}`,
      imageUrl: (p) => `img://${p}`,
      info,
      loadTexture: (u) => {
        urls.push(u);
        return new Texture();
      },
    });
    const tri = model.root.getObjectByName("Tri") as Mesh;
    expect(tri.isMesh).toBe(true);
    const m = tri.material as MeshStandardMaterial;
    expect(m.name).toBe("Hull");
    expect(m.roughness).toBeCloseTo(0.25);
    expect(m.map?.repeat.toArray()).toEqual([2, 3]);
    expect(urls).toEqual(["img://Assets/T/hull.png"]);
    expect(model.clips.map((c) => [c.name, c.duration])).toEqual([["Slide", 1]]);
    expect(model.approximated).toEqual(["BoatAttack/Hull"]);
  });

  it("reports the download as it streams, then parsing", async () => {
    const big = "o Box\n" + "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n".repeat(2000);
    serve({ "m://a.obj": big });
    const steps: string[] = [];
    await loadModel({
      kind: "obj",
      url: "m://a.obj",
      name: "a.obj",
      refUrl: (r) => r,
      imageUrl: (p) => p,
      info: null,
      onProgress: (s) => steps.push(s.phase === "download" ? `download ${s.loaded}` : s.phase),
    });
    expect(steps[0]).toBe("request");
    expect(steps).toContain(`download ${big.length}`);
    expect(steps.slice(-2)).toEqual(["parse", "done"]);
  });

  it("loads OBJ text", async () => {
    serve({ "m://a.obj": "o Box\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n" });
    const model = await loadModel({ kind: "obj", url: "m://a.obj", refUrl: (r) => r, imageUrl: (p) => p, info: null });
    expect(model.root.getObjectByName("Box")).toBeTruthy();
    expect(model.clips).toEqual([]);
  });

  it("reports the backend's reason when the file can't be served", async () => {
    serve({});
    const load = loadModel({ kind: "fbx", url: "m://x.fbx", refUrl: (r) => r, imageUrl: (p) => p, info: null });
    await expect(load).rejects.toBeInstanceOf(ModelLoadError);
    await expect(load).rejects.toThrow("x.fbx is not in that version");
  });
});

describe("materialFrom", () => {
  it("renders missing materials magenta and unlit shaders flat", () => {
    const missing = materialFrom(desc({ missing: true }), (p) => p, () => new Texture());
    expect(missing).toBeInstanceOf(MeshBasicMaterial);
    expect((missing as MeshBasicMaterial).color.getHexString()).toBe("ff00ff");
    const unlit = materialFrom(desc({ unlit: true, baseMap: null }), (p) => p, () => new Texture());
    expect(unlit).toBeInstanceOf(MeshBasicMaterial);
  });
});
