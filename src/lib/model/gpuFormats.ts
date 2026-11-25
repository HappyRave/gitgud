let bc: boolean | null = null;

/**
 * Whether the GPU takes BC1/BC3 (DXT) textures, linear and sRGB, so 3D previews can
 * ask the backend for compressed ones: no decoding or mip building on the main thread.
 */
export function supportsBc(): boolean {
  if (bc === null) {
    const gl = typeof document === "undefined" ? null : document.createElement("canvas").getContext("webgl2");
    if (typeof gl?.getExtension !== "function") {
      bc = false;
      return bc;
    }
    bc = !!gl.getExtension("WEBGL_compressed_texture_s3tc") && !!gl.getExtension("WEBGL_compressed_texture_s3tc_srgb");
    gl.getExtension("WEBGL_lose_context")?.loseContext();
  }
  return bc;
}
