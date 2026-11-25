import {
  AnimationClip,
  Box3,
  BoxGeometry,
  BufferGeometry,
  CapsuleGeometry,
  CylinderGeometry,
  EdgesGeometry,
  Float32BufferAttribute,
  Group,
  LineBasicMaterial,
  LineSegments,
  type Material,
  Mesh,
  MeshStandardMaterial,
  type Object3D,
  PlaneGeometry,
  Points,
  PointsMaterial,
  QuaternionKeyframeTrack,
  Sphere,
  SphereGeometry,
  Vector3,
  VectorKeyframeTrack,
} from "three";
import type { MeshRef, ModelInfo, ModelMaterial, ModelScene, SceneClip } from "$lib/api";
import { clone as cloneSkinned } from "three/examples/jsm/utils/SkeletonUtils.js";
import { modelCache } from "./cache";
import { loadShared, materialKey } from "./shared";
import { inlineBatcher } from "./inlineBatch";
import { type LoadedModel, TrackedFiles } from "./load";
import type { Step } from "./progress";
import { type LoadTexture, MISSING_COLOR, materialFrom } from "./materials";
import { meshFileKind } from "./source";
import { unityFileId } from "./unityId";
import { framingCamera } from "./view";

export interface PrefabContext {
  modelUrl: (path: string) => string;
  /** Several Meshes stored inside the scene or prefab `path`, as one bundle (see `meshBundle.ts`). */
  meshesUrl: (path: string, ids: string[]) => string;
  refUrl: (near: string) => (ref: string) => string;
  imageUrl: (path: string) => string;
  info: (path: string) => Promise<ModelInfo>;
  loadTexture?: LoadTexture;
  /** Where to report the progress of the file `key`. */
  progress?: (key: string) => (step: Step) => void;
  /** A node's mesh arrived and replaced its placeholder, while the rest still loads. */
  onPiece?: (obj: Object3D) => void;
}

/**
 * A prefab that can show before its meshes load. `load()` starts loading them (once;
 * later calls return the same promise) and settles once all of them are in.
 */
export interface BuildingModel extends LoadedModel {
  load: () => Promise<LoadedModel>;
}

/** Faint dots marking where meshes will appear, one shared geometry and material for all. */
const placeholderGeometry = new BufferGeometry().setAttribute("position", new Float32BufferAttribute([0, 0, 0], 3));
const placeholderMaterial = new PointsMaterial({ color: "#9aa3ad", size: 4, sizeAttenuation: false, transparent: true, opacity: 0.55 });
placeholderGeometry.userData.shared = true;
placeholderMaterial.userData.shared = true;

function placeholder(): Points {
  const dot = new Points(placeholderGeometry, placeholderMaterial);
  dot.userData.placeholder = true;
  dot.raycast = () => {};
  return dot;
}

/** Node indices with a mesh, nearest to the default camera first, so what's in view fills in first. */
function nearestFirst(scene: ModelScene, objects: Object3D[]): number[] {
  const at = objects.map((o) => o.getWorldPosition(new Vector3()));
  const withMesh = scene.nodes.flatMap((n, i) => (n.mesh ? [i] : []));
  if (withMesh.length === 0) {
    return withMesh;
  }
  const camera = framingCamera(new Box3().setFromPoints(withMesh.map((i) => at[i])).getBoundingSphere(new Sphere()));
  const dist = at.map((p) => p.distanceToSquared(camera));
  return withMesh.sort((a, b) => dist[a] - dist[b]);
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

/** Turns a backend `ModelScene` into three.js objects, once every model file it uses has loaded. */
export function assemblePrefab(scene: ModelScene, materials: ModelMaterial[], ctx: PrefabContext): Promise<LoadedModel> {
  return buildPrefab(scene, materials, ctx).load();
}

/**
 * Turns a backend `ModelScene` into three.js objects at once, each mesh a
 * placeholder dot until `load()` brings its model file (nearest to the camera first).
 */
export function buildPrefab(scene: ModelScene, materials: ModelMaterial[], ctx: PrefabContext): BuildingModel {
  const models = new Map<string, Promise<LoadedModel>>();
  const made = new Map<number, Material>();
  const approximated = new Set<string>();
  const missing = [...scene.missing];
  const textures = new TrackedFiles(null, ctx.progress?.("textures"));
  const loadTexture = ctx.loadTexture ?? textures.loadTexture;

  const bundled = inlineBatcher(ctx.meshesUrl);
  // By content: the other side of a compare, and a later reopening, reuse the same mesh.
  const inline = (ref: { path: string; id: string; key: string }) => modelCache.model(`inline:${ref.key}`, () => bundled(ref.path, ref.id));

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
              geometryOnly: true,
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
        const src = firstMesh((await inline(ref)).root);
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
  scene.nodes.forEach((n, i) => (n.parent === null ? root : objects[n.parent]).add(objects[i]));
  root.updateMatrixWorld(true);
  const order = nearestFirst(scene, objects);
  const dots = new Map(order.map((i) => [i, placeholder()]));
  for (const [i, dot] of dots) {
    objects[i].add(dot);
  }

  let loading: Promise<LoadedModel> | null = null;
  const load = () => {
    if (loading) {
      return loading;
    }
    const pieces = order.map(async (i) => {
      const n = scene.nodes[i];
      const body = await drawn(n.mesh!, n.materials);
      objects[i].remove(dots.get(i)!);
      if (body) {
        body.name = n.name;
        objects[i].add(body);
        ctx.onPiece?.(body);
      }
    });
    // Clips from model files load alongside the meshes, not one after another once they're in.
    const read = scene.clips.map((c) => sceneClip(c, objects[c.node], model));
    loading = Promise.all(pieces).then(async () => {
      const clips: AnimationClip[] = [];
      const clipRoots = new Map<AnimationClip, Object3D>();
      for (const [k, clip] of (await Promise.all(read)).entries()) {
        const c = scene.clips[k];
        if (clip instanceof AnimationClip) {
          clips.push(clip);
          clipRoots.set(clip, objects[c.node]);
        } else {
          missing.push(`Clip ${c.name} couldn't be read: ${clip}`);
        }
      }
      await textures.idle();
      ctx.progress?.("textures")({ phase: "done" });
      Object.assign(building, { clips, clipRoots, approximated: [...approximated], missing });
      return building;
    });
    return loading;
  };

  const building: BuildingModel = { root, clips: [], approximated: [], missing: [], clipRoots: new Map(), load };
  return building;
}

/** The object at Unity animation path `path` ("Body/Arm") under `from`. */
function byPath(from: Object3D, path: string): Object3D | null {
  let cur: Object3D | undefined = from;
  for (const part of path.split("/").filter(Boolean)) {
    cur = cur?.children.find((c) => c.name === part);
  }
  return cur ?? null;
}

/** The clip, or why it couldn't be read. */
async function sceneClip(c: SceneClip, animator: Object3D, model: (path: string) => Promise<LoadedModel>): Promise<AnimationClip | string> {
  if (c.source.type === "model") {
    const { path, take } = c.source;
    try {
      const loaded = await model(path);
      const found = loaded.clips.find((x) => x.name === take) ?? (loaded.clips.length === 1 ? loaded.clips[0] : null);
      return found
        ? Object.assign(found.clone(), { name: c.name })
        : `${path} has no take "${take}" (it has ${loaded.clips.map((x) => `"${x.name}"`).join(", ") || "none"})`;
    } catch (e) {
      return `${path}: ${e instanceof Error ? e.message : String(e)}`;
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
