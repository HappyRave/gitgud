import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { prefs, viewerChoice } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { font } from "$lib/test/font";
import FontViewer from "./FontViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

const loaded: string[] = [];

beforeEach(() => {
  prefs.viewer = {};
  loaded.length = 0;
  vi.stubGlobal(
    "FontFace",
    class {
      constructor(
        public family: string,
        public source: string,
      ) {
        loaded.push(source);
      }
      load = () => Promise.resolve(this);
    },
  );
  Object.defineProperty(document, "fonts", { configurable: true, value: { add: vi.fn(), delete: vi.fn() } });
});

describe("FontViewer", () => {
  it("sets the sample in both versions and tints the glyphs that changed", async () => {
    mockCommands({
      font_info: ({ version }) =>
        version === "head" ? font({ A: 1, B: 2, "€": 3 }) : font({ A: 1, B: 9, "₽": 4 }, { version: "Version 1.3" }),
    });
    render(FontViewer, { repoId: 1, path: "UI/Harbour.otf", before: HEAD, after: DISK });
    expect(await screen.findByText("OTF · v1.2 → v1.3")).toBeInTheDocument();
    expect(screen.getByText("4 → 4 glyphs")).toBeInTheDocument();
    expect(screen.getByText("1 outlines changed")).toBeInTheDocument();
    expect(screen.getAllByTitle("U+0042 redrawn")).toHaveLength(2);
    expect(loaded.every((s) => s.includes("/file?") && s.includes("path=UI%2FHarbour.otf"))).toBe(true);
    await fireEvent.input(screen.getByLabelText("Sample text"), { target: { value: "Ahoy" } });
    expect(screen.getAllByText("Ahoy")).toHaveLength(2);
  });

  it("remembers the view per format and shows metrics", async () => {
    mockCommands({ font_info: ({ version }) => font({ A: 1 }, { weight: version === "head" ? 400 : 700 }) });
    render(FontViewer, { repoId: 1, path: "UI/Harbour.ttf", before: HEAD, after: DISK });
    await fireEvent.click(await screen.findByRole("radio", { name: "Metrics" }));
    expect(viewerChoice("Other.ttf", "tab", ["sample", "glyphs", "metrics"])).toBe("metrics");
    const weight = screen.getByText("Weight").closest("tr")!;
    expect(weight).toHaveClass("changed");
    expect(weight).toHaveTextContent("Weight400700");
  });

  it("lists every glyph once for a new font and offers axis sliders", async () => {
    mockCommands({
      font_info: () => font({ A: 1, B: 2 }, { axes: [{ tag: "wght", name: "Weight", min: 100, default: 400, max: 900 }] }),
    });
    render(FontViewer, { repoId: 1, path: "UI/New.ttf", before: null, after: DISK });
    await fireEvent.click(await screen.findByRole("radio", { name: "All glyphs" }));
    expect(screen.getAllByTitle(/^U\+00/)).toHaveLength(2);
    expect(screen.getByText("3 glyphs")).toBeInTheDocument();
    expect(screen.getByLabelText("Weight axis")).toHaveValue("400");
  });

  it("says why a file can't be read", async () => {
    mockCommands({
      font_info: () => {
        throw new Error("UI/Broken.ttf: not a font this viewer can read (malformed font)");
      },
    });
    render(FontViewer, { repoId: 1, path: "UI/Broken.ttf", before: null, after: DISK });
    expect(await screen.findByRole("alert")).toHaveTextContent("not a font this viewer can read");
  });
});
