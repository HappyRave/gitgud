import type { MaterialView } from "$lib/api";

/** A MaterialView whose properties map keys to values; `#RRGGBB` values get a swatch. */
export function material(
  props: Record<string, string>,
  keywords: string[],
  over: { shader?: string; renderQueue?: number | null } = {},
): MaterialView {
  const shader = over.shader ?? "Universal Render Pipeline/Lit";
  return {
    material: {
      name: "Hull",
      path: "Assets/M/hull.mat",
      shader,
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
    },
    properties: Object.entries(props).map(([key, value]) => ({
      key,
      label: key.replace(/^_/, ""),
      value,
      color: value.startsWith("#") ? [1, 1, 1, 1] : null,
      stale: false,
    })),
    keywords,
    renderQueue: over.renderQueue ?? null,
  };
}
