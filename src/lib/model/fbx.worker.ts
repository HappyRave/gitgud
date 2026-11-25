import { readMeshBundle } from "./meshBundle";
import { type GeometryKind, readGeometry } from "./readGeometry";

/**
 * Fetches and parses model files off the main thread for prefabs and scenes: geometry
 * and clips only. A scene has hundreds; each response and parse would cost the window.
 * A "bundle" is several of a scene's own meshes in one answer.
 */
self.onmessage = async (e: MessageEvent<{ id: number; url: string; kind: GeometryKind | "bundle" }>) => {
  const { id, url, kind } = e.data;
  try {
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error((await res.text()) || `${res.status} ${res.statusText}`);
    }
    const bytes = await res.arrayBuffer();
    if (kind === "bundle") {
      const { meshes, transfer } = await readMeshBundle(bytes);
      self.postMessage({ id, meshes }, { transfer });
      return;
    }
    const { packed, transfer } = await readGeometry(bytes, kind);
    self.postMessage({ id, packed }, { transfer });
  } catch (err) {
    self.postMessage({ id, error: err instanceof Error ? err.message : String(err) });
  }
};
