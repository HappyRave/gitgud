import { readGeometry, type GeometryKind } from "./readGeometry";

/**
 * Fetches and parses model files off the main thread for prefabs and scenes: geometry
 * and clips only. A scene has hundreds; each response and parse would cost the window.
 */
self.onmessage = async (e: MessageEvent<{ id: number; url: string; kind: GeometryKind }>) => {
  const { id, url, kind } = e.data;
  try {
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error((await res.text()) || `${res.status} ${res.statusText}`);
    }
    const { packed, transfer } = await readGeometry(await res.arrayBuffer(), kind);
    self.postMessage({ id, packed }, { transfer });
  } catch (err) {
    self.postMessage({ id, error: err instanceof Error ? err.message : String(err) });
  }
};
