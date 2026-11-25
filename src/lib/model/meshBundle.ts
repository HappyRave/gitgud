import type { PackedModel } from "./packFbx";
import { readGeometry } from "./readGeometry";

/** One mesh of a bundle: its geometry, or why the backend couldn't read it. */
export type BundledMesh = { id: string; packed: PackedModel } | { id: string; error: string };

/**
 * Splits the backend's answer to `/model?…&meshes=a,b` (per mesh: a status byte, then
 * the fileID and the GLB or the reason, each a little-endian u32 length and its bytes).
 */
export function splitBundle(bytes: ArrayBuffer): { ok: boolean; id: string; payload: ArrayBuffer }[] {
  const view = new DataView(bytes);
  const text = new TextDecoder();
  const out: { ok: boolean; id: string; payload: ArrayBuffer }[] = [];
  let at = 0;
  const field = () => {
    const len = view.getUint32(at, true);
    at += 4;
    const part = bytes.slice(at, at + len);
    at += len;
    return part;
  };
  while (at < bytes.byteLength) {
    const ok = view.getUint8(at) === 1;
    at += 1;
    const id = text.decode(field());
    out.push({ ok, id, payload: field() });
  }
  return out;
}

/** Reads every mesh of a bundle, with the buffers to hand back from a worker. */
export async function readMeshBundle(bytes: ArrayBuffer): Promise<{ meshes: BundledMesh[]; transfer: ArrayBuffer[] }> {
  const meshes: BundledMesh[] = [];
  const transfer: ArrayBuffer[] = [];
  for (const { ok, id, payload } of splitBundle(bytes)) {
    if (!ok) {
      meshes.push({ id, error: new TextDecoder().decode(payload) });
      continue;
    }
    try {
      const read = await readGeometry(payload, "glb");
      meshes.push({ id, packed: read.packed });
      transfer.push(...read.transfer);
    } catch (e) {
      meshes.push({ id, error: e instanceof Error ? e.message : String(e) });
    }
  }
  return { meshes, transfer };
}
