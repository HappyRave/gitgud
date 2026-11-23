import type { Material, Mesh, Object3D } from "three";

export type ChangeKind = "added" | "removed" | "modified";
export type Status = ChangeKind | "same";

/** Clay tint per status in the Changes shading (R6, from D). */
export const TINT: Record<Status, string> = {
  same: "#ffffff",
  added: "#46e0a6",
  removed: "#ff6b5a",
  modified: "#ffc533",
};

export interface Change {
  kind: ChangeKind;
  name: string;
  obj: Object3D;
  /** Why it counts as changed, or how many children came with it. */
  note: string;
}

export interface Comparison {
  status: Map<Object3D, Status>;
  changes: Change[];
}

const r3 = (n: number) => Math.round(n * 1000) / 1000;

/** The original material(s) of a mesh, even while a clay material is shown. */
export function litMaterials(o: Object3D): Material[] {
  const m = (o.userData.lit ?? (o as Mesh).material) as Material | Material[] | undefined;
  return m ? (Array.isArray(m) ? m : [m]) : [];
}

function geometryKey(o: Object3D): string {
  const g = (o as Mesh).isMesh ? (o as Mesh).geometry : null;
  if (!g) {
    return "";
  }
  if (!g.boundingBox) {
    g.computeBoundingBox();
  }
  const b = g.boundingBox!;
  const count = g.attributes.position?.count ?? 0;
  return [count, g.index?.count ?? 0, r3(b.min.x), r3(b.min.y), r3(b.min.z), r3(b.max.x), r3(b.max.y), r3(b.max.z)].join(",");
}

const materialKey = (o: Object3D) => litMaterials(o).map((m) => m.name).join("+");

function transformKey(o: Object3D): string {
  const { position: p, quaternion: q, scale: s } = o;
  return [p.x, p.y, p.z, q.x, q.y, q.z, q.w, s.x, s.y, s.z].map(r3).join(",");
}

const signature = (o: Object3D) => `${geometryKey(o)}|${materialKey(o)}|${transformKey(o)}`;

/** Every object under `root` by its name path; repeated sibling names get `#2`, `#3`. */
export function namePaths(root: Object3D): Map<string, Object3D> {
  const out = new Map<string, Object3D>();
  const walk = (o: Object3D, prefix: string) => {
    const seen = new Map<string, number>();
    for (const child of o.children) {
      const base = child.name || child.type;
      const n = (seen.get(base) ?? 0) + 1;
      seen.set(base, n);
      const key = `${prefix}${n > 1 ? `${base}#${n}` : base}`;
      out.set(key, child);
      walk(child, `${key}/`);
    }
  };
  walk(root, "");
  return out;
}

function why(before: Object3D, after: Object3D): string {
  const out: string[] = [];
  const mb = materialKey(before);
  const ma = materialKey(after);
  if (mb !== ma) {
    out.push(`${mb || "no material"} → ${ma || "no material"}`);
  }
  if (geometryKey(before) !== geometryKey(after)) {
    out.push("mesh changed");
  }
  const d = before.position.distanceTo(after.position);
  if (d > 1e-3) {
    out.push(`moved ${d.toFixed(2)} m`);
  }
  if (before.quaternion.angleTo(after.quaternion) > 1e-3) {
    out.push("rotated");
  }
  if (before.scale.distanceTo(after.scale) > 1e-3) {
    out.push("scaled");
  }
  return out.join(" · ");
}

function childNote(o: Object3D): string {
  let n = -1;
  o.traverse(() => n++);
  return n ? `with ${n} ${n === 1 ? "child" : "children"}` : "";
}

/**
 * Matches objects across two versions by name path. Each object of either side
 * gets a status; `changes` lists the topmost added / removed objects and every
 * modified one, in tree order.
 */
export function compare(before: Object3D, after: Object3D): Comparison {
  const a = namePaths(before);
  const b = namePaths(after);
  const status = new Map<Object3D, Status>();
  const changes: Change[] = [];
  const parentKey = (k: string) => k.slice(0, Math.max(0, k.lastIndexOf("/")));
  for (const [k, o] of b) {
    const p = a.get(k);
    if (!p) {
      status.set(o, "added");
      const pk = parentKey(k);
      if (!pk || a.has(pk)) {
        changes.push({ kind: "added", name: o.name || o.type, obj: o, note: childNote(o) });
      }
    } else if (signature(p) !== signature(o)) {
      status.set(o, "modified");
      changes.push({ kind: "modified", name: o.name || o.type, obj: o, note: why(p, o) });
    } else {
      status.set(o, "same");
    }
  }
  for (const [k, o] of a) {
    const q = b.get(k);
    if (!q) {
      status.set(o, "removed");
      const pk = parentKey(k);
      if (!pk || b.has(pk)) {
        changes.push({ kind: "removed", name: o.name || o.type, obj: o, note: childNote(o) });
      }
    } else {
      status.set(o, status.get(q) === "modified" ? "modified" : "same");
    }
  }
  return { status, changes };
}

export interface ModelStats {
  objects: number;
  meshes: number;
  vertices: number;
  triangles: number;
  materials: number;
  textures: number;
}

export function stats(root: Object3D): ModelStats {
  let objects = 0;
  let meshes = 0;
  let vertices = 0;
  let triangles = 0;
  const mats = new Set<string>();
  const tex = new Set<string>();
  root.traverse((o) => {
    if (o === root) {
      return;
    }
    objects++;
    const mesh = o as Mesh;
    if (!mesh.isMesh) {
      return;
    }
    meshes++;
    const g = mesh.geometry;
    const count = g.attributes.position?.count ?? 0;
    vertices += count;
    triangles += Math.round((g.index ? g.index.count : count) / 3);
    for (const m of litMaterials(o)) {
      mats.add(m.uuid);
      const map = (m as Material & { map?: { uuid: string } | null }).map;
      if (map) {
        tex.add(map.uuid);
      }
    }
  });
  return { objects, meshes, vertices, triangles, materials: mats.size, textures: tex.size };
}
