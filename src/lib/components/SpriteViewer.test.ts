import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import type { Sprite } from "$lib/api";
import type { ModelSide } from "$lib/model/open";
import { prefs, viewerChoice } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { SPRITE_LAYERS } from "$lib/viewers/sprites";
import SpriteViewer from "./SpriteViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };
const sp = (name: string, x: number, border = 0): Sprite => ({ name, rect: [x, 0, 32, 32], pivot: [0.5, 0.5], border: [border, 0, 0, 0] });

async function loadImages() {
  for (const img of screen.getAllByRole("img", { name: /texture$/ }) as HTMLImageElement[]) {
    Object.defineProperty(img, "naturalWidth", { configurable: true, value: 128 });
    Object.defineProperty(img, "naturalHeight", { configurable: true, value: 32 });
    await fireEvent.load(img);
  }
}

beforeEach(() => {
  prefs.viewer = {};
});

describe("SpriteViewer", () => {
  it("draws each version's slices tinted by change and always shows a changed 9-slice border", async () => {
    const calls = mockCommands({
      sprite_sheet: () => ({
        mode: "multiple",
        before: [sp("anchor", 0), sp("raft", 64)],
        after: [sp("anchor", 0, 4), sp("fuel", 96)],
        changes: [
          { name: "anchor", kinds: ["border"], note: "9-slice border changed" },
          { name: "fuel", kinds: ["added"], note: "32×32" },
          { name: "raft", kinds: ["removed"], note: "" },
        ],
      }),
    });
    render(SpriteViewer, { repoId: 1, path: "UI/HUD.png", before: HEAD, after: DISK });
    expect(await screen.findByText("Sprite sheet · 2 slices")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "sprite_sheet")?.args).toEqual({ id: 1, texture: "UI/HUD.png", before: "head", after: "worktree" });
    await loadImages();
    expect(screen.getByRole("img", { name: "raft removed" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "fuel added" })).toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: "anchor changed" })).toHaveLength(2);
    const after = screen.getByLabelText("On disk slices");
    expect(after.querySelector("rect")).toHaveAttribute("y", "0");
    expect(after.querySelectorAll(".nine")).toHaveLength(1);

    await fireEvent.click(screen.getByRole("radio", { name: "Pivots" }));
    expect(after.querySelectorAll("circle")).toHaveLength(2);
    expect(viewerChoice("Other.png", "sprites", SPRITE_LAYERS)).toBe("pivots");
    await fireEvent.click(screen.getByRole("radio", { name: "Pixels" }));
    expect(screen.queryByLabelText("On disk slices")).toBeNull();
  });

  it("shows a plain texture without slice controls", async () => {
    mockCommands({ sprite_sheet: () => ({ mode: "none", before: [], after: [], changes: [] }) });
    render(SpriteViewer, { repoId: 1, path: "Art/rock.png", before: HEAD, after: DISK });
    expect(await screen.findByText("not imported as sprites")).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "Slices" })).toBeNull();
  });
});
