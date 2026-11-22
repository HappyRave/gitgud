import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
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
    expect(li).toHaveTextContent(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}/);
    expect(within(screen.getByText("commit 1").closest("li")!).getByText("HEAD")).toHaveClass("head");
  });

  it("shows the working-copy row without a date", async () => {
    serve(2, { 0: { wip: true, subject: "Uncommitted changes", id: "", shortId: "" } });
    render(Graph, { repoId: 1, rowCount: 2 });

    const li = (await screen.findByText("Uncommitted changes")).closest("li")!;
    expect(li).toHaveClass("wip");
    expect(li).not.toHaveTextContent(/\d{4}-/);
  });

  it("selects a row on click", async () => {
    serve(3);
    render(Graph, { repoId: 1, rowCount: 3 });

    const li = (await screen.findByText("commit 1")).closest("li")!;
    await fireEvent.click(li);

    expect(li).toHaveAttribute("aria-selected", "true");
    expect(selectedSubjects()).toHaveLength(1);
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

  it("shows a failed fetch", async () => {
    mockCommands({
      graph_rows: () => {
        throw "repository 1 is not open";
      },
    });
    render(Graph, { repoId: 1, rowCount: 10 });

    expect(await screen.findByRole("alert")).toHaveTextContent("repository 1 is not open");
  });
});
