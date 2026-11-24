import {
  AnimationClip,
  BoxGeometry,
  type BufferGeometry,
  CapsuleGeometry,
  CylinderGeometry,
  EdgesGeometry,
  Group,
  LineBasicMaterial,
  LineSegments,
  type Material,
  Mesh,
  MeshStandardMaterial,
  type Object3D,
  PlaneGeometry,
  QuaternionKeyframeTrack,
  SphereGeometry,
  Vector3,
  VectorKeyframeTrack,
} from "three";
import type { MeshRef, ModelInfo, ModelMaterial, ModelScene, SceneClip } from "$lib/api";
import { clone as cloneSkinned } from "three/examples/jsm/utils/SkeletonUtils.js";
import { modelCache } from "./cache";
import { loadShared, materialKey } from "./shared";
import { type LoadedModel, loadModel, TrackedFiles } from "./load";
import type { Step } from "./progress";
import { type LoadTexture, MISSING_COLOR, materialFrom } from "./materials";
import { meshFileKind } from "./source";
import { unityFileId } from "./unityId";

export interface PrefabContext {
  modelUrl: (path: string) => string;
  /** A Mesh stored inside the scene or prefab `path`, as GLB. */
  meshUrl: (path: string, id: string) => string;
  refUrl: (near: string) => (ref: string) => string;
  imageUrl: (path: string) => string;
  info: (path: string) => Promise<ModelInfo>;
  loadTexture?: LoadTexture;
  /** Where to report the progress of the file `key`. */
  progress?: (key: string) => (step: Step) => void;
}

/** Unity's built-in primitive meshes, sized as Unity makes them. */
export function builtinGeometry(name: string): BufferGeometry {
  switch (name) {
    case "Sphere":
      return new SphereGeometry(0.5, 32, 16);
    case "Capsule":
      return new CapsuleGeometry(0.5, 1, 8, 24);
    case "Cylinder":
      return new CylinderGeometry(0.5, 0.5, 2, 24);
    case "Plane":
      return new PlaneGeometry(10, 10).rotateX(-Math.PI / 2);
    case "Quad":
      return new PlaneGeometry(1, 1).rotateY(Math.PI);
    default:
      return new BoxGeometry(1, 1, 1);
  }
}

function missingMesh(): Object3D {
  const box = new LineSegments(new EdgesGeometry(new BoxGeometry(1, 1, 1)), new LineBasicMaterial({ color: MISSING_COLOR }));
  box.userData.missing = true;
  return box;
}

interface FbxTransform {
  scale?: number[];
  rotationPivot?: number[];
  scalingPivot?: number[];
  scalingOffset?: number[];
}

/**
 * Unity bakes an FBX node's pivots into its mesh, so the mesh's origin is the
 * rotation pivot; three.js keeps them in the node's transform instead. With
 * FBX's T·Roff·Rp·R·Rp⁻¹·Soff·Sp·S·Sp⁻¹, the part Unity's transform doesn't
 * carry is a translation of the geometry by (Soff + Sp − Rp) / S − Sp.
 */
export function pivotOffset(t: FbxTransform | undefined): Vector3 {
  const at = (v: number[] | undefined, i: number) => v?.[i] ?? 0;
  const out = new Vector3();
  for (let i = 0; i < 3; i++) {
    const s = at(t?.scale, i) || 1;
    out.setComponent(i, (at(t?.scalingOffset, i) + at(t?.scalingPivot, i) - at(t?.rotationPivot, i)) / s - at(t?.scalingPivot, i));
  }
  return out;
}

/** The mesh a Unity reference means: by name from the importer table, else by hashed fileID. */
function firstMesh(root: Object3D): Mesh | null {
  let found: Mesh | null = null;
  root.traverse((o) => {
    found ??= (o as Mesh).isMesh ? (o as Mesh) : null;
  });
  return found;
}

function findMesh(root: Object3D, name: string | null, id: string | null): Mesh | null {
  let found: Mesh | null = null;
  const seen = new Map<string, number>();
  root.traverse((o) => {
    const mesh = o as Mesh;
    if (found || !mesh.isMesh) {
      return;
    }
    const raw = (o.userData.originalName as string | undefined) ?? o.name;
    const n = seen.get(raw) ?? 0;
    seen.set(raw, n + 1);
    if (name !== null ? raw === name || o.name === name : id !== null && unityFileId("Mesh", raw, n) === id) {
      found = mesh;
    }
  });
  return found;
}

