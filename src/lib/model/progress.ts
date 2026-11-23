import { formatSize } from "$lib/merge/resolve";

/** Where one file of a model load stands. */
export type Step =
  | { phase: "info" }
  | { phase: "request"; name: string }
  | { phase: "download"; loaded: number; total: number }
  | { phase: "files"; loaded: number; total: number }
  | { phase: "parse" }
  | { phase: "done" };

export interface Summary {
  label: string;
  /** 0–1, or `null` while the step can't be measured. */
  value: number | null;
}

interface Item {
  step: Step;
  bytes: [number, number];
}

/**
 * Folds the steps of every file a model load touches (both versions, every
 * model a prefab uses) into one label and one bar.
 */
export class LoadProgress {
  onchange: (s: Summary) => void = () => {};
  private items = new Map<string, Item>();

  report(key: string, step: Step) {
    const prev = this.items.get(key);
    const bytes: [number, number] = step.phase === "download" ? [step.loaded, step.total] : (prev?.bytes ?? [0, 0]);
    this.items.set(key, { step, bytes });
    this.onchange(this.summary());
  }

  /** Reports `fn`'s steps under `key`. */
  reporter(key: string) {
    return (step: Step) => this.report(key, step);
  }

  summary(): Summary {
    const active = [...this.items.values()].filter((i) => i.step.phase !== "done");
    const by = (p: Step["phase"]) => active.filter((i) => i.step.phase === p);
    const downloading = by("download");
    if (downloading.length > 0) {
      const sized = [...this.items.values()].filter((i) => i.bytes[1] > 0);
      const loaded = sized.reduce((n, i) => n + i.bytes[0], 0);
      const total = sized.reduce((n, i) => n + i.bytes[1], 0);
      const unsized = downloading.some((i) => i.bytes[1] === 0);
      return unsized || total === 0
        ? { label: `Downloading ${formatSize(downloading.reduce((n, i) => n + i.bytes[0], 0))}…`, value: null }
        : { label: `Downloading ${formatSize(loaded)} / ${formatSize(total)}`, value: loaded / total };
    }
    const waiting = by("request");
    if (waiting.length > 0) {
      const first = waiting[0].step as { name: string };
      const more = waiting.length > 1 ? ` and ${waiting.length - 1} more` : "";
      return { label: `Waiting for ${first.name}${more}…`, value: null };
    }
    if (by("info").length > 0) {
      return { label: "Reading Unity references…", value: null };
    }
    const files = by("files");
    if (files.length > 0) {
      let loaded = 0;
      let total = 0;
      for (const i of files) {
        const s = i.step as { loaded: number; total: number };
        loaded += s.loaded;
        total += s.total;
      }
      return { label: `Loading textures ${loaded} / ${total}`, value: total ? loaded / total : null };
    }
    return { label: "Opening in three.js…", value: null };
  }
}
