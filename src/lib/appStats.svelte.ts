import { appStats, type AppStats } from "$lib/api";

export const INTERVAL_MS = 2000;
/** 20 samples at 2 s: the trace covers the last 40 seconds. */
export const HISTORY = 20;

/** Polls `app_stats` while anything shows it and the window is visible; keeps the CPU history. */
class StatsFeed {
  latest = $state<AppStats | null>(null);
  history = $state<number[]>([]);
  error = $state<string | null>(null);
  private users = 0;
  private timer: ReturnType<typeof setInterval> | null = null;

  /** Starts polling for one viewer; returns the function that stops it. */
  subscribe(): () => void {
    this.users += 1;
    if (this.timer === null) {
      void this.tick();
      this.timer = setInterval(() => void this.tick(), INTERVAL_MS);
    }
    return () => {
      this.users -= 1;
      if (this.users === 0 && this.timer !== null) {
        clearInterval(this.timer);
        this.timer = null;
      }
    };
  }

  async tick() {
    if (document.visibilityState === "hidden") {
      return;
    }
    try {
      const s = await appStats();
      this.latest = s;
      this.history = [...this.history.slice(-(HISTORY - 1)), s.cpu];
      this.error = null;
    } catch (e) {
      this.latest = null;
      this.history = [];
      this.error = String(e);
    }
  }
}

export const statsFeed = new StatsFeed();

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** "0 B", "640 KB", "812 MB", "1.3 GB": three significant digits at most, 1024-based. */
export function formatBytes(bytes: number): string {
  let v = bytes;
  let u = 0;
  while (v >= 1000 && u < UNITS.length - 1) {
    v /= 1024;
    u += 1;
  }
  const digits = u === 0 || v >= 10 ? 0 : 1;
  return `${v.toFixed(digits)} ${UNITS[u]}`;
}

/** The colour step of a CPU share: amber above 50%, orange above 85%. */
export function cpuLevel(cpu: number): "" | "hot" | "burn" {
  return cpu > 85 ? "burn" : cpu > 50 ? "hot" : "";
}

/** Polyline points for `history` (0–100) right-aligned in a `w`×`h` box, newest at the right. */
export function tracePoints(history: number[], w: number, h: number): string {
  const step = w / (HISTORY - 1);
  const start = w - (history.length - 1) * step;
  return history
    .map((v, i) => `${(start + i * step).toFixed(1)},${(h - 1 - (Math.min(100, Math.max(0, v)) / 100) * (h - 2)).toFixed(1)}`)
    .join(" ");
}
