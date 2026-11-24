import type { AudioInfo } from "$lib/api";

/** A mono 48 kHz, 2 s AudioInfo; `over` replaces any field. */
export function sound(over: Partial<AudioInfo> = {}): AudioInfo {
  return {
    format: "Waveform Audio File Format",
    codec: "PCM Signed 16-bit Little-Endian Interleaved",
    sampleRate: 48000,
    channels: 1,
    bitsPerSample: 16,
    frames: 96000,
    duration: 2,
    peakDbfs: -3,
    loudnessLufs: -18,
    peaks: [[[-0.5, 0.5]]],
    ...over,
  };
}
