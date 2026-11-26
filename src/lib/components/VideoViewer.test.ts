import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoInfo } from "$lib/api";
import type { ModelSide } from "$lib/model/open";
import { prefs } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import VideoViewer from "./VideoViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

const info = (over: Partial<VideoInfo> = {}): VideoInfo => ({
  codec: "H.264",
  width: 1920,
  height: 1080,
  duration: 8,
  fps: 25,
  frames: 200,
  bitrate: 8e6,
  audioCodec: "AAC",
  ...over,
});

let played: HTMLMediaElement[];

function metadata(video: HTMLVideoElement, duration: number) {
  Object.defineProperty(video, "duration", { configurable: true, value: duration });
  Object.defineProperty(video, "videoWidth", { configurable: true, value: 1920 });
  Object.defineProperty(video, "videoHeight", { configurable: true, value: 1080 });
  return fireEvent(video, new Event("loadedmetadata"));
}

beforeEach(() => {
  prefs.volume = 0.8;
  played = [];
  vi.stubGlobal("fetch", vi.fn(async () => new Response(new Blob(["mp4"]), { status: 200 })));
  URL.createObjectURL = vi.fn(() => "blob:x");
  URL.revokeObjectURL = vi.fn();
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    played.push(this);
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("VideoViewer", () => {
  it("plays both versions together, muted until asked, and steps frame by frame", async () => {
    mockCommands({ video_info: ({ version }) => (version === "head" ? info() : info({ bitrate: 12e6 })) });
    render(VideoViewer, { repoId: 1, path: "Video/Intro.mp4", before: HEAD, after: DISK });
    const [b, a] = (await screen.findAllByLabelText(/video$/)) as HTMLVideoElement[];
    await metadata(b, 8);
    await metadata(a, 8);
    expect(screen.getByText("MP4 · 1080p")).toBeInTheDocument();
    expect(screen.getByText("8.0 Mb/s → 12 Mb/s")).toHaveClass("d-mod");
    expect(played).toHaveLength(0);
    expect(a.muted && b.muted).toBe(true);

    await fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(played).toHaveLength(2);
    await fireEvent.click(screen.getByRole("button", { name: "Unmute" }));
    expect(a.muted).toBe(false);
    expect(b.muted).toBe(true);
    expect(a.volume).toBe(0.8);

    await fireEvent.click(screen.getByRole("button", { name: "Next frame" }));
    expect(a.currentTime).toBeCloseTo(0.04);
    expect(b.currentTime).toBeCloseTo(0.04);
    expect(screen.getByRole("button", { name: "Find where frames differ" })).toBeInTheDocument();
  });

  it("shows the earlier version's failure in its own panel", async () => {
    mockCommands({
      video_info: ({ version }) => {
        if (version === "head") {
          throw new Error("Video/Intro.mp4's LFS object isn't downloaded");
        }
        return info();
      },
    });
    render(VideoViewer, { repoId: 1, path: "Video/Intro.mp4", before: HEAD, after: DISK });
    expect(await screen.findAllByLabelText(/video$/)).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("HEAD can't load: Video/Intro.mp4's LFS object isn't downloaded");
  });

  it("offers the default app when the codec can't play here", async () => {
    mockCommands({ video_info: () => info({ codec: "ProRes" }) });
    render(VideoViewer, { repoId: 1, path: "Video/Master.mov", before: null, after: DISK });
    const [a] = (await screen.findAllByLabelText(/video$/)) as HTMLVideoElement[];
    Object.defineProperty(a, "error", { configurable: true, value: { code: 4 } });
    await fireEvent(a, new Event("error"));
    expect(screen.getByText("ProRes can't play here")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open On disk in default app" })).toBeInTheDocument();
  });

  it("reads WebM without the box parser", async () => {
    const calls = mockCommands({});
    render(VideoViewer, { repoId: 1, path: "Video/Loop.webm", before: null, after: DISK });
    const [a] = (await screen.findAllByLabelText(/video$/)) as HTMLVideoElement[];
    await metadata(a, 2.5);
    expect(screen.getByText("2.5 s")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "video_info")).toBe(false);
  });
});
