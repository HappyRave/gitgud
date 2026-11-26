import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Row } from "$lib/api";
import { contextOf } from "$lib/test/canvas";
import { mockCommands } from "$lib/test/ipc";
import { CHUNK, ROW_HEIGHT } from "$lib/graph/window";
import Graph from "./Graph.svelte";

const VIEWPORT = 20 * ROW_HEIGHT;

function row(i: number, extra: Partial<Row> = {}): Row {
  return {
    id: `${i}`.padStart(40, "0"),
    shortId: `${i}`.padStart(7, "0"),
    subject: `commit ${i}`,
    author: "Ada",
    time: 1_700_000_000,
    labels: [],
    wip: false,
    layout: { node: 0, segments: [{ from: 0, to: 0, part: "through" }], overflow: false },
    ...extra,
  };
}

function serve(total: number, overrides: Record<number, Partial<Row>> = {}) {
  return mockCommands({
    graph_rows: ({ start, count }) =>
      Array.from({ length: Math.max(0, Math.min(count, total - start)) }, (_, i) => row(start + i, overrides[start + i])),
  });
}

const list = () => screen.getByRole("listbox", { name: "Commits" });
const items = () => within(list()).queryAllByRole("option");
const selectedSubjects = () =>
  items()
    .filter((i) => i.getAttribute("aria-selected") === "true")
    .map((i) => i.textContent);

// jsdom has no layout: give every element the viewport's height, then fall back
// to Element.prototype's own getter again.
beforeEach(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, get: () => VIEWPORT });
});
afterEach(() => {
  delete (HTMLElement.prototype as { clientHeight?: number }).clientHeight;
});

