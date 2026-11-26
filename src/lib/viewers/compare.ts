export interface SwipeDrag {
  /** The element the split is measured across; by default the handle's swipe line's parent. */
  host?: () => HTMLElement | null | undefined;
  set: (split: number) => void;
  /** Closest the split gets to either edge, as a share of the host's width. */
  margin?: number;
}

/** Drags a swipe handle: the split follows the pointer across `host` until it is let go. */
export function swipeDrag(handle: HTMLElement, opts: SwipeDrag) {
  let current = opts;
  const move = (e: PointerEvent) => {
    const host = current.host ? current.host() : handle.parentElement?.parentElement;
    if (!host) {
      return;
    }
    const r = host.getBoundingClientRect();
    const m = current.margin ?? 0.02;
    current.set(Math.min(1 - m, Math.max(m, (e.clientX - r.left) / r.width)));
  };
  const up = () => handle.removeEventListener("pointermove", move);
  const down = (e: PointerEvent) => {
    handle.setPointerCapture(e.pointerId);
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up, { once: true });
  };
  handle.addEventListener("pointerdown", down);
  return {
    update(next: SwipeDrag) {
      current = next;
    },
    destroy() {
      handle.removeEventListener("pointerdown", down);
      up();
    },
  };
}

/** How two versions of a 2D view are shown. */
export type FlatMode = "side" | "swipe" | "onion";

export const FLAT_MODES: [FlatMode, string][] = [
  ["side", "Side by side"],
  ["swipe", "Swipe"],
  ["onion", "Onion"],
];
