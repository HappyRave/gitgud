import { ModelLoadError } from "./load";
import type { PackedModel } from "./packFbx";
import { type GeometryKind, readGeometry } from "./readGeometry";

interface Job {
  id: number;
  url: string;
  kind: GeometryKind;
  resolve: (p: PackedModel) => void;
  reject: (e: Error) => void;
}

/**
 * Jobs one worker holds at a time: enough that it stays busy while the main thread is
 * slow to hand out more, few enough that a scene's thousand files don't all hit the
 * backend at once and come back out of the order asked (nearest the camera first).
 */
const PER_WORKER = 8;

let workers: { worker: Worker; busy: number }[] | null = null;
const waiting: Job[] = [];
const running = new Map<number, Job>();
let next = 0;

function pool() {
  if (!workers) {
    const count = Math.max(1, Math.min(6, (navigator.hardwareConcurrency || 2) - 2));
    workers = Array.from({ length: count }, () => {
      const worker = new Worker(new URL("./fbx.worker.ts", import.meta.url), { type: "module" });
      const slot = { worker, busy: 0 };
      worker.onmessage = (e: MessageEvent<{ id: number; packed?: PackedModel; error?: string }>) => {
        slot.busy--;
        const job = running.get(e.data.id);
        running.delete(e.data.id);
        if (e.data.packed) {
          job?.resolve(e.data.packed);
        } else {
          job?.reject(new ModelLoadError(e.data.error ?? "the model worker failed"));
        }
        dispatch();
      };
      return slot;
    });
  }
  return workers;
}

/** Hands waiting jobs, oldest first, to workers with room. */
function dispatch() {
  for (const slot of pool()) {
    while (slot.busy < PER_WORKER && waiting.length > 0) {
      const job = waiting.shift()!;
      slot.busy++;
      running.set(job.id, job);
      slot.worker.postMessage({ id: job.id, url: job.url, kind: job.kind });
    }
  }
}

/**
 * Fetches and parses a model file's geometry and clips in a few workers, off the main
 * thread, in the order asked. Without workers (tests), does it in place.
 */
export async function loadGeometry(url: string, kind: GeometryKind): Promise<PackedModel> {
  if (typeof Worker === "undefined") {
    const res = await fetch(url);
    if (!res.ok) {
      throw new ModelLoadError((await res.text()) || `${res.status} ${res.statusText}`);
    }
    return (await readGeometry(await res.arrayBuffer(), kind)).packed;
  }
  return new Promise((resolve, reject) => {
    waiting.push({ id: next++, url, kind, resolve, reject });
    dispatch();
  });
}
