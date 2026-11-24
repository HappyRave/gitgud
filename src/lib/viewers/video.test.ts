import { describe, expect, it } from "vitest";
import type { VideoInfo } from "$lib/api";
import { diffSpans, frameDifference, quality, sampleTimes, timecode, videoCaption } from "./video";

const info = (over: Partial<VideoInfo> = {}): VideoInfo => ({
  codec: "H.264",
  width: 1920,
  height: 1080,
  duration: 8,
  fps: 30,
  frames: 240,
  bitrate: 8e6,
  audioCodec: null,
  ...over,
});
const meta = { width: 1920, height: 1080, duration: 8 };

describe("videoCaption", () => {
  it("flags what changed between versions", () => {
    const parts = videoCaption({ info: info(), meta }, { info: info({ bitrate: 12e6, codec: "HEVC" }), meta });
    expect(parts.map((p) => p.text)).toEqual(["8.0 s", "1920×1080", "30 fps", "H.264 → HEVC", "8.0 Mb/s → 12 Mb/s", "no sound"]);
    expect(parts.filter((p) => p.changed)).toHaveLength(2);
  });

  it("falls back to what the player knows for WebM", () => {
    expect(videoCaption(null, { info: null, meta: { width: 640, height: 360, duration: 2.5 } }).map((p) => p.text)).toEqual([
      "2.5 s",
      "640×360",
    ]);
  });
});

describe("frame comparison", () => {
  it("measures mean difference and finds the stretches over the threshold", () => {
    const a = new Uint8ClampedArray([0, 0, 0, 255, 100, 100, 100, 255]);
    const b = new Uint8ClampedArray([0, 0, 0, 255, 106, 106, 106, 255]);
    expect(frameDifference(a, b)).toBe(3);
    expect(diffSpans([0, 9, 9, 0, 0, 7])).toEqual([
      [1 / 6, 3 / 6],
      [5 / 6, 1],
    ]);
    expect(diffSpans([1, 2])).toEqual([]);
  });

  it("samples one time per frame, capped", () => {
    expect(sampleTimes(1, 4)).toEqual([0.125, 0.375, 0.625, 0.875]);
    expect(sampleTimes(600, 60)).toHaveLength(240);
  });
});

it("formats time and quality", () => {
  expect(timecode(64.32)).toBe("01:04.3");
  expect([quality(1080), quality(2160), quality(0)]).toEqual(["1080p", "4K", ""]);
});
