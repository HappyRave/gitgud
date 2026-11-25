import { Loader, type LoadingManager, Texture } from "three";

/**
 * Loads textures like three's `TextureLoader`, but decodes them with `createImageBitmap`,
 * off the main thread: a scene's hundred textures otherwise decode inside `texSubImage2D`
 * and freeze the window for seconds. The bitmap is flipped while decoding (WebGL ignores
 * `flipY` for bitmaps), so UVs match `TextureLoader`'s.
 */
export class BitmapTextureLoader extends Loader<Texture> {
  constructor(manager?: LoadingManager) {
    super(manager);
  }

  override load(url: string, onLoad?: (t: Texture) => void, _onProgress?: unknown, onError?: (e: unknown) => void): Texture {
    const texture = new Texture();
    texture.flipY = false;
    const resolved = this.manager.resolveURL(url);
    this.manager.itemStart(resolved);
    fetch(resolved)
      .then((res) => {
        if (!res.ok) {
          throw new Error(`${res.status} ${res.statusText} for ${resolved}`);
        }
        return res.blob();
      })
      .then((blob) => createImageBitmap(blob, { imageOrientation: "flipY", premultiplyAlpha: "none", colorSpaceConversion: "none" }))
      .then(
        (bitmap) => {
          texture.image = bitmap;
          texture.needsUpdate = true;
          texture.addEventListener("dispose", () => bitmap.close());
          onLoad?.(texture);
          this.manager.itemEnd(resolved);
        },
        (e) => {
          console.error(`Texture failed to load: ${resolved}`, e);
          onError?.(e);
          this.manager.itemError(resolved);
          this.manager.itemEnd(resolved);
        },
      );
    return texture;
  }
}