describe("Graph", () => {
  it("renders only the rows in view plus overscan", async () => {
    serve(5000);
    render(Graph, { repoId: 1, rowCount: 5000 });

    await screen.findByText("commit 0");
    expect(items()).toHaveLength(30);
    expect(list()).toHaveStyle({ height: `${5000 * ROW_HEIGHT}px` });
  });

  it("asks for the first chunk with the repo id", async () => {
    const calls = serve(5000);
    render(Graph, { repoId: 7, rowCount: 5000 });

    await screen.findByText("commit 0");
    expect(calls).toEqual([{ cmd: "graph_rows", args: { id: 7, start: 0, count: CHUNK } }]);
  });

  it("fetches the next chunk when scrolled to it", async () => {
    const calls = serve(5000);
    const { container } = render(Graph, { repoId: 1, rowCount: 5000 });
    await screen.findByText("commit 0");

    const viewport = container.querySelector(".viewport")!;
    viewport.scrollTop = 1000 * ROW_HEIGHT;
    await fireEvent.scroll(viewport);

    expect(await screen.findByText("commit 1000")).toBeInTheDocument();
    expect(calls.map((c) => (c.args as { start: number }).start)).toContain(1000 - (1000 % CHUNK));
    expect(screen.queryByText("commit 0")).not.toBeInTheDocument();
  });

  it("shows labels, author, date and short id", async () => {
    serve(3, {
      0: {
        labels: [
          { kind: "branch", name: "main", current: true },
          { kind: "remote", name: "origin/main" },
          { kind: "tag", name: "v1" },
        ],
      },
      1: { labels: [{ kind: "head" }] },
    });
    render(Graph, { repoId: 1, rowCount: 3 });

    const first = await screen.findByText("commit 0");
    const li = first.closest("li")!;
    expect(within(li).getByText("main")).toHaveClass("label", "branch", "current");
    expect(within(li).getByText("origin/main")).toHaveClass("remote");
    expect(within(li).getByText("v1")).toHaveClass("tag");
    expect(li).toHaveTextContent("Ada");
    expect(li).toHaveTextContent("0000000");
    expect(li.querySelector(".date")?.getAttribute("title")).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    expect(li.querySelector(".date")?.textContent).not.toBe("");
    expect(within(li).getByText("main").style.getPropertyValue("--lane")).toBe("var(--l0)");
    expect(within(screen.getByText("commit 1").closest("li")!).getByText("HEAD")).toHaveClass("head");
  });

  it("shows the working-copy row without a date", async () => {
    serve(2, { 0: { wip: true, subject: "Uncommitted changes", id: "", shortId: "" } });
    render(Graph, { repoId: 1, rowCount: 2 });

    const li = (await screen.findByText("Uncommitted changes")).closest("li")!;
    expect(li).toHaveClass("wip");
    expect(li.querySelector(".date")?.textContent).toBe("");
    expect(li.querySelector(".date")).not.toHaveAttribute("title");
  });

  it("selects a row on click", async () => {
    serve(3);
    render(Graph, { repoId: 1, rowCount: 3 });

    const li = (await screen.findByText("commit 1")).closest("li")!;
    await fireEvent.click(li);

    expect(li).toHaveAttribute("aria-selected", "true");
    expect(selectedSubjects()).toHaveLength(1);
  });

  it("Ctrl+click on a second row compares the selected row with it", async () => {
    serve(3);
    const oncompare = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, oncompare });

    const first = (await screen.findByText("commit 0")).closest("li")!;
    const second = screen.getByText("commit 2").closest("li")!;
    await fireEvent.click(second, { ctrlKey: true });
    expect(oncompare, "nothing selected yet: a plain select").not.toHaveBeenCalled();
    await fireEvent.click(first);
    await fireEvent.click(second, { ctrlKey: true });

    expect(oncompare).toHaveBeenCalledTimes(1);
    expect(oncompare.mock.calls[0].map((r: Row) => r.subject)).toEqual(["commit 0", "commit 2"]);
    expect(first).toHaveAttribute("aria-selected", "true");
  });

  it("moves the selection with the arrow keys, within the history", async () => {
    serve(3);
    render(Graph, { repoId: 1, rowCount: 3 });
    await screen.findByText("commit 2");

    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    expect(selectedSubjects()).toEqual([expect.stringContaining("commit 0")]);
    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    expect(selectedSubjects()).toEqual([expect.stringContaining("commit 2")]);
    await fireEvent.keyDown(list(), { key: "ArrowUp" });
    expect(selectedSubjects()).toEqual([expect.stringContaining("commit 1")]);
  });

  it("reports the selected row, and null when Escape clears it", async () => {
    serve(3);
    const onselect = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, onselect });
    await screen.findByText("commit 2");

    await fireEvent.click(screen.getByText("commit 1"));
    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    await fireEvent.keyDown(list(), { key: "Escape" });

    expect(onselect.mock.calls.map(([row]) => row?.subject ?? null)).toEqual(["commit 1", "commit 2", null]);
    expect(selectedSubjects()).toEqual([]);
  });

  it("Ctrl+C hands over the selected commit; nothing for the working-copy row", async () => {
    serve(3, { 0: { wip: true, subject: "Uncommitted changes" } });
    const oncopy = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, oncopy });
    await screen.findByText("commit 2");

    await fireEvent.click(screen.getByText("Uncommitted changes"));
    await fireEvent.keyDown(list(), { key: "c", ctrlKey: true });
    await fireEvent.click(screen.getByText("commit 2"));
    await fireEvent.keyDown(list(), { key: "c", ctrlKey: true });

    expect(oncopy.mock.calls.map(([row]) => row.subject)).toEqual(["commit 2"]);
  });

  it("reveal selects a far row, reports it once loaded, and marks find hits", async () => {
    const calls = serve(1000);
    const onselect = vi.fn();
    const view = render(Graph, { repoId: 1, rowCount: 1000, onselect, hits: new Set([700]) });
    await screen.findByText("commit 0");

    view.component.reveal(700);

    await vi.waitFor(() => expect(onselect).toHaveBeenCalledWith(expect.objectContaining({ subject: "commit 700" })));
    expect(onselect).not.toHaveBeenCalledWith(null);
    expect(calls.some((c) => (c.args as { start: number }).start <= 700)).toBe(true);
    expect((await screen.findByText("commit 700")).closest("li")).toHaveClass("hit", "selected");
  });

  it("follows a selection set from outside", async () => {
    serve(3);
    const view = render(Graph, { repoId: 1, rowCount: 3, selected: 2 });
    await screen.findByText("commit 2");
    expect(selectedSubjects()).toEqual([expect.stringContaining("commit 2")]);

    await view.rerender({ repoId: 1, rowCount: 3, selected: null });

    expect(selectedSubjects()).toEqual([]);
  });

  it("scrolls a row selected by keyboard into view", async () => {
    serve(5000);
    const { container } = render(Graph, { repoId: 1, rowCount: 5000 });
    await screen.findByText("commit 0");
    const viewport = container.querySelector(".viewport")!;

    for (let i = 0; i < 25; i++) {
      await fireEvent.keyDown(list(), { key: "ArrowDown" });
    }

    expect(viewport.scrollTop).toBe(25 * ROW_HEIGHT - VIEWPORT);
    expect(selectedSubjects()).toEqual([expect.stringContaining("commit 24")]);
  });

  it("draws the visible rows' lanes on the canvas", async () => {
    serve(3);
    const { container } = render(Graph, { repoId: 1, rowCount: 3 });
    await screen.findByText("commit 2");

    const ctx = contextOf(container.querySelector("canvas")!);
    expect(ctx.calls.filter((c) => c.op === "arc").length).toBeGreaterThanOrEqual(3);
  });

  it("widens the graph column for wide rows", async () => {
    serve(2, { 1: { layout: { node: 3, segments: [], overflow: false } } });
    const { container } = render(Graph, { repoId: 1, rowCount: 2 });
    await screen.findByText("commit 1");

    expect(container.querySelector("canvas")).toHaveStyle({ width: `${4 * 14 + 8}px` });
  });

  it("draws a plan preview from its own rows: after rows tagged, replaced rows faded", async () => {
    mockCommands({});
    const preview = (start: number, count: number) =>
      Promise.resolve(
        [row(0, { id: "", shortId: "", subject: "Move camera right", after: "reworded" }), row(1, { faded: true })].slice(start, start + count),
      );
    const { rerender } = render(Graph, { repoId: 1, rowCount: 2, rows: preview, rowsVersion: 1 });

    const ghost = (await screen.findByText("Move camera right")).closest("li")!;
    expect(ghost).toHaveClass("ghost");
    expect(within(ghost).getByText("reworded")).toBeInTheDocument();
    expect(screen.getByText("commit 1").closest("li")).toHaveClass("faded");

    const next = (start: number, count: number) => Promise.resolve([row(0, { subject: "Move camera, again", after: "reworded" })].slice(start, start + count));
    await rerender({ repoId: 1, rowCount: 1, rows: next, rowsVersion: 2 });
    expect(await screen.findByText("Move camera, again")).toBeInTheDocument();
  });

  it("R, F, D and Shift+E on a selected commit ask for an edit; not on the working copy or an after row", async () => {
    serve(3, { 0: { wip: true, subject: "Uncommitted changes" } });
    const onedit = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, onedit });
    await screen.findByText("commit 2");

    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    await fireEvent.keyDown(list(), { key: "r" });
    expect(onedit).not.toHaveBeenCalled();
    await fireEvent.keyDown(list(), { key: "ArrowDown" });
    await fireEvent.keyDown(list(), { key: "f" });
    await fireEvent.keyDown(list(), { key: "E", shiftKey: true });
    expect(onedit.mock.calls.map(([r, e]) => [r.subject, e])).toEqual([
      ["commit 1", "fixup"],
      ["commit 1", "plan"],
    ]);
  });

  it("right-click on a commit selects it and asks for the menu", async () => {
    serve(3);
    const onmenu = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, onmenu });
    const target = await screen.findByText("commit 2");

    await fireEvent.contextMenu(target, { clientX: 40, clientY: 50 });

    expect(onmenu).toHaveBeenCalledWith(expect.objectContaining({ subject: "commit 2" }), 40, 50);
    expect(selectedSubjects()[0]).toContain("commit 2");
  });

  it("renames inline: Enter gives the new subject, Esc gives null", async () => {
    serve(3);
    const onrename = vi.fn();
    render(Graph, { repoId: 1, rowCount: 3, renaming: 0, onrename });

    const input = await screen.findByRole("textbox", { name: "New commit subject" });
    expect(input).toHaveValue("commit 0");
    expect(screen.getByText("Enter save · Esc cancel")).toBeInTheDocument();
    await fireEvent.input(input, { target: { value: "commit zero" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onrename).toHaveBeenCalledWith("commit zero");
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(onrename).toHaveBeenLastCalledWith(null);
  });

  it("shows a failed fetch", async () => {
    mockCommands({
      graph_rows: () => {
        throw "repository 1 is not open";
      },
    });
    render(Graph, { repoId: 1, rowCount: 10 });

    expect(await screen.findByRole("alert")).toHaveTextContent("repository 1 is not open");
  });

  it("exposes virtualized ARIA setsize, posinset, and aria-activedescendant", async () => {
    serve(100);
    render(Graph, { repoId: 1, rowCount: 100 });
    const row2 = (await screen.findByText("commit 2")).closest("li")!;

    expect(list()).not.toHaveAttribute("aria-activedescendant");
    expect(row2).toHaveAttribute("aria-setsize", "100");
    expect(row2).toHaveAttribute("aria-posinset", "3");
    expect(row2.id).not.toBe("");

    await fireEvent.click(row2);
    expect(list()).toHaveAttribute("aria-activedescendant", row2.id);

    await fireEvent.keyDown(list(), { key: "Escape" });
    expect(list()).not.toHaveAttribute("aria-activedescendant");
  });

  it("resizes the canvas backing store when devicePixelRatio changes", async () => {
    const origDpr = Object.getOwnPropertyDescriptor(window, "devicePixelRatio");
    const origMatchMedia = window.matchMedia;
    let currentDpr = 1;
    const listeners = new Map<string, Set<() => void>>();

    Object.defineProperty(window, "devicePixelRatio", {
      configurable: true,
      get: () => currentDpr,
    });
    window.matchMedia = ((query: string) => {
      const set = listeners.get(query) ?? new Set<() => void>();
      listeners.set(query, set);
      return {
        matches: false,
        media: query,
        addEventListener: (_: string, fn: () => void) => set.add(fn),
        removeEventListener: (_: string, fn: () => void) => set.delete(fn),
      } as unknown as MediaQueryList;
    }) as typeof window.matchMedia;

    try {
      serve(3);
      const { container } = render(Graph, { repoId: 1, rowCount: 3 });
      await screen.findByText("commit 2");

      const canvas = container.querySelector("canvas")!;
      expect(canvas.height).toBe(VIEWPORT);

      currentDpr = 2;
      for (const fn of listeners.get("(resolution: 1dppx)") ?? []) {
        fn();
      }

      await vi.waitFor(() => expect(canvas.height).toBe(VIEWPORT * 2));
      expect(listeners.get("(resolution: 2dppx)")?.size).toBe(1);
    } finally {
      window.matchMedia = origMatchMedia;
      if (origDpr) {
        Object.defineProperty(window, "devicePixelRatio", origDpr);
      } else {
        delete (window as { devicePixelRatio?: number }).devicePixelRatio;
      }
    }
  });

  it("updates visible rows via ResizeObserver when the pane resizes", async () => {
    const origResizeObserver = globalThis.ResizeObserver;
    const observers: ((entries: ResizeObserverEntry[]) => void)[] = [];
    const disconnect = vi.fn();

    globalThis.ResizeObserver = class {
      constructor(cb: (entries: ResizeObserverEntry[]) => void) {
        observers.push(cb);
      }
      observe() {}
      unobserve() {}
      disconnect() {
        disconnect();
      }
    } as unknown as typeof ResizeObserver;

    try {
      serve(5000);
      const view = render(Graph, { repoId: 1, rowCount: 5000 });
      await screen.findByText("commit 0");
      expect(items()).toHaveLength(30);

      for (const cb of observers) {
        cb([{ contentRect: { height: 2 * ROW_HEIGHT } as DOMRectReadOnly } as ResizeObserverEntry]);
      }
      await vi.waitFor(() => expect(items()).toHaveLength(12));

      view.unmount();
      expect(disconnect).toHaveBeenCalledTimes(1);
    } finally {
      globalThis.ResizeObserver = origResizeObserver;
    }
  });
});
