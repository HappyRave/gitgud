import { describe, expect, it } from "vitest";
import { LoadProgress } from "./progress";

describe("LoadProgress", () => {
  it("walks through the stages of a two-version load", () => {
    const p = new LoadProgress((flush) => flush());
    const seen: string[] = [];
    p.onchange = (s) => seen.push(`${s.label}${s.value === null ? "" : ` @${s.value.toFixed(2)}`}`);

    p.report("before:info", { phase: "info" });
    p.report("before:info", { phase: "done" });
    p.report("before:boat.fbx", { phase: "request", name: "boat.fbx" });
    p.report("after:boat.fbx", { phase: "request", name: "boat.fbx" });
    p.report("before:boat.fbx", { phase: "download", loaded: 1024 * 1024, total: 4 * 1024 * 1024 });
    p.report("after:boat.fbx", { phase: "download", loaded: 0, total: 4 * 1024 * 1024 });
    p.report("before:boat.fbx", { phase: "parse" });
    p.report("after:boat.fbx", { phase: "download", loaded: 4 * 1024 * 1024, total: 4 * 1024 * 1024 });
    p.report("after:boat.fbx", { phase: "files", loaded: 1, total: 4 });
    p.report("before:boat.fbx", { phase: "done" });
    p.report("after:boat.fbx", { phase: "done" });

    expect(seen).toEqual([
      "Reading Unity references…",
      "Opening in three.js…",
      "Waiting for boat.fbx…",
      "Waiting for boat.fbx and 1 more…",
      "Downloading 1.0 MB / 4.0 MB @0.25",
      "Downloading 1.0 MB / 8.0 MB @0.13",
      "Downloading 1.0 MB / 8.0 MB @0.13",
      "Downloading 5.0 MB / 8.0 MB @0.63",
      "Loading textures 1 / 4 @0.25",
      "Loading textures 1 / 4 @0.25",
      "Opening in three.js…",
    ]);
  });

  it("sums up a burst of steps once per frame", () => {
    const frames: (() => void)[] = [];
    const p = new LoadProgress((flush) => frames.push(flush));
    const seen: string[] = [];
    p.onchange = (s) => seen.push(s.label);
    for (let i = 0; i < 500; i++) {
      p.report(`mesh${i}`, { phase: "request", name: `mesh${i}` });
    }
    expect(frames).toHaveLength(1);
    expect(seen).toEqual([]);
    frames.shift()!();
    expect(seen).toEqual(["Waiting for mesh0 and 499 more…"]);
    p.report("mesh0", { phase: "done" });
    expect(frames).toHaveLength(1);
  });

  it("can't measure a download without a size", () => {
    const p = new LoadProgress();
    p.report("a", { phase: "download", loaded: 2048, total: 0 });
    expect(p.summary()).toEqual({ label: "Downloading 2.0 KB…", value: null });
  });
});