/** Turns a backend `ModelScene` into three.js objects, loading the model files it uses. */
export async function assemblePrefab(scene: ModelScene, materials: ModelMaterial[], ctx: PrefabContext): Promise<LoadedModel> {
  const models = new Map<string, Promise<LoadedModel>>();
  const made = new Map<number, Material>();
  const approximated = new Set<string>();
  const missing = [...scene.missing];
  const textures = new TrackedFiles(null, ctx.progress?.("textures"));
  const loadTexture = ctx.loadTexture ?? textures.loadTexture;

  const inline = (path: string, id: string) => {
    const key = `${path}#${id}`;
    let p = models.get(key);
    if (!p) {
      p = loadModel({ kind: "asset", url: ctx.meshUrl(path, id), refUrl: () => "", imageUrl: ctx.imageUrl, info: null, loadTexture });
      models.set(key, p);
    }
    return p;
  };

  const model = (path: string) => {
    let p = models.get(path);
    if (!p) {
      const kind = meshFileKind(path);
      const report = ctx.progress?.(path);
      report?.({ phase: "request", name: path.slice(path.lastIndexOf("/") + 1) });
      p = kind
        ? ctx.info(path).then((info) =>
            loadShared({
              kind,
              url: ctx.modelUrl(path),
              refUrl: ctx.refUrl(path),
              imageUrl: ctx.imageUrl,
              info,
              loadTexture,
              name: path.slice(path.lastIndexOf("/") + 1),
              onProgress: report,
            }),
          )
        : Promise.reject(new Error(`${path} is not a model file the viewer reads`));
      p.catch(() => report?.({ phase: "done" }));
      models.set(path, p);
    }
    return p;
  };

  const material = (ix: number): Material => {
    let m = made.get(ix);
    if (!m) {
      const desc = materials[ix];
      const key = materialKey(desc);
      const make = () => materialFrom(desc, ctx.imageUrl, loadTexture);
      m = key ? modelCache.material(key, make) : make();
      if (desc.approximated) {
        approximated.add(desc.shader);
      }
      made.set(ix, m);
    }
    return m;
  };

  const fallback = new MeshStandardMaterial({ color: "#bbbbbb", name: "(no material)" });
  const drawn = async (ref: MeshRef, mats: number[]): Promise<Object3D | null> => {
    const pick = (count: number) => {
      const list = mats.map(material);
      return count > 1 && list.length > 1 ? list : (list[0] ?? fallback);
    };
    if (ref.type === "missing") {
      return missingMesh();
    }
    if (ref.type === "builtin") {
      return new Mesh(builtinGeometry(ref.name), pick(1));
    }
    if (ref.type === "inline") {
      try {
        const src = firstMesh((await inline(ref.path, ref.id)).root);
        if (!src) {
          throw new Error("no mesh in it");
        }
        return new Mesh(src.geometry, pick(src.geometry.groups.length));
      } catch (e) {
        missing.push(`${ref.path} › Mesh &${ref.id}: ${e instanceof Error ? e.message : String(e)}`);
        return missingMesh();
      }
    }
    let loaded: LoadedModel;
    try {
      loaded = await model(ref.path);
    } catch (e) {
      missing.push(`${ref.path}: ${e instanceof Error ? e.message : String(e)}`);
      return missingMesh();
    }
    if (ref.name === null && ref.id === null) {
      return cloneSkinned(loaded.root);
    }
    // A Mesh .asset holds one mesh, referenced by its main fileID (4300000), not a hashed one.
    const src = meshFileKind(ref.path) === "asset" ? firstMesh(loaded.root) : findMesh(loaded.root, ref.name, ref.id);
    if (!src) {
      missing.push(`${ref.path} has no mesh ${ref.name ?? `with fileID ${ref.id}`}`);
      return missingMesh();
    }
    const fileScale = loaded.root.children[0]?.scale.x ?? 1;
    const mesh = new Mesh(src.geometry, pick(src.geometry.groups.length));
    mesh.scale.setScalar(fileScale);
    mesh.position.copy(pivotOffset(src.userData.transformData)).multiplyScalar(fileScale);
    return mesh;
  };

  const objects: Group[] = scene.nodes.map((n) => {
    const g = new Group();
    g.name = n.name;
    g.position.fromArray(n.position);
    g.quaternion.fromArray(n.rotation);
    g.scale.fromArray(n.scale);
    g.visible = n.active;
    return g;
  });
  const root = new Group();
  root.name = "(prefab)";
  await Promise.all(
    scene.nodes.map(async (n, i) => {
      (n.parent === null ? root : objects[n.parent]).add(objects[i]);
      if (n.mesh) {
        const body = await drawn(n.mesh, n.materials);
        if (body) {
          body.name = n.name;
          objects[i].add(body);
        }
      }
    }),
  );
  const clips: AnimationClip[] = [];
  for (const c of scene.clips) {
    const clip = await sceneClip(c, objects[c.node], model);
    if (clip) {
      clips.push(clip);
    } else {
      missing.push(`Clip ${c.name} couldn't be read`);
    }
  }
  await textures.idle();
  ctx.progress?.("textures")({ phase: "done" });
  return { root, clips, approximated: [...approximated], missing };
}

/** The object at Unity animation path `path` ("Body/Arm") under `from`. */
function byPath(from: Object3D, path: string): Object3D | null {
  let cur: Object3D | undefined = from;
  for (const part of path.split("/").filter(Boolean)) {
    cur = cur?.children.find((c) => c.name === part);
  }
  return cur ?? null;
}

async function sceneClip(c: SceneClip, animator: Object3D, model: (path: string) => Promise<LoadedModel>): Promise<AnimationClip | null> {
  if (c.source.type === "model") {
    const take = c.source.take;
    try {
      const loaded = await model(c.source.path);
      const found = loaded.clips.find((x) => x.name === take) ?? (loaded.clips.length === 1 ? loaded.clips[0] : null);
      return found ? Object.assign(found.clone(), { name: c.name }) : null;
    } catch {
      return null;
    }
  }
  const tracks = c.source.tracks.flatMap((t) => {
    const target = byPath(animator, t.path);
    if (!target) {
      return [];
    }
    const name = `${target.uuid}.${t.property}`;
    return [t.property === "quaternion" ? new QuaternionKeyframeTrack(name, t.times, t.values) : new VectorKeyframeTrack(name, t.times, t.values)];
  });
  return new AnimationClip(c.name, c.source.duration, tracks);
}
