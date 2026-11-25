import { type AnimationClip, Group, LoadingManager, type Material, type Mesh, type Object3D, TextureLoader } from "three";
import type { ModelInfo, ModelKind } from "$lib/api";
import { type LoadTexture, materialFrom } from "./materials";
import type { Step } from "./progress";

export interface LoadedModel {
  root: Object3D;
  clips: AnimationClip[];
  /** Shaders shown through an approximation, by name. */
  approximated: string[];
  /** References that couldn't be found ("Cargo › mesh → guid …"). */
  missing: string[];
  /** Gitignored files drawn from the disk copy, the same in every version. */
  fromDisk?: number;
}

export interface LoadSource {
  kind: ModelKind;
  /** The model file on the `gitgud` protocol. */
  url: string;
  /** URL of a file the model refers to by name. */
  refUrl: (ref: string) => string;
  /** URL of a repo image by path. */
  imageUrl: (path: string) => string;
  info: ModelInfo | null;
  loadTexture?: LoadTexture;
  /** The model file's name, for "Waiting for …". */
  name?: string;
  onProgress?: (step: Step) => void;
  /**
   * Only the meshes, their pivots and the clips (prefabs draw with Unity's materials):
   * an FBX then parses in a worker, without its own materials and textures.
   */
  geometryOnly?: boolean;
}

/** Thrown when the protocol answers with an error; `message` is the backend's reason. */
export class ModelLoadError extends Error {}

/** Fetches `url`, reporting bytes as they arrive (when the response says its size). */
export async function fetchBytes(url: string, name: string, report: (s: Step) => void = () => {}): Promise<Uint8Array> {
  report({ phase: "request", name });
  const res = await fetch(url);
  if (!res.ok) {
    throw new ModelLoadError((await res.text()) || `${res.status} ${res.statusText}`);
  }
  const total = Number(res.headers.get("content-length")) || 0;
  if (!res.body) {
    const all = new Uint8Array(await res.arrayBuffer());
    report({ phase: "download", loaded: all.length, total: all.length });
    return all;
  }
  const reader = res.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  report({ phase: "download", loaded, total });
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    chunks.push(value);
    loaded += value.length;
    report({ phase: "download", loaded, total });
  }
  const out = new Uint8Array(loaded);
  let at = 0;
  for (const c of chunks) {
    out.set(c, at);
    at += c.length;
  }
  return out;
}

/**
 * A three.js loading manager that sends the files a model names through `/ref`,
 * reports how many have loaded, and can be awaited until they all have.
 */
export class TrackedFiles {
  readonly manager = new LoadingManager();
  private busy = false;
  private waiting: (() => void)[] = [];

  constructor(refUrl: ((ref: string) => string) | null, report: (s: Step) => void = () => {}) {
    const m = this.manager;
    m.setURLModifier((u) => (refUrl && !/^(data:|blob:|https?:|gitgud:)/.test(u) ? refUrl(u) : u));
    m.addHandler(/\.(tga|psd|dds|exr|hdr|tiff?)$/i, new TextureLoader(m));
    m.onStart = () => (this.busy = true);
    m.onProgress = (_url, loaded, total) => report({ phase: "files", loaded, total });
    m.onLoad = () => {
      this.busy = false;
      for (const w of this.waiting.splice(0)) {
        w();
      }
    };
  }

  /** Loads a texture through this manager, so it is counted. */
  readonly loadTexture: LoadTexture = (url) => new TextureLoader(this.manager).load(url);

  idle(): Promise<void> {
    return this.busy ? new Promise((resolve) => this.waiting.push(resolve)) : Promise.resolve();
  }
}

function remap(root: Object3D, src: LoadSource, approximated: Set<string>, load: LoadTexture) {
  const info = src.info;
  if (!info || Object.keys(info.remaps).length === 0) {
    return;
  }
  const made = new Map<number, Material>();
  const swap = (m: Material): Material => {
    const ix = info.remaps[m.name];
    if (ix === undefined) {
      return m;
    }
    let out = made.get(ix);
    if (!out) {
      const desc = info.materials[ix];
      out = materialFrom(desc, src.imageUrl, load);
      if (desc.approximated) {
        approximated.add(desc.shader);
      }
      made.set(ix, out);
    }
    return out;
  };
  root.traverse((o) => {
    const mesh = o as Mesh;
    if (mesh.isMesh) {
      mesh.material = Array.isArray(mesh.material) ? mesh.material.map(swap) : swap(mesh.material);
    }
  });
}

/** Loads a standalone model file (FBX, OBJ, glTF, GLB) the way Unity imports it. */
export async function loadModel(src: LoadSource): Promise<LoadedModel> {
  const report = src.onProgress ?? (() => {});
  const approximated = new Set<string>();
  const bytes = await fetchBytes(src.url, src.name ?? "model", report);
  if (src.kind === "fbx" && src.geometryOnly) {
    report({ phase: "parse" });
    const { parseFbxGeometry } = await import("./fbxPool");
    const { unpackFbx } = await import("./packFbx");
    const model = unpackFbx(await parseFbxGeometry(bytes.buffer as ArrayBuffer), src.info?.scale ?? 1);
    report({ phase: "done" });
    return model;
  }
  const files = new TrackedFiles(src.refUrl, report);
  const manager = files.manager;
  const text = () => new TextDecoder().decode(bytes);
  report({ phase: "parse" });
  let root: Object3D;
  let clips: AnimationClip[] = [];
  switch (src.kind) {
    case "fbx": {
      const { FBXLoader } = await import("./vendor/FBXLoader.js");
      const group = new FBXLoader(manager).parse(bytes.buffer as ArrayBuffer, "");
      const fileUnits = (group.userData.unitScaleFactor as number | undefined) ?? 1;
      group.scale.multiplyScalar((fileUnits / 100) * (src.info?.scale ?? 1));
      root = group;
      clips = group.animations;
      break;
    }
    case "obj": {
      const { OBJLoader } = await import("three/examples/jsm/loaders/OBJLoader.js");
      root = new OBJLoader(manager).parse(text());
      break;
    }
    case "gltf":
    case "glb":
    case "asset": {
      const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
      const data = src.kind === "gltf" ? text() : (bytes.buffer as ArrayBuffer);
      const gltf = await new GLTFLoader(manager).parseAsync(data, "");
      root = gltf.scene;
      clips = gltf.animations;
      break;
    }
    default:
      throw new ModelLoadError(`${src.kind} files aren't loaded by loadModel`);
  }
  remap(root, src, approximated, src.loadTexture ?? files.loadTexture);
  await files.idle();
  report({ phase: "done" });
  const wrapper = new Group();
  wrapper.name = "(model)";
  wrapper.add(root);
  return { root: wrapper, clips, approximated: [...approximated], missing: [] };
}
