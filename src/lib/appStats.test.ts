import { afterEach, describe, expect, it, vi } from "vitest";
import { HISTORY, INTERVAL_MS, cpuLevel, formatBytes, statsFeed, tracePoints } from "$lib/appStats.svelte";
import { mockCommands } from "$lib/test/ipc";

describe("formatBytes", () => {
  it("keeps three significant digits at most", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(640 * 1024)).toBe("640 KB");
    expect(formatBytes(812 * 1024 * 1024)).toBe("812 MB");
    expect(formatBytes(1.3 * 1024 ** 3)).toBe("1.3 GB");
    expect(formatBytes(14.2 * 1024 * 1024)).toBe("14 MB");
    expect(formatBytes(1000 * 1024)).toBe("1.0 MB");
  });
});

describe("cpuLevel", () => {
  it("warms up above 50% and 85%", () => {
    expect([cpuLevel(2), cpuLevel(50), cpuLevel(51), cpuLevel(85), cpuLevel(86)]).toEqual(["", "", "hot", "hot", "burn"]);
  });
});

describe("tracePoints", () => {
  it("puts the newest sample at the right edge and clamps to the box", () => {
    expect(tracePoints([0, 100], 38, 12)).toBe("36.0,11.0 38.0,1.0");
    const full = tracePoints(Array.from({ length: HISTORY }, () => 150), 38, 12).split(" ");
    expect(full[0]).toBe("0.0,1.0");
    expect(full.at(-1)).toBe("38.0,1.0");
  });
});

describe("statsFeed", () => {
  afterEach(() => vi.useRealTimers());

  it("polls while subscribed, keeps the last 40 s, and stops when nobody watches", async () => {
    vi.useFakeTimers();
    let cpu = 0;
    const calls = mockCommands({ app_stats: () => ({ cpu: ++cpu, appBytes: 1, viewBytes: 2, gitBytes: 3 }) });
    const stopA = statsFeed.subscribe();
    const stopB = statsFeed.subscribe();
    await vi.advanceTimersByTimeAsync(INTERVAL_MS * (HISTORY + 4));
    expect(statsFeed.history).toHaveLength(HISTORY);
    expect(statsFeed.history.at(-1)).toBe(cpu);
    expect(statsFeed.latest?.viewBytes).toBe(2);

    stopA();
    await vi.advanceTimersByTimeAsync(INTERVAL_MS);
    const before = calls.length;
    stopB();
    await vi.advanceTimersByTimeAsync(INTERVAL_MS * 5);
    expect(calls.length).toBe(before);
  });

  it("skips samples while the window is hidden and reports a failure", async () => {
    mockCommands({
      app_stats: () => {
        throw "cannot read processes";
      },
    });
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    await statsFeed.tick();
    expect(statsFeed.error).toBeNull();
    visibility.mockReturnValue("visible");
    await statsFeed.tick();
    expect(statsFeed.error).toBe("cannot read processes");
    expect([statsFeed.latest, statsFeed.history]).toEqual([null, []]);
    visibility.mockRestore();
  });
});
