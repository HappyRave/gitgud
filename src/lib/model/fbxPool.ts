import type { PackedModel } from "./packFbx";

type Pending = { resolve: (p: PackedModel) => void; reject: (e: Error) => void };

let workers: { worker: Worker; busy: number }[] | null = null;
const pending = new Map<number, Pending>();
let next = 0;

function pool() {
  if (!workers) {
    const count = Math.max(1, Math.min(4, (navigator.hardwareConcurrency || 2) - 1));
    workers = Array.from({ length: count }, () => {
      const worker = new Worker(new URL("./fbx.worker.ts", import.meta.url), { type: "module" });
      const slot = { worker, busy: 0 };
      worker.onmessage = (e: MessageEvent<{ id: number; packed?: PackedModel; error?: string }>) => {
        slot.busy--;
        const p = pending.get(e.data.id);
        pending.delete(e.data.id);
        if (e.data.packed) {
          p?.resolve(e.data.packed);
        } else {
          p?.reject(new Error(e.data.error ?? "the FBX worker failed"));
        }
      };
      return slot;
    });
  }
  return workers;
}

/**
 * Parses an FBX's geometry and clips on the least busy of a few workers, so a
 * scene's hundreds of meshes parse in parallel and off the main thread. Without
 * workers (tests), parses in place.
 */
export async function parseFbxGeometry(bytes: ArrayBuffer): Promise<PackedModel> {
  if (typeof Worker === "undefined") {
    const [{ FBXLoader }, { packFbx }] = await Promise.all([import("./vendor/FBXLoader.js"), import("./packFbx")]);
    const loader = new FBXLoader() as InstanceType<typeof FBXLoader> & { geometryOnly: boolean };
    loader.geometryOnly = true;
    const group = loader.parse(bytes, "");
    const units = (group.userData.unitScaleFactor as number | undefined) ?? 1;
    return packFbx(group, units / 100, group.animations).packed;
  }
  const slot = pool().reduce((a, b) => (b.busy < a.busy ? b : a));
  const id = next++;
  slot.busy++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    slot.worker.postMessage({ id, bytes }, [bytes]);
  });
}
