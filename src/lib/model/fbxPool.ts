import { ModelLoadError } from "./load";
import { type BundledMesh, readMeshBundle } from "./meshBundle";
import { fetchGeometry } from "./geometryCache";
import type { PackedModel } from "./packFbx";
import type { GeometryKind } from "./readGeometry";

type Kind = GeometryKind | "bundle";
type Answer = { packed?: PackedModel; meshes?: BundledMesh[] };

interface Job {
  id: number;
  url: string;
  kind: Kind;
  cache?: string;
  resolve: (a: Answer) => void;
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
      worker.onmessage = (e: MessageEvent<{ id: number; error?: string } & Answer>) => {
        slot.busy--;
        const job = running.get(e.data.id);
        running.delete(e.data.id);
        if (e.data.error === undefined) {
          job?.resolve(e.data);
        } else {
          job?.reject(new ModelLoadError(e.data.error));
        }
        dispatch();
        idleSoon();
      };
      return slot;
    });
  }
  return workers;
}

/**
 * How long the pool stays idle before workers send the meshes they parsed to be kept:
 * the protocol answers one request at a time, so keeping them mid-load would slow it.
 */
const IDLE_MS = 1000;
let idleTimer: ReturnType<typeof setTimeout> | undefined;

function idleSoon() {
  clearTimeout(idleTimer);
  if (waiting.length > 0 || running.size > 0) {
    return;
  }
  idleTimer = setTimeout(() => {
    if (waiting.length === 0 && running.size === 0) {
      for (const slot of pool()) {
        slot.worker.postMessage({ flush: true });
      }
    }
  }, IDLE_MS);
}

/** Hands waiting jobs, oldest first, to workers with room. */
function dispatch() {
  for (const slot of pool()) {
    while (slot.busy < PER_WORKER && waiting.length > 0) {
      const job = waiting.shift()!;
      slot.busy++;
      running.set(job.id, job);
      slot.worker.postMessage({ id: job.id, url: job.url, kind: job.kind, cache: job.cache });
    }
  }
}

/** Runs a job in the workers, in the order asked; without workers (tests), in place. */
async function run(url: string, kind: Kind, cache?: string): Promise<Answer> {
  if (typeof Worker === "undefined") {
    if (kind !== "bundle") {
      try {
        return { packed: (await fetchGeometry(url, kind, cache)).packed };
      } catch (e) {
        throw new ModelLoadError(e instanceof Error ? e.message : String(e));
      }
    }
    const res = await fetch(url);
    if (!res.ok) {
      throw new ModelLoadError((await res.text()) || `${res.status} ${res.statusText}`);
    }
    return { meshes: (await readMeshBundle(await res.arrayBuffer())).meshes };
  }
  return new Promise((resolve, reject) => {
    waiting.push({ id: next++, url, kind, cache, resolve, reject });
    dispatch();
  });
}

/**
 * Fetches and parses a model file's geometry and clips off the main thread; with `cache`,
 * through the meshes gitgud keeps by content.
 */
export async function loadGeometry(url: string, kind: GeometryKind, cache?: string): Promise<PackedModel> {
  const { packed } = await run(url, kind, cache);
  if (!packed) {
    throw new ModelLoadError(`no geometry came back for ${url}`);
  }
  return packed;
}

/** Fetches and parses several of a scene's own meshes in one request, off the main thread. */
export async function loadMeshBundle(url: string): Promise<BundledMesh[]> {
  const { meshes } = await run(url, "bundle");
  if (!meshes) {
    throw new ModelLoadError(`no meshes came back for ${url}`);
  }
  return meshes;
}
