import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vitest";
import { fakeContext } from "./canvas";

HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement & { __fake?: unknown }) {
  this.__fake ??= fakeContext();
  return this.__fake;
} as typeof HTMLCanvasElement.prototype.getContext;

afterEach(() => clearMocks());
