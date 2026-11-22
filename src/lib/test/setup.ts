import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vitest";
import { fakeContext } from "./canvas";

HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement & { __fake?: unknown }) {
  this.__fake ??= fakeContext();
  return this.__fake;
} as typeof HTMLCanvasElement.prototype.getContext;

// Unmount first: components unsubscribe from mocked events on destroy.
afterEach(() => {
  cleanup();
  clearMocks();
});
