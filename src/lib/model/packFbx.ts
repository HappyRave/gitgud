import { AnimationClip, BufferAttribute, BufferGeometry, Group, Mesh, type Object3D } from "three";
import type { LoadedModel } from "./load";

/** The attributes prefabs draw with; skinning and morphs aren't used there. */
const KEPT = ["position", "normal", "uv", "uv1", "color"] as const;

type Typed = Float32Array | Uint32Array | Uint16Array | Int16Array | Uint8Array | Int8Array;

export interface PackedMesh {
  name: string;
  originalName: string | null;
  transformData: unknown;
  attributes: { name: string; array: Typed; itemSize: number; normalized: boolean }[];
  index: Uint32Array | Uint16Array | null;
  groups: { start: number; count: number; materialIndex?: number }[];
}

/** A parsed FBX reduced to what prefabs and scenes use, in buffers a worker can hand over. */
export interface PackedModel {
  /** The file's unit scale (FBX centimetres to metres). */
  scale: number;
  /** Every mesh in traversal order, so hashed mesh IDs still match. */
  meshes: PackedMesh[];
  clips: ReturnType<typeof AnimationClip.toJSON>[];
}

/** Packs `root` (an FBXLoader result) and lists the buffers to transfer. */
export function packFbx(root: Object3D, scale: number, clips: AnimationClip[]): { packed: PackedModel; transfer: ArrayBuffer[] } {
  const meshes: PackedMesh[] = [];
  const transfer = new Set<ArrayBuffer>();
  root.traverse((o) => {
    const mesh = o as Mesh;
    if (!mesh.isMesh) {
      return;
    }
    const g = mesh.geometry;
    const attributes = KEPT.flatMap((name) => {
      const a = g.getAttribute(name) as BufferAttribute | undefined;
      if (!a) {
        return [];
      }
      transfer.add(a.array.buffer as ArrayBuffer);
      return [{ name, array: a.array as Typed, itemSize: a.itemSize, normalized: a.normalized }];
    });
    const index = (g.index?.array as Uint32Array | Uint16Array | undefined) ?? null;
    if (index) {
      transfer.add(index.buffer as ArrayBuffer);
    }
    meshes.push({
      name: mesh.name,
      originalName: (mesh.userData.originalName as string | undefined) ?? null,
      transformData: mesh.userData.transformData ?? null,
      attributes,
      index,
      groups: g.groups.map((x) => ({ ...x })),
    });
  });
  return { packed: { scale, meshes, clips: clips.map((c) => AnimationClip.toJSON(c)) }, transfer: [...transfer] };
}

/** Rebuilds a packed model the way `loadModel` returns one: a wrapper, the scaled file, its meshes. */
export function unpackFbx(p: PackedModel, importScale: number): LoadedModel {
  const file = new Group();
  file.scale.setScalar(p.scale * importScale);
  for (const m of p.meshes) {
    const g = new BufferGeometry();
    for (const a of m.attributes) {
      g.setAttribute(a.name, new BufferAttribute(a.array, a.itemSize, a.normalized));
    }
    if (m.index) {
      g.setIndex(new BufferAttribute(m.index, 1));
    }
    for (const x of m.groups) {
      g.addGroup(x.start, x.count, x.materialIndex);
    }
    const mesh = new Mesh(g);
    mesh.name = m.name;
    if (m.originalName !== null) {
      mesh.userData.originalName = m.originalName;
    }
    if (m.transformData !== null) {
      mesh.userData.transformData = m.transformData;
    }
    file.add(mesh);
  }
  const wrapper = new Group();
  wrapper.name = "(model)";
  wrapper.add(file);
  return { root: wrapper, clips: p.clips.map((c) => AnimationClip.parse(c)), approximated: [], missing: [] };
}
