import { afterEach, describe, expect, it, vi } from "vitest";
import type { PackedModel } from "./packFbx";

/** A stand-in worker that keeps what it was sent until the test answers. */
class FakeWorker {
  static all: FakeWorker[] = [];
  sent: { id: number; url: string; kind: string }[] = [];
  onmessage: ((e: MessageEvent) => void) | null = null;
  onerror: ((e: ErrorEvent) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  terminated = false;
  constructor() {
    FakeWorker.all.push(this);
  }
  terminate() {
    this.terminated = true;
  }
  postMessage(data: { id: number; url: string; kind: string }) {
    this.sent.push(data);
  }
  answer(id: number, packed: Partial<PackedModel> | null) {
    this.onmessage?.({ data: packed ? { id, packed } : { id, error: "gone" } } as MessageEvent);
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  FakeWorker.all = [];
});

describe("loadGeometry", () => {
  it("keeps a bounded number of jobs in the workers, handed out in the order asked", async () => {
    vi.stubGlobal("Worker", FakeWorker);
    vi.stubGlobal("navigator", { hardwareConcurrency: 4 });
    const { loadGeometry } = await import("./fbxPool");
    const jobs = Array.from({ length: 20 }, (_, i) => loadGeometry(`m://${i}`, i % 2 ? "fbx" : "glb"));

    const [a, b] = FakeWorker.all;
    expect(FakeWorker.all).toHaveLength(2);
    expect(a.sent.map((s) => s.url)).toEqual(Array.from({ length: 8 }, (_, i) => `m://${i}`));
    expect(b.sent.map((s) => s.url)).toEqual(Array.from({ length: 8 }, (_, i) => `m://${i + 8}`));
    expect(a.sent[1].kind).toBe("fbx");

    a.answer(a.sent[0].id, { scale: 1, meshes: [], clips: [] });
    expect((await jobs[0]).scale).toBe(1);
    expect(a.sent.map((s) => s.url).at(-1)).toBe("m://16");

    b.answer(b.sent[0].id, null);
    await expect(jobs[8]).rejects.toThrow("gone");
    expect(b.sent.map((s) => s.url).at(-1)).toBe("m://17");
  });

  it("runs a newly opened model's jobs before those an earlier one left waiting", async () => {
    vi.stubGlobal("Worker", FakeWorker);
    vi.stubGlobal("navigator", { hardwareConcurrency: 3 });
    const { loadGeometry, preferNewest } = await import("./fbxPool");
    for (let i = 0; i < 12; i++) {
      void loadGeometry(`old://${i}`, "fbx");
    }
    preferNewest();
    void loadGeometry("new://0", "fbx");
    void loadGeometry("new://1", "fbx");
    const [w] = FakeWorker.all;
    const answer = (n: number) => {
      for (const s of w.sent.slice(0, n)) {
        w.answer(s.id, { scale: 1, meshes: [], clips: [] });
      }
    };
    answer(3);
    expect(w.sent.slice(8).map((s) => s.url)).toEqual(["new://0", "new://1", "old://8"]);
  });

  it("fails a dead worker's jobs and gives the rest to its replacement", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.stubGlobal("Worker", FakeWorker);
    vi.stubGlobal("navigator", { hardwareConcurrency: 3 });
    const { loadGeometry } = await import("./fbxPool");
    const jobs = Array.from({ length: 10 }, (_, i) => loadGeometry(`m://${i}`, "fbx"));
    for (const j of jobs) {
      j.catch(() => {});
    }
    const [dead] = FakeWorker.all;
    dead.onerror?.({ message: "out of memory" } as ErrorEvent);

    await expect(jobs[0]).rejects.toThrow("m://0: out of memory");
    await expect(jobs[7]).rejects.toThrow("out of memory");
    expect(dead.terminated).toBe(true);
    const [, fresh] = FakeWorker.all;
    expect(fresh.sent.map((s) => s.url)).toEqual(["m://8", "m://9"]);
    fresh.answer(fresh.sent[0].id, { scale: 2, meshes: [], clips: [] });
    expect((await jobs[8]).scale).toBe(2);
  });

  it("passes the cache URL on, and has workers keep their parses only once the pool has been idle", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("Worker", FakeWorker);
    vi.stubGlobal("navigator", { hardwareConcurrency: 3 });
    const { loadGeometry } = await import("./fbxPool");
    const flushes = () => FakeWorker.all[0].sent.filter((s) => "flush" in s).length;
    const one = loadGeometry("m://a", "fbx", "g://a");
    const two = loadGeometry("m://b", "fbx");
    const [w] = FakeWorker.all;
    expect(w.sent.map((s) => (s as { cache?: string }).cache)).toEqual(["g://a", undefined]);

    w.answer(w.sent[0].id, { scale: 1, meshes: [], clips: [] });
    await one;
    vi.advanceTimersByTime(5000);
    expect(flushes(), "a job still runs").toBe(0);

    w.answer(w.sent[1].id, { scale: 1, meshes: [], clips: [] });
    await two;
    vi.advanceTimersByTime(500);
    expect(flushes()).toBe(0);
    vi.advanceTimersByTime(600);
    expect(flushes()).toBe(1);
    vi.useRealTimers();
  });
});
