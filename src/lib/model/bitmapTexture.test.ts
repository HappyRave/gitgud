import { LinearMipmapLinearFilter, LoadingManager, RGB_S3TC_DXT1_Format, type Texture } from "three";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BitmapTextureLoader } from "./bitmapTexture";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function stub(ok: boolean, type = "image/png", body: ArrayBuffer = new ArrayBuffer(3)) {
  const bitmap = { width: 4, height: 2, close: vi.fn() };
  const decode = vi.fn(async () => bitmap);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({
      ok,
      status: ok ? 200 : 404,
      statusText: ok ? "OK" : "Not Found",
      headers: { get: (h: string) => (h === "content-type" ? type : null) },
      blob: async () => new Blob([body]),
      arrayBuffer: async () => body,
    })),
  );
  vi.stubGlobal("createImageBitmap", decode);
  return { bitmap, decode };
}

/** An 8×8 BC1 DDS with its 4 mip levels, laid out as the backend writes it. */
function dds(): ArrayBuffer {
  const h = new Uint32Array(32);
  const four = (s: string) => s.charCodeAt(0) | (s.charCodeAt(1) << 8) | (s.charCodeAt(2) << 16) | (s.charCodeAt(3) << 24);
  h[0] = four("DDS ");
  h[1] = 124;
  h[2] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000;
  h[3] = 8;
  h[4] = 8;
  h[5] = 32;
  h[7] = 4;
  h[19] = 32;
  h[20] = 0x4;
  h[21] = four("DXT1");
  h[27] = 0x1000 | 0x400000 | 0x8;
  const out = new Uint8Array(128 + 32 + 8 + 8 + 8);
  out.set(new Uint8Array(h.buffer));
  return out.buffer;
}

describe("BitmapTextureLoader", () => {
  it("decodes off the main thread, flipped while decoding, and keeps the bitmap past dispose", async () => {
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
    expect(bitmap.close, "a viewer still drawing it re-uploads it").not.toHaveBeenCalled();
    expect(texture.image).toBe(bitmap);
  });

  it("uploads a GPU-compressed answer as is, with its mip levels", async () => {
    const { decode } = stub(true, "image/vnd-ms.dds", dds());
    const manager = new LoadingManager();
    const done = new Promise<void>((resolve) => (manager.onLoad = resolve));
    const texture = new BitmapTextureLoader(manager).load("http://gitgud.localhost/image?path=a.png&max=1024&gpu=bc");
    await done;
    expect(decode).not.toHaveBeenCalled();
    const t = texture as Texture & { isCompressedTexture?: boolean; mipmaps: { width: number }[] };
    expect(t.isCompressedTexture).toBe(true);
    expect(t.format).toBe(RGB_S3TC_DXT1_Format);
    expect(t.mipmaps.map((m) => m.width)).toEqual([8, 4, 2, 1]);
    expect(t.image).toEqual({ width: 8, height: 8 });
    expect([t.generateMipmaps, t.flipY, t.minFilter]).toEqual([false, false, LinearMipmapLinearFilter]);
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
