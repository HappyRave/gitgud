import { describe, expect, it } from "vitest";
import { sound } from "$lib/test/audio";
import { fakeContext } from "$lib/test/canvas";
import { audioCaption, clock, drawWave, needsWav } from "./audio";

describe("audioCaption", () => {
  it("spells out length, peak and loudness changes", () => {
    const parts = audioCaption(sound({ duration: 2.4, peakDbfs: -3.1, loudnessLufs: -20 }), sound({ duration: 2.85, peakDbfs: -1.2, loudnessLufs: -17.6 }));
    expect(parts.map((p) => p.text)).toEqual(["2.40 → 2.85 s", "+0.45 s", "48 kHz", "mono", "peak −3.1 dBFS → −1.2 dBFS", "loudness +2.4 LU"]);
    expect(parts.filter((p) => p.changed).map((p) => p.text)).toEqual(["+0.45 s", "peak −3.1 dBFS → −1.2 dBFS", "loudness +2.4 LU"]);
  });

  it("flags a resample and a new channel layout", () => {
    const parts = audioCaption(sound({ sampleRate: 44100 }), sound({ channels: 2 }));
    expect(parts.filter((p) => p.changed).map((p) => p.text)).toEqual(["44.1 kHz → 48 kHz", "mono → stereo"]);
    expect(parts.at(-1)?.text).toBe("same loudness");
  });

  it("describes a lone version", () => {
    expect(audioCaption(null, sound({ peakDbfs: null, loudnessLufs: null })).map((p) => p.text)).toEqual([
      "2.00 s",
      "48 kHz",
      "mono",
      "peak silent",
    ]);
  });
});

describe("drawWave", () => {
  it("fills only the part of the shared axis the version lasts", () => {
    const g = fakeContext();
    drawWave(g, [[-0.5, 0.5]], 100, 40, 1, 2, "#00f");
    const bars = g.calls.filter((c) => c.op === "fillRect" && c.fillStyle === "#00f");
    expect(bars).toHaveLength(50);
    expect(bars[0].args).toEqual([0, 10, 1, 20]);
  });
});

it("plays AIFF through Rust's WAV and formats the clock", () => {
  expect([needsWav("a/Horn.AIFF"), needsWav("a/x.aif"), needsWav("a/x.wav")]).toEqual([true, true, false]);
  expect(clock(1.234, 2.85)).toBe("1.23 / 2.85 s");
});
