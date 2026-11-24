import type { Sprite, SpriteSheet } from "$lib/api";

export type SpriteState = "same" | "added" | "removed" | "changed";

export type SpriteLayer = "slices" | "pixels" | "pivots";
export const SPRITE_LAYERS: readonly SpriteLayer[] = ["slices", "pixels", "pivots"];

/** A sprite's box in image space (origin top-left); a Single sprite covers the texture. */
export function spriteBox(s: Sprite, texW: number, texH: number) {
  const [x, y, w, h] = s.rect;
  if (w === 0 && h === 0) {
    return { left: 0, top: 0, width: texW, height: texH };
  }
  return { left: x, top: texH - y - h, width: w, height: h };
}

/** Each sprite name's change on the given side. */
export function spriteStates(sheet: SpriteSheet): Map<string, SpriteState> {
  const out = new Map<string, SpriteState>();
  for (const c of sheet.changes) {
    out.set(c.name, c.kinds.includes("added") ? "added" : c.kinds.includes("removed") ? "removed" : "changed");
  }
  return out;
}

/** "12 → 13 slices · +1 icon_fuel · 2 moved · icon_anchor resized 32×32 → 40×32". */
export function spriteCaption(sheet: SpriteSheet): { text: string; tone: "" | "add" | "del" | "mod" }[] {
  if (sheet.mode === "none") {
    return [{ text: "not imported as sprites", tone: "" }];
  }
  if (sheet.mode === "single") {
    return [{ text: "single sprite", tone: "" }, ...sheet.changes.map((c) => ({ text: c.note || c.kinds.join(", "), tone: "mod" as const }))];
  }
  const n = sheet.after.length;
  const b = sheet.before?.length;
  const out: { text: string; tone: "" | "add" | "del" | "mod" }[] = [
    { text: b !== undefined && b !== n ? `${b} → ${n} slices` : `${n} slices`, tone: "" },
  ];
  const of = (k: string) => sheet.changes.filter((c) => c.kinds.includes(k as never));
  const names = (list: { name: string }[]) => list.slice(0, 3).map((c) => c.name).join(", ") + (list.length > 3 ? "…" : "");
  const added = of("added");
  const removed = of("removed");
  if (added.length) {
    out.push({ text: `+${added.length} ${names(added)}`, tone: "add" });
  }
  if (removed.length) {
    out.push({ text: `−${removed.length} ${names(removed)}`, tone: "del" });
  }
  for (const k of ["moved", "resized", "pivot", "border"] as const) {
    const list = of(k);
    if (list.length === 1) {
      out.push({ text: `${list[0].name} ${list[0].note.split(" · ").find((p) => p.startsWith(k === "border" ? "9-slice" : k)) ?? k}`, tone: "mod" });
    } else if (list.length > 1) {
      out.push({ text: `${list.length} ${k === "pivot" ? "pivots moved" : k === "border" ? "borders changed" : k}`, tone: "mod" });
    }
  }
  if (sheet.before && sheet.changes.length === 0) {
    out.push({ text: "slicing unchanged", tone: "" });
  }
  return out;
}
