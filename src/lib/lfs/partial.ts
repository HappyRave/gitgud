import type { LfsPlan, PartialState } from "$lib/api";

/** Bytes a plan keeps on disk: what the scenes use, what Unity always loads, what was kept. */
export function wantedBytes(plan: LfsPlan): number {
  return plan.used.bytes + plan.always.bytes + plan.kept.bytes;
}

/** What applying `plan` does to the disk: download the missing part, or free what goes. */
export function applyEffect(plan: LfsPlan): { kind: "download" | "free" | "none"; bytes: number } {
  const delta = wantedBytes(plan) - plan.onDisk.bytes;
  if (delta > 0) {
    return { kind: "download", bytes: delta };
  }
  if (delta < 0) {
    return { kind: "free", bytes: -delta };
  }
  return { kind: "none", bytes: 0 };
}

/** Bar segment widths in percent: used, always, kept, not downloaded. */
export function shares(plan: LfsPlan): [number, number, number, number] {
  const total = plan.total.bytes || 1;
  const pct = (b: number) => (100 * b) / total;
  return [pct(plan.used.bytes), pct(plan.always.bytes), pct(plan.kept.bytes), pct(plan.skipped.bytes)];
}

export function sameState(a: PartialState, b: PartialState): boolean {
  return (
    a.buildScenes === b.buildScenes &&
    a.scenes.length === b.scenes.length &&
    a.scenes.every((s, i) => s === b.scenes[i]) &&
    a.kept.length === b.kept.length &&
    a.kept.every((k, i) => k === b.kept[i])
  );
}

/** `state` with `path` added to its scenes, once. */
export function withScene(state: PartialState, path: string): PartialState {
  const p = path.trim();
  return !p || state.scenes.includes(p) ? state : { ...state, scenes: [...state.scenes, p] };
}

export function withoutScene(state: PartialState, path: string): PartialState {
  return { ...state, scenes: state.scenes.filter((s) => s !== path) };
}

export function withoutKept(state: PartialState, path: string): PartialState {
  return { ...state, kept: state.kept.filter((k) => k !== path) };
}
