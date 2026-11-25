import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.restoreAllMocks();
  vi.resetModules();
});

function gpu(extensions: string[]) {
  const loseContext = vi.fn();
  const gl = {
    getExtension: (name: string) => (name === "WEBGL_lose_context" ? { loseContext } : extensions.includes(name) ? {} : null),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(gl as never);
  return loseContext;
}

describe("supportsBc", () => {
  it("needs the linear and the sRGB BC formats, asks once, and lets the probe context go", async () => {
    const loseContext = gpu(["WEBGL_compressed_texture_s3tc", "WEBGL_compressed_texture_s3tc_srgb"]);
    const { supportsBc } = await import("./gpuFormats");
    expect(supportsBc()).toBe(true);
    expect(supportsBc()).toBe(true);
    expect(HTMLCanvasElement.prototype.getContext).toHaveBeenCalledTimes(1);
    expect(loseContext).toHaveBeenCalled();
  });

  it("is off without the sRGB formats, or without WebGL 2", async () => {
    gpu(["WEBGL_compressed_texture_s3tc"]);
    expect((await import("./gpuFormats")).supportsBc()).toBe(false);
    vi.resetModules();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    expect((await import("./gpuFormats")).supportsBc()).toBe(false);
  });
});
