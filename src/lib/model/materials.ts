import {
  CanvasTexture,
  Color,
  LessDepth,
  LessEqualDepth,
  type Material,
  MeshBasicMaterial,
  MeshMatcapMaterial,
  MeshStandardMaterial,
  RepeatWrapping,
  SRGBColorSpace,
  type Texture,
  TextureLoader,
} from "three";
import type { ModelMaterial } from "$lib/api";
import { shaderBase } from "$lib/prefs.svelte";
import { type Status, TINT } from "./compare";

export const MISSING_COLOR = "#ff00ff";

/** Loads a texture from a URL; swapped in tests. */
export type LoadTexture = (url: string) => Texture;

/** Fires "load" whenever a texture finishes loading, so stages can redraw. */
export const textureEvents = new EventTarget();

export const loadTexture: LoadTexture = (url) => new TextureLoader().load(url, () => textureEvents.dispatchEvent(new Event("load")));

/**
 * `desc` with the base map taken from the texture property picked for its shader,
 * when it is approximated and has that property.
 */
export function withBase(desc: ModelMaterial): ModelMaterial {
  const pick = desc.approximated ? shaderBase(desc.shader) : null;
  const slot = pick && pick !== desc.baseProperty ? desc.textures.find((t) => t.property === pick) : undefined;
  if (!slot) {
    return desc;
  }
  return { ...desc, baseMap: slot.path, baseMapKey: slot.key, tiling: slot.tiling, offset: slot.offset, baseProperty: slot.property };
}

/**
 * A three.js material for a Unity material. Smoothness becomes 1 - roughness;
 * missing materials are flat magenta, as in Unity.
 */
export function materialFrom(given: ModelMaterial, imageUrl: (path: string) => string, load: LoadTexture = loadTexture): Material {
  const desc = withBase(given);
  if (desc.missing) {
    return Object.assign(new MeshBasicMaterial({ color: MISSING_COLOR }), { name: desc.name });
  }
  const [r, g, b, a] = desc.color;
  const map = desc.baseMap ? load(imageUrl(desc.baseMap)) : null;
  if (map) {
    map.colorSpace = SRGBColorSpace;
    map.wrapS = map.wrapT = RepeatWrapping;
    map.repeat.set(desc.tiling[0], desc.tiling[1]);
    map.offset.set(desc.offset[0], desc.offset[1]);
  }
  const common = { color: new Color(r, g, b), map, transparent: a < 1, opacity: a };
  if (desc.unlit) {
    return Object.assign(new MeshBasicMaterial(common), { name: desc.name });
  }
  const normalMap = desc.normalMap ? load(imageUrl(desc.normalMap)) : null;
  if (normalMap) {
    normalMap.wrapS = normalMap.wrapT = RepeatWrapping;
    normalMap.repeat.copy(map?.repeat ?? normalMap.repeat);
  }
  return Object.assign(
    new MeshStandardMaterial({ ...common, normalMap, metalness: desc.metallic, roughness: 1 - desc.smoothness }),
    { name: desc.name },
  );
}

function matcapTexture(): Texture {
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d");
  if (g) {
    g.fillStyle = "#757b85";
    g.fillRect(0, 0, 128, 128);
    const gr = g.createRadialGradient(51, 38, 2, 64, 64, 64);
    gr.addColorStop(0, "#ffffff");
    gr.addColorStop(0.45, "#e2e5e9");
    gr.addColorStop(0.85, "#a6acb5");
    gr.addColorStop(1, "#757b85");
    g.fillStyle = gr;
    g.beginPath();
    g.arc(64, 64, 64, 0, Math.PI * 2);
    g.fill();
  }
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  return t;
}

/** The clay, ghost and wire materials one viewer shares across its meshes. */
export class ShadingKit {
  readonly ghost = new MeshBasicMaterial({
    color: "#ff8c42",
    transparent: true,
    opacity: 0.45,
    depthWrite: false,
    depthFunc: LessDepth,
  });
  readonly wire = new MeshBasicMaterial({
    color: "#ffffff",
    wireframe: true,
    transparent: true,
    opacity: 0.22,
    depthFunc: LessEqualDepth,
  });
  /** The older version's changed objects in the merged view. */
  readonly ghostWire = new MeshBasicMaterial({
    color: "#ff8c42",
    wireframe: true,
    transparent: true,
    opacity: 0.7,
    depthWrite: false,
  });
  private readonly matcap = matcapTexture();
  private readonly clays = new Map<Status, MeshMatcapMaterial>();
  private dim = false;

  clay(status: Status): MeshMatcapMaterial {
    let m = this.clays.get(status);
    if (!m) {
      m = new MeshMatcapMaterial({ matcap: this.matcap, color: TINT[status] });
      this.clays.set(status, m);
      if (status === "same") {
        this.dimSame(this.dim);
      }
    }
    return m;
  }

  /** Fades the "same" clay so only changes stand out. */
  dimSame(dim: boolean) {
    this.dim = dim;
    const m = this.clays.get("same");
    if (m) {
      m.transparent = dim;
      m.opacity = dim ? 0.12 : 1;
      m.depthWrite = !dim;
      m.needsUpdate = true;
    }
  }

  dispose() {
    this.ghost.dispose();
    this.ghostWire.dispose();
    this.wire.dispose();
    this.matcap.dispose();
    for (const m of this.clays.values()) {
      m.dispose();
    }
  }
}
