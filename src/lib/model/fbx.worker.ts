import { fetchGeometry, keepGeometry } from "./geometryCache";
import { readMeshBundle } from "./meshBundle";
import type { GeometryKind } from "./readGeometry";

/** Parsed meshes waiting to be kept: sent once the pool is idle, so they don't slow a load. */
const pending: [string, ArrayBuffer][] = [];
let flushing = false;

async function flush() {
  if (flushing) {
    return;
  }
  flushing = true;
  for (let next = pending.shift(); next; next = pending.shift()) {
    await keepGeometry(...next);
  }
  flushing = false;
}

/**
 * Fetches and parses model files off the main thread for prefabs and scenes: geometry
 * and clips only. A scene has hundreds; each response and parse would cost the window.
 * A "bundle" is several of a scene's own meshes in one answer. `cache` is where parsed
 * meshes are kept by content (see `fetchGeometry`); `flush` sends the ones parsed so far.
 */
self.onmessage = async (
  e: MessageEvent<{ id: number; url: string; kind: GeometryKind | "bundle"; cache?: string } | { flush: true }>,
) => {
  if ("flush" in e.data) {
    void flush();
    return;
  }
  const { id, url, kind, cache } = e.data;
  try {
    if (kind === "bundle") {
      const res = await fetch(url);
      if (!res.ok) {
        throw new Error((await res.text()) || `${res.status} ${res.statusText}`);
      }
      const { meshes, transfer } = await readMeshBundle(await res.arrayBuffer());
      self.postMessage({ id, meshes }, { transfer });
      return;
    }
    const { packed, transfer } = await fetchGeometry(url, kind, cache, (c, b) => pending.push([c, b]));
    self.postMessage({ id, packed }, { transfer });
  } catch (err) {
    self.postMessage({ id, error: err instanceof Error ? err.message : String(err) });
  }
};
