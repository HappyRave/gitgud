import type { BufferGeometry, Material, Mesh, Object3D, Texture } from "three";
import type { LoadedModel } from "./load";

/** Roughly how much loaded model data the viewer keeps before dropping the least recently used. */
export const MODEL_CACHE_BUDGET = 768 * 1024 * 1024;

interface Entry<T> {
  value: T;
  used: number;
}

export interface ModelCacheStats {
  models: number;
  materials: number;
  /** Estimated geometry and texture memory, in bytes. */
  bytes: number;
}

const isTexture = (v: unknown): v is Texture => !!v && typeof v === "object" && (v as Texture).isTexture === true;

function texturesOf(m: Material): Texture[] {
  return Object.values(m).filter(isTexture);
}

function materialsOf(o: Object3D): Material[] {
  const mesh = o as Mesh;
  return mesh.isMesh ? [mesh.material].flat() : [];
}

/** Marks what a cached model or material holds, so a viewer releasing its scene leaves it alone. */
function markShared(geometries: BufferGeometry[], materials: Material[]) {
  for (const g of geometries) {
    g.userData.shared = true;
  }
  for (const m of materials) {
    m.userData.shared = true;
    for (const t of texturesOf(m)) {
      t.userData.shared = true;
    }
  }
}

function partsOf(root: Object3D) {
  const geometries = new Set<BufferGeometry>();
  const materials = new Set<Material>();
  root.traverse((o) => {
    const mesh = o as Mesh;
    if (mesh.isMesh) {
      geometries.add(mesh.geometry);
      for (const m of materialsOf(o)) {
        materials.add(m);
      }
    }
  });
  return { geometries: [...geometries], materials: [...materials] };
}

function geometryBytes(g: BufferGeometry): number {
  let n = g.index?.array.byteLength ?? 0;
  for (const a of Object.values(g.attributes)) {
    n += (a as { array?: ArrayLike<number> & { byteLength?: number } }).array?.byteLength ?? 0;
  }
  return n;
}

function textureBytes(t: Texture): number {
  const img = t.image as { width?: number; height?: number } | null;
  return img?.width && img.height ? img.width * img.height * 4 : 0;
}

/** Bytes of `textures` that `seen` doesn't hold yet. */
function unseenBytes(textures: Texture[], seen: Set<object>): number {
  let n = 0;
  for (const t of textures) {
    if (!seen.has(t.source)) {
      seen.add(t.source);
      n += textureBytes(t);
    }
  }
  return n;
}

function disposeAll(geometries: BufferGeometry[], materials: Material[]) {
  for (const g of geometries) {
    g.dispose();
  }
  for (const m of materials) {
    for (const t of texturesOf(m)) {
      t.dispose();
    }
    m.dispose();
  }
}

/**
 * Loaded model files and Unity materials, by the backend's content key, shared by
 * every viewer: both sides of a compare and every reopening reuse them. Callers
 * clone what they put in a scene; the GPU data stays shared.
 */
export class ModelCache {
  budget = MODEL_CACHE_BUDGET;
  private models = new Map<string, Entry<Promise<LoadedModel>>>();
  /** Each loaded model's parts and geometry size, read once: a scene adds hundreds, each one trims. */
  private loaded = new Map<string, { geometries: BufferGeometry[]; materials: Material[]; textures: Texture[]; geometry: number }>();
  private materials = new Map<string, Entry<Material> & { textures: Texture[] }>();
  private clock = 0;

  /** The model `key` names, loaded once with `load`. A failed load isn't kept. */
  model(key: string, load: () => Promise<LoadedModel>): Promise<LoadedModel> {
    const hit = this.models.get(key);
    if (hit) {
      hit.used = ++this.clock;
      return hit.value;
    }
    const entry: Entry<Promise<LoadedModel>> = { value: load(), used: ++this.clock };
    this.models.set(key, entry);
    entry.value.then(
      (m) => {
        if (this.models.get(key) !== entry) {
          return;
        }
        const { geometries, materials } = partsOf(m.root);
        markShared(geometries, materials);
        const geometry = geometries.reduce((n, g) => n + geometryBytes(g), 0);
        this.loaded.set(key, { geometries, materials, textures: materials.flatMap(texturesOf), geometry });
        this.trim();
      },
      () => {
        if (this.models.get(key) === entry) {
          this.models.delete(key);
        }
      },
    );
    return entry.value;
  }

  /** The material `key` names, made once with `make`. */
  material(key: string, make: () => Material): Material {
    const hit = this.materials.get(key);
    if (hit) {
      hit.used = ++this.clock;
      return hit.value;
    }
    const m = make();
    markShared([], [m]);
    this.materials.set(key, { value: m, used: ++this.clock, textures: texturesOf(m) });
    this.trim();
    return m;
  }

  stats(): ModelCacheStats {
    const seen = new Set<object>();
    let bytes = 0;
    for (const m of this.loaded.values()) {
      bytes += m.geometry + unseenBytes(m.textures, seen);
    }
    for (const e of this.materials.values()) {
      bytes += unseenBytes(e.textures, seen);
    }
    return { models: this.models.size, materials: this.materials.size, bytes };
  }

  /** Frees everything; a viewer still showing a model re-uploads what it draws. */
  clear() {
    for (const m of this.loaded.values()) {
      disposeAll(m.geometries, m.materials);
    }
    disposeAll([], [...this.materials.values()].map((e) => e.value));
    this.models.clear();
    this.loaded.clear();
    this.materials.clear();
  }

  /** Drops the least recently used entries while over budget, keeping at least the newest. */
  private trim() {
    while (this.models.size + this.materials.size > 1 && this.stats().bytes > this.budget) {
      const oldest = (map: Map<string, Entry<unknown>>) =>
        [...map.entries()].reduce<[string, number] | null>((o, [k, e]) => (!o || e.used < o[1] ? [k, e.used] : o), null);
      const model = oldest(this.models);
      const material = oldest(this.materials);
      if (model && (!material || model[1] < material[1])) {
        const loaded = this.loaded.get(model[0]);
        if (loaded) {
          disposeAll(loaded.geometries, loaded.materials);
        }
        this.loaded.delete(model[0]);
        this.models.delete(model[0]);
      } else if (material) {
        disposeAll([], [this.materials.get(material[0])!.value]);
        this.materials.delete(material[0]);
      }
    }
  }
}

/** The one cache every model viewer in the app shares. */
export const modelCache = new ModelCache();
