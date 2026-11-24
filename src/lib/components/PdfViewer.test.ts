import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { mockCommands } from "$lib/test/ipc";
import type { PdfPageText } from "$lib/viewers/pdfdoc";
import PdfViewer from "./PdfViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

const texts: Record<string, string[][]> = {};
const rendered: { doc: string; index: number; width: number }[] = [];

const run = (str: string, x: number) => ({ str, t: [1, 0, 0, 10, x, 700], w: 40, h: 10 });
const page = (runs: string[]): PdfPageText => ({ text: runs.join(" "), runs: runs.map((s, i) => run(s, i * 50)) });

vi.mock("$lib/viewers/pdfdoc", () => ({
  openPdf: async (bytes: ArrayBuffer) => {
    const name = new TextDecoder().decode(bytes);
    return { name, numPages: texts[name].length, loadingTask: { destroy: vi.fn(async () => {}) } };
  },
  readPages: async (doc: { name: string }, progress: (n: number) => void) => {
    const pages = texts[doc.name].map(page);
    progress(pages.length);
    return pages;
  },
  renderPage: async (doc: { name: string }, index: number, _canvas: HTMLCanvasElement, width: number, runs: { str: string }[]) => {
    rendered.push({ doc: doc.name, index, width });
    return { width, height: 100, boxes: runs.map((_, i) => ({ x: i * 50, y: 0, w: 40, h: 10 })) };
  },
}));

beforeEach(() => {
  rendered.length = 0;
  texts.head = [["Intro"], ["Two tugs", "turn back"], ["Fuel"]];
  texts.worktree = [["Intro"], ["Two tugs", "follow"], ["Map"], ["Fuel"]];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => new Response(new URL(url).searchParams.get("at") ?? "", { status: 200 })),
  );
  Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => 800 });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PdfViewer", () => {
  it("opens on the first changed page, highlights changed text and lists pages in the rail", async () => {
    mockCommands({});
    render(PdfViewer, { repoId: 1, path: "Docs/GDD.pdf", before: HEAD, after: DISK });
    expect(await screen.findByText("PDF · 4 pages")).toBeInTheDocument();
    const rail = screen.getAllByRole("button").filter((b) => b.classList.contains("page"));
    expect(rail.map((b) => b.textContent)).toEqual(["1", "2", "3", "3→4"]);
    expect(rail[1]).toHaveAttribute("aria-current", "page");
    expect(rail[2]).toHaveClass("added");
    await vi.waitFor(() => expect(document.querySelectorAll(".hl")).toHaveLength(2));
    expect(rendered.map((r) => `${r.doc}:${r.index}`)).toContain("head:1");
    expect(screen.getByText("1 changed (p.2)")).toBeInTheDocument();
    expect(screen.getByText("1 added (p.3)")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("radio", { name: "Swipe" }));
    expect(screen.getByRole("button", { name: "Drag to swipe" })).toBeInTheDocument();
    await fireEvent.click(rail[2]);
    expect(screen.getByRole("radio", { name: "Swipe" })).toBeDisabled();
  });

  it("says why a PDF can't be opened", async () => {
    mockCommands({});
    vi.stubGlobal("fetch", vi.fn(async () => new Response("Docs/GDD.pdf isn't in that version", { status: 404 })));
    render(PdfViewer, { repoId: 1, path: "Docs/GDD.pdf", before: null, after: DISK });
    expect(await screen.findByRole("alert")).toHaveTextContent("isn't in that version");
  });
});
