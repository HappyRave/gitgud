import type { ModelMaterial } from "$lib/api";

/** A custom shader's colour-like texture properties a base map can come from, and the one picked without asking. */
export interface BaseSlots {
  auto: string | null;
  slots: string[];
}

/** The custom shaders a model draws through an approximation, with their base map choices. */
export class Approximations {
  private readonly seen = new Map<string, BaseSlots>();

  note(desc: ModelMaterial) {
    if (!desc.approximated) {
      return;
    }
    const entry = this.seen.get(desc.shader) ?? { auto: desc.baseProperty, slots: [] };
    for (const p of desc.baseCandidates) {
      if (!entry.slots.includes(p)) {
        entry.slots.push(p);
      }
    }
    this.seen.set(desc.shader, entry);
  }

  get shaders(): string[] {
    return [...this.seen.keys()];
  }

  get bases(): Record<string, BaseSlots> {
    return Object.fromEntries(this.seen);
  }
}

/** Both sides' choices per shader, properties merged in order. */
export function mergeBases(...sides: (Record<string, BaseSlots> | undefined)[]): Record<string, BaseSlots> {
  const out: Record<string, BaseSlots> = {};
  for (const side of sides) {
    for (const [shader, b] of Object.entries(side ?? {})) {
      const into = (out[shader] ??= { auto: b.auto, slots: [] });
      into.auto ??= b.auto;
      for (const s of b.slots) {
        if (!into.slots.includes(s)) {
          into.slots.push(s);
        }
      }
    }
  }
  return out;
}
