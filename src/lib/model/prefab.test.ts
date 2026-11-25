import {
  AnimationClip,
  AnimationMixer,
  BufferAttribute,
  BufferGeometry,
  Group,
  type LineSegments,
  Mesh,
  type MeshStandardMaterial,
  Texture,
  VectorKeyframeTrack,
} from "three";
import { describe, expect, it, vi } from "vitest";
import type { ModelInfo, ModelMaterial, ModelNode, ModelScene } from "$lib/api";
import { loadMeshBundle } from "./fbxPool";
import { assemblePrefab, buildPrefab, builtinGeometry, pivotOffset } from "./prefab";
import { unityFileId } from "./unityId";

vi.mock("./load", async (orig) => ({
  ...(await orig<typeof import("./load")>()),
  loadModel: vi.fn(async ({ url }: { url: string }) => {
    if (url.includes("gone")) {
      throw new Error("Assets/gone.fbx is not in that version");
    }
    const file = new Group();
    file.scale.setScalar(0.01);
    const geo = new BufferGeometry();
    geo.setAttribute("position", new BufferAttribute(new Float32Array([0, 0, 0, 100, 0, 0, 0, 100, 0]), 3));
    const hull = Object.assign(new Mesh(geo), { name: "Hull_Mesh" });
    hull.userData.transformData = { rotationPivot: [1, 2, 3], scalingPivot: [1, 2, 3] };
    file.add(hull);
    const wrapper = new Group();
    wrapper.add(file);
    const take = new AnimationClip("Take 001", 1, [new VectorKeyframeTrack("Hull_Mesh.position", [0, 1], [0, 0, 0, 1, 0, 0])]);
    return { root: wrapper, clips: [take], approximated: [], missing: [] };
  }),
}));

vi.mock("./fbxPool", () => ({
  loadMeshBundle: vi.fn(async (url: string) => {
    const [path, ids] = url.slice("m://".length).split("?meshes=");
    if (path.includes("gone")) {
      throw new Error(`${path} is not in that version`);
    }
    return ids.split(",").map((id) => ({
      id,
      packed: {
        scale: 1,
        clips: [],
        meshes: [
          {
            name: "Mesh",
            originalName: null,
            transformData: null,
            attributes: [{ name: "position", array: new Float32Array(9), itemSize: 3, normalized: false }],
            index: null,
            groups: [],
          },
        ],
      },
    }));
  }),
}));

const mat = (name: string, over: Partial<ModelMaterial> = {}): ModelMaterial => ({
  name,
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
  ...over,
});

const node = (name: string, over: Partial<ModelNode> = {}): ModelNode => ({
  name,
  parent: null,
  position: [0, 0, 0],
  rotation: [0, 0, 0, 1],
  scale: [1, 1, 1],
  active: true,
  mesh: null,
  materials: [],
  skinned: false,
  controller: null,
  ...over,
});

const ctx = {
  modelUrl: (p: string) => `m://${p}`,
  meshesUrl: (p: string, ids: string[]) => `m://${p}?meshes=${ids.join(",")}`,
  refUrl: () => (r: string) => r,
  imageUrl: (p: string) => p,
  info: async (): Promise<ModelInfo> => ({ kind: "fbx", key: null, materials: [], remaps: {}, scale: 1, scene: null, fromDisk: 0 }),
  loadTexture: () => new Texture(),
};

