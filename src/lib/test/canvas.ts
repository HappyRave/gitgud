/** A 2D context that records drawing calls; jsdom has no canvas. */
export interface FakeContext {
  calls: { op: string; args: unknown[]; strokeStyle: unknown; fillStyle: unknown }[];
}

export function fakeContext(): CanvasRenderingContext2D & FakeContext {
  const state: Record<string, unknown> = { strokeStyle: "", fillStyle: "", lineWidth: 1, lineCap: "butt" };
  const calls: FakeContext["calls"] = [];
  return new Proxy(state, {
    get(target, key) {
      if (key === "calls") {
        return calls;
      }
      if (typeof key === "string" && key in target) {
        return target[key];
      }
      return (...args: unknown[]) => {
        calls.push({ op: String(key), args, strokeStyle: target.strokeStyle, fillStyle: target.fillStyle });
      };
    },
    set(target, key, value) {
      target[key as string] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D & FakeContext;
}

/** The fake context a canvas got from `getContext("2d")`, once the test setup installed it. */
export function contextOf(canvas: HTMLCanvasElement): FakeContext {
  return (canvas as unknown as { __fake: FakeContext }).__fake;
}
