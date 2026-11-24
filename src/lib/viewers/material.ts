import type { MaterialProperty, MaterialView } from "$lib/api";

export interface MaterialRow {
  key: string;
  label: string;
  before: MaterialProperty | null;
  after: MaterialProperty | null;
  changed: boolean;
}

export interface MaterialDiff {
  /** Shader, render queue and every property, in the newer file's order. */
  rows: MaterialRow[];
  changed: number;
  keywordsAdded: string[];
  keywordsRemoved: string[];
}

const fake = (key: string, label: string, value: string): MaterialProperty => ({ key, label, value, color: null, stale: false });

function head(v: MaterialView): MaterialProperty[] {
  const queue = v.renderQueue === null || v.renderQueue === -1 ? "from shader" : String(v.renderQueue);
  return [fake("shader", "Shader", v.material.shader), fake("queue", "Render Queue", queue)];
}

/** What changed between two versions of a material; a lone version changes nothing. */
export function diffMaterial(before: MaterialView | null, after: MaterialView | null): MaterialDiff {
  const b = new Map((before ? [...head(before), ...before.properties] : []).map((p) => [p.key, p]));
  const a = after ? [...head(after), ...after.properties] : [];
  const compare = before !== null && after !== null;
  const rows: MaterialRow[] = a.map((p) => ({
    key: p.key,
    label: p.label,
    before: b.get(p.key) ?? null,
    after: p,
    changed: compare && b.get(p.key)?.value !== p.value,
  }));
  const seen = new Set(a.map((p) => p.key));
  for (const p of b.values()) {
    if (!seen.has(p.key)) {
      rows.push({ key: p.key, label: p.label, before: p, after: null, changed: compare });
    }
  }
  const bk = new Set(before?.keywords ?? []);
  const ak = new Set(after?.keywords ?? []);
  return {
    rows,
    changed: rows.filter((r) => r.changed).length,
    keywordsAdded: compare ? [...ak].filter((k) => !bk.has(k)) : [],
    keywordsRemoved: compare ? [...bk].filter((k) => !ak.has(k)) : [],
  };
}

/** "Material · URP/Lit": the shader's name, "Universal Render Pipeline" shortened. */
export function materialPill(view: MaterialView): string {
  return `Material · ${view.material.shader.replace(/^Universal Render Pipeline\//, "URP/")}`;
}

export type MaterialShape = "sphere" | "cube" | "plane" | "mesh";
export const SHAPES: readonly MaterialShape[] = ["sphere", "cube", "plane", "mesh"];
export const SHAPE_LABEL: Record<MaterialShape, string> = { sphere: "Sphere", cube: "Cube", plane: "Plane", mesh: "Your mesh" };
