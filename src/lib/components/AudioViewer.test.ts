import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { prefs } from "$lib/prefs.svelte";
import { sound } from "$lib/test/audio";
import { mockCommands } from "$lib/test/ipc";
import AudioViewer from "./AudioViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

let fetched: string[];
let played: HTMLMediaElement[];

beforeEach(() => {
  prefs.viewer = {};
  prefs.volume = 0.8;
  fetched = [];
  played = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      fetched.push(url);
      return new Response(new Blob(["RIFF"]), { status: 200 });
    }),
  );
  URL.createObjectURL = vi.fn(() => `blob:${fetched.length}`);
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

describe("AudioViewer", () => {
  it("stacks both versions, never autoplays, and A/B swaps what plays at the same moment", async () => {
    mockCommands({
      audio_info: ({ version }) => (version === "head" ? sound({ duration: 2.4 }) : sound({ duration: 2.85, loudnessLufs: -15.6 })),
    });
    render(AudioViewer, { repoId: 1, path: "Audio/Horn.wav", before: HEAD, after: DISK });
    expect(await screen.findByText("HEAD · 2.40 s")).toBeInTheDocument();
    expect(screen.getByText("On disk · 2.85 s")).toBeInTheDocument();
    expect(screen.getByText("loudness +2.4 LU")).toHaveClass("d-mod");
    expect(played).toHaveLength(0);
    expect(fetched.every((u) => u.includes("/file?"))).toBe(true);

    await fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(played).toHaveLength(1);
    const b = played[0];
    b.currentTime = 1.5;
    await fireEvent.click(screen.getByRole("radio", { name: "A" }));
    expect(played).toHaveLength(2);
    expect(played[1]).not.toBe(b);
    expect(played[1].currentTime).toBe(1.5);
    expect(played[1].volume).toBe(0.8);
    expect(prefs.viewer["wav.ab"]).toBe("a");
  });

  it("plays AIFF through the decoded WAV and keeps the volume app-wide", async () => {
    mockCommands({ audio_info: () => sound() });
    render(AudioViewer, { repoId: 1, path: "Audio/Bell.aiff", before: null, after: DISK });
    await screen.findByText("On disk · 2.00 s");
    expect(fetched[0]).toContain("/audio?");
    expect(screen.queryByRole("radio", { name: "A" })).toBeNull();
    await fireEvent.input(screen.getByLabelText("Volume"), { target: { value: "0.3" } });
    expect(prefs.volume).toBe(0.3);
    expect(localStorage.getItem("gitgud.media.volume")).toBe("0.3");
  });

  it("says why a file can't be decoded", async () => {
    mockCommands({
      audio_info: () => {
        throw new Error("Audio/x.wav: not a sound file this viewer can read (malformed stream)");
      },
    });
    render(AudioViewer, { repoId: 1, path: "Audio/x.wav", before: null, after: DISK });
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("not a sound file"));
  });
});