describe("buildPrefab", () => {
  it("shows dots at once, loads on request nearest the camera first, and swaps each dot for its mesh", async () => {
    const scene: ModelScene = {
      nodes: [
        node("Far", { position: [-10, 0, -10], mesh: { type: "inline", path: "Assets/Room.unity", id: "1", key: "far" } }),
        node("Near", { position: [10, 0, 10], mesh: { type: "inline", path: "Assets/Room.unity", id: "2", key: "near" } }),
        node("Empty", { position: [0, 5, 0] }),
      ],
      missing: [],
      clips: [],
    };
    vi.mocked(loadMeshBundle).mockClear();
    const pieces: string[] = [];
    const built = buildPrefab(scene, [], { ...ctx, onPiece: (o) => pieces.push(o.name) });

    const dots = (name: string) => built.root.getObjectByName(name)!.children.filter((c) => c.userData.placeholder).length;
    expect([dots("Far"), dots("Near"), dots("Empty")]).toEqual([1, 1, 0]);
    await new Promise((r) => setTimeout(r, 5));
    expect(loadMeshBundle).not.toHaveBeenCalled();

    const loading = built.load();
    expect(built.load()).toBe(loading);
    expect(await loading).toBe(built);
    expect(vi.mocked(loadMeshBundle).mock.calls.map(([url]) => url)).toEqual(["m://Assets/Room.unity?meshes=2,1"]);
    expect([dots("Far"), dots("Near")]).toEqual([0, 0]);
    expect(pieces.sort()).toEqual(["Far", "Near"]);
    expect((built.root.getObjectByName("Near")!.children[0] as Mesh).isMesh).toBe(true);

    // The other side of a compare: the same contents under other fileIDs come from the first.
    const other = buildPrefab(
      {
        ...scene,
        nodes: [
          node("Near", { position: [10, 0, 10], mesh: { type: "inline", path: "Assets/Room.unity", id: "20", key: "near" } }),
          node("New", { mesh: { type: "inline", path: "Assets/Room.unity", id: "30", key: "new" } }),
        ],
      },
      [],
      ctx,
    );
    await other.load();
    expect(vi.mocked(loadMeshBundle).mock.calls.map(([url]) => url).slice(1)).toEqual(["m://Assets/Room.unity?meshes=30"]);
  });
});

