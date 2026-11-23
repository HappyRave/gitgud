import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vitest";
import { fakeContext } from "./canvas";

HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement & { __fake?: unknown }) {
  this.__fake ??= fakeContext();
  return this.__fake;
} as typeof HTMLCanvasElement.prototype.getContext;

// jsdom has no layout observers; the 3D viewer only needs them to exist.
class NoopObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= NoopObserver as unknown as typeof ResizeObserver;
globalThis.IntersectionObserver ??= NoopObserver as unknown as typeof IntersectionObserver;

// Unmount first: components unsubscribe from mocked events on destroy.
afterEach(() => {
  cleanup();
  clearMocks();
});
