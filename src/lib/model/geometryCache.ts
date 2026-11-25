import type { PackedModel } from "./packFbx";
import { readPacked, writePacked } from "./packedBytes";
import { type GeometryKind, readGeometry } from "./readGeometry";

/**
 * Part of every kept-meshes key: bump it when the parser (the patched FBXLoader) or what
 * `packFbx` keeps changes, so meshes parsed the old way are parsed again.
 */
export const PARSER_VERSION = 1;

/** Hands freshly parsed meshes, packed as one buffer, to be kept at `cache`. */
export type Keep = (cache: string, bytes: ArrayBuffer) => void;

/** Sends packed meshes to gitgud's disk cache; a failure is logged, never thrown. */
export async function keepGeometry(cache: string, bytes: ArrayBuffer): Promise<void> {
  try {
    const r = await fetch(cache, { method: "POST", body: bytes });
    if (!r.ok) {
      console.error(`gitgud: can't keep meshes at ${cache}: ${r.status} ${await r.text()}`);
    }
  } catch (e) {
    console.error(`gitgud: can't keep meshes at ${cache}: ${e}`);
  }
}

/**
 * Fetches and parses a model file's geometry. With `cache` (a `/geometry` URL), meshes
 * kept in gitgud's disk cache are used as they are, and a fresh parse goes to `keep`.
 */
export async function fetchGeometry(
  url: string,
  kind: GeometryKind,
  cache?: string,
  keep: Keep = (c, b) => void keepGeometry(c, b),
): Promise<{ packed: PackedModel; transfer: ArrayBuffer[] }> {
  if (cache) {
    const kept = await fetch(cache);
    if (kept.ok) {
      const buf = await kept.arrayBuffer();
      return { packed: readPacked(buf), transfer: [buf] };
    }
    if (kept.status !== 404) {
      console.error(`gitgud: kept meshes for ${url} can't be read: ${kept.status} ${await kept.text()}`);
    }
  }
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error((await res.text()) || `${res.status} ${res.statusText}`);
  }
  const parsed = await readGeometry(await res.arrayBuffer(), kind);
  if (cache) {
    keep(cache, writePacked(parsed.packed));
  }
  return parsed;
}
