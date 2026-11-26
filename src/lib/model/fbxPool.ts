import { ModelLoadError } from "./load";
import { type BundledMesh, readMeshBundle } from "./meshBundle";
import { fetchGeometry } from "./geometryCache";
import type { PackedModel } from "./packFbx";
import type { GeometryKind } from "./readGeometry";

type Kind = GeometryKind | "bundle";
type Answer = { packed?: PackedModel; meshes?: BundledMesh[] };

interface Job {
  id: number;
  /** The load it was asked for; see `preferNewest`. */
  epoch: number;
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

interface Slot {
  worker: Worker;
  busy: number;
}

let workers: Slot[] | null = null;
const waiting: Job[] = [];
/** Jobs handed out, with the worker holding each. */
const running = new Map<number, { job: Job; slot: Slot }>();
let next = 0;
let epoch = 0;

/**
 * A new model is being opened: its jobs go ahead of those still waiting for earlier
 * ones (a scene left half-loaded would otherwise hold the workers for seconds). The
 * earlier jobs still run after, filling the cache, since another view may share them.
 */
export function preferNewest() {
  epoch++;
}

function spawn(): Slot {
  const worker = new Worker(new URL("./fbx.worker.ts", import.meta.url), { type: "module" });
  const slot: Slot = { worker, busy: 0 };
  worker.onmessage = (e: MessageEvent<{ id: number; error?: string } & Answer>) => {
    slot.busy--;
    const held = running.get(e.data.id);
    running.delete(e.data.id);
    if (e.data.error === undefined) {
      held?.job.resolve(e.data);
    } else {
      held?.job.reject(new ModelLoadError(e.data.error));
    }
    dispatch();
    idleSoon();
  };
  worker.onerror = (e) => fail(slot, e.message || "a model worker stopped");
  worker.onmessageerror = () => fail(slot, "a model worker sent what can't be read");
  return slot;
}

/** A worker died (out of memory on a huge file, a module that didn't load): its jobs fail, a new one takes its place. */
function fail(slot: Slot, why: string) {
  console.error(`Model worker failed: ${why}`);
  for (const [id, held] of running) {
    if (held.slot === slot) {
      running.delete(id);
      held.job.reject(new ModelLoadError(`${held.job.url}: ${why}`));
    }
  }
  slot.worker.terminate();
  const all = pool();
  all[all.indexOf(slot)] = spawn();
  dispatch();
  idleSoon();
}

function pool() {
  if (!workers) {
    const count = Math.max(1, Math.min(6, (navigator.hardwareConcurrency || 2) - 2));
    workers = Array.from({ length: count }, spawn);
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

/** The next job to run: the newest load's, oldest first within it (jobs arrive in epoch order). */
function take(): Job {
  const newest = waiting[waiting.length - 1].epoch;
  let at = waiting.length - 1;
  while (at > 0 && waiting[at - 1].epoch === newest) {
    at--;
  }
  return waiting.splice(at, 1)[0];
}

/** Hands waiting jobs to workers with room. */
function dispatch() {
  for (const slot of pool()) {
    while (slot.busy < PER_WORKER && waiting.length > 0) {
      const job = take();
      slot.busy++;
      running.set(job.id, { job, slot });
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
    waiting.push({ id: next++, epoch, url, kind, cache, resolve, reject });
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