describe("assemblePrefab", () => {
  it("says why a clip from a model file couldn't be read", async () => {
    const scene: ModelScene = {
      nodes: [node("Skiff")],
      missing: [],
      clips: [
        { node: 0, name: "Gone", source: { type: "model", path: "Assets/gone.fbx", take: "Take 001" } },
        { node: 0, name: "Wave", source: { type: "model", path: "Assets/hull.fbx", take: "Wave" } },
      ],
    };
    const model = await assemblePrefab(scene, [], ctx);
    expect(model.missing).toEqual([
      "Clip Gone couldn't be read: Assets/gone.fbx: Assets/gone.fbx is not in that version",
    ]);
    expect(model.clips.map((c) => c.name)).toEqual(["Wave"]);
  });

  it("takes a Mesh .asset's one mesh whatever fileID points at it", async () => {
    const scene: ModelScene = {
      nodes: [node("Floor", { mesh: { type: "asset", path: "Assets/Meshes/Floor.asset", name: null, id: "4300000", scale: 1 } })],
      missing: [],
      clips: [],
    };
    const model = await assemblePrefab(scene, [], ctx);
    expect((model.root.getObjectByName("Floor")!.children[0] as Mesh).isMesh).toBe(true);
    expect(model.missing).toEqual([]);
  });

  it("draws meshes stored inside the scene, loading each one once", async () => {
    vi.mocked(loadMeshBundle).mockClear();
    const scene: ModelScene = {
      nodes: [
        node("Wall_A", { mesh: { type: "inline", path: "Assets/Room.unity", id: "1234", key: "wall" }, materials: [0] }),
        node("Wall_B", { mesh: { type: "inline", path: "Assets/Room.unity", id: "1234", key: "wall" }, materials: [0] }),
        node("Floor", { mesh: { type: "inline", path: "Assets/gone.unity", id: "9", key: "floor" } }),
      ],
      missing: [],
      clips: [],
    };
    const model = await assemblePrefab(scene, [mat("Wall")], ctx);
    const wall = model.root.getObjectByName("Wall_A")!.children[0] as Mesh;
    expect(wall.isMesh).toBe(true);
    expect((wall.material as MeshStandardMaterial).name).toBe("Wall");
    expect(vi.mocked(loadMeshBundle).mock.calls.map(([url]) => url).sort()).toEqual([
      "m://Assets/Room.unity?meshes=1234",
      "m://Assets/gone.unity?meshes=9",
    ]);
    expect(model.missing).toEqual(["Assets/gone.unity › Mesh &9: Assets/gone.unity is not in that version"]);
  });

  it("builds the hierarchy with FBX meshes, primitives and Unity materials", async () => {
    const scene: ModelScene = {
      nodes: [
        node("Skiff", { position: [-1, 0, 0] }),
        node("Hull", {
          parent: 0,
          mesh: { type: "asset", path: "Assets/hull.fbx", name: null, id: unityFileId("Mesh", "Hull_Mesh"), scale: 1 },
          materials: [0],
        }),
        node("Crate", { parent: 0, mesh: { type: "builtin", name: "Cube" }, materials: [1], active: false }),
        node("Gone", { parent: 0, mesh: { type: "asset", path: "Assets/gone.fbx", name: "X", id: null, scale: 1 } }),
      ],
      missing: ["Lost › MeshFilter.m_Mesh → guid dead…007 (not in this version)"],
      clips: [
        {
          node: 0,
          name: "Bob",
          source: {
            type: "curves",
            duration: 2,
            looping: true,
            tracks: [
              { path: "Hull", property: "position", times: [0, 1], values: [0, 0, 0, 0, 1, 0] },
              { path: "Nope/Gone", property: "scale", times: [0], values: [1, 1, 1] },
            ],
          },
        },
        { node: 0, name: "Run", source: { type: "model", path: "Assets/hull.fbx", take: "Take 001" } },
      ],
    };
    const model = await assemblePrefab(scene, [mat("Red", { shader: "Toon", approximated: true }), mat("Default-Material")], ctx);

    const skiff = model.root.getObjectByName("Skiff")!;
    expect(skiff.position.x).toBe(-1);
    const hull = skiff.getObjectByName("Hull")!.children[0] as Mesh;
    expect(hull.isMesh).toBe(true);
    expect((hull.material as MeshStandardMaterial).name).toBe("Red");
    expect(hull.scale.x).toBeCloseTo(0.01);
    expect(hull.position.toArray().map((v) => +v.toFixed(4))).toEqual([-0.01, -0.02, -0.03]);
    const crate = skiff.getObjectByName("Crate")!;
    expect(crate.visible).toBe(false);
    expect((crate.children[0] as Mesh).geometry.type).toBe("BoxGeometry");
    const gone = skiff.getObjectByName("Gone")!.children[0] as LineSegments;
    expect(gone.userData.missing).toBe(true);
    expect(model.approximated).toEqual(["Toon"]);
    expect(model.missing).toEqual([
      "Lost › MeshFilter.m_Mesh → guid dead…007 (not in this version)",
      "Assets/gone.fbx: Assets/gone.fbx is not in that version",
    ]);

    const [bob, run] = model.clips;
    expect(bob.name).toBe("Bob");
    expect(bob.duration).toBe(2);
    expect(bob.tracks.map((t) => t.name)).toEqual([`${skiff.getObjectByName("Hull")!.uuid}.position`]);
    expect(run.name).toBe("Run");
    expect(run.tracks[0].name).toBe("Hull_Mesh.position");

    expect(model.clipRoots?.get(bob)).toBe(skiff);
    expect(model.clipRoots?.get(run)).toBe(skiff);
    const mixer = new AnimationMixer(model.root);
    mixer.clipAction(bob, model.clipRoots?.get(bob)).play();
    mixer.setTime(1);
    expect(skiff.getObjectByName("Hull")!.position.y).toBe(1);
  });

  it("moves FBX geometry so its pivot is the origin, as Unity's import does", () => {
    const round = (v: { toArray(): number[] }) => v.toArray().map((n) => +n.toFixed(3) + 0);
    const crystal = [-15.447, 33.963, -34.239];
    expect(round(pivotOffset({ rotationPivot: crystal, scalingPivot: crystal }))).toEqual([15.447, -33.963, 34.239]);
    expect(round(pivotOffset({ scale: [2, 2, 2], scalingPivot: [4, 0, 0], scalingOffset: [2, 0, 0], rotationPivot: [0, 6, 0] }))).toEqual([
      -1, -3, 0,
    ]);
    expect(round(pivotOffset(undefined))).toEqual([0, 0, 0]);
  });

  it("sizes Unity's primitives like Unity does", () => {
    const size = (name: string) => {
      const g = builtinGeometry(name);
      g.computeBoundingBox();
      return g.boundingBox!.max.toArray().map((v) => Math.round(v * 100) / 100);
    };
    expect(size("Cube")).toEqual([0.5, 0.5, 0.5]);
    expect(size("Cylinder")).toEqual([0.5, 1, 0.5]);
    expect(size("Capsule")).toEqual([0.5, 1, 0.5]);
    expect(size("Plane")).toEqual([5, 0, 5]);
  });
});
