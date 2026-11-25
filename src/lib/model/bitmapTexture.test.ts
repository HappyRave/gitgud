import { LoadingManager } from "three";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BitmapTextureLoader } from "./bitmapTexture";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function stub(ok: boolean) {
  const bitmap = { width: 4, height: 2, close: vi.fn() };
  const decode = vi.fn(async () => bitmap);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ ok, status: ok ? 200 : 404, statusText: ok ? "OK" : "Not Found", blob: async () => new Blob(["png"]) })),
  );
  vi.stubGlobal("createImageBitmap", decode);
  return { bitmap, decode };
}

describe("BitmapTextureLoader", () => {
  it("decodes off the main thread, flipped while decoding, and closes the bitmap on dispose", async () => {
    const { bitmap, decode } = stub(true);
    const manager = new LoadingManager();
    const done = new Promise<void>((resolve) => (manager.onLoad = resolve));
    const loaded = vi.fn();
    const texture = new BitmapTextureLoader(manager).load("http://gitgud.localhost/image?path=a.png", loaded);
    expect(texture.flipY).toBe(false);
    const version = texture.version;
    await done;
    expect(decode).toHaveBeenCalledWith(expect.any(Blob), expect.objectContaining({ imageOrientation: "flipY" }));
    expect(texture.image).toBe(bitmap);
    expect(texture.version).toBeGreaterThan(version);
    expect(loaded).toHaveBeenCalledWith(texture);
    texture.dispose();
    expect(bitmap.close).toHaveBeenCalled();
  });

  it("reports a texture that can't be fetched", async () => {
    stub(false);
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const manager = new LoadingManager();
    const failed = vi.fn();
    manager.onError = failed;
    const done = new Promise<void>((resolve) => (manager.onLoad = resolve));
    const onError = vi.fn();
    new BitmapTextureLoader(manager).load("http://gitgud.localhost/image?path=gone.png", undefined, undefined, onError);
    await done;
    expect(onError).toHaveBeenCalled();
    expect(failed).toHaveBeenCalledWith("http://gitgud.localhost/image?path=gone.png");
    expect(errors).toHaveBeenCalled();
  });
});
