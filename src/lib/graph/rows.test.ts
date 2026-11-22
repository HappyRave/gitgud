import { describe, expect, it, vi } from "vitest";
import type { Row } from "$lib/api";
import { RowStore } from "./rows.svelte";
import { CHUNK } from "./window";

const row = (i: number): Row => ({
  id: `id${i}`,
  shortId: `s${i}`,
  subject: `commit ${i}`,
  author: "a",
  time: 0,
  labels: [],
  wip: false,
  layout: { node: 0, segments: [], overflow: false },
});

const fetcher = () => vi.fn(async (start: number, count: number) => Array.from({ length: count }, (_, i) => row(start + i)));

describe("RowStore", () => {
  it("fetches the chunks a range needs and serves rows from them", async () => {
    const fetch = fetcher();
    const store = new RowStore(fetch);

    await store.ensure({ start: CHUNK - 5, end: CHUNK + 5 });

    expect(fetch.mock.calls).toEqual([
      [0, CHUNK],
      [CHUNK, CHUNK],
    ]);
    expect(store.get(CHUNK + 3)?.subject).toBe(`commit ${CHUNK + 3}`);
    expect(store.get(5 * CHUNK)).toBeUndefined();
    expect(store.version).toBe(2);
  });

  it("never fetches a chunk twice, even while it is in flight", async () => {
    const fetch = fetcher();
    const store = new RowStore(fetch);

    await Promise.all([store.ensure({ start: 0, end: 10 }), store.ensure({ start: 5, end: 20 })]);
    await store.ensure({ start: 0, end: 30 });

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("keeps the error message when a fetch fails", async () => {
    const store = new RowStore(async () => {
      throw "repository 3 is not open";
    });

    await store.ensure({ start: 0, end: 10 });

    expect(store.error).toBe("repository 3 is not open");
    expect(store.get(0)).toBeUndefined();
  });
});
