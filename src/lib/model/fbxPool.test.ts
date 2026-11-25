import { afterEach, describe, expect, it, vi } from "vitest";
import type { PackedModel } from "./packFbx";

/** A stand-in worker that keeps what it was sent until the test answers. */
class FakeWorker {
  static all: FakeWorker[] = [];
  sent: { id: number; url: string; kind: string }[] = [];
  onmessage: ((e: MessageEvent) => void) | null = null;
  constructor() {
    FakeWorker.all.push(this);
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
});
