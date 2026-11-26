import { LinearFilter, LinearMipmapLinearFilter, Loader, type LoadingManager, Texture } from "three";
import { DDSLoader } from "three/examples/jsm/loaders/DDSLoader.js";

/** The protocol's type for a GPU-compressed texture (`gpu=bc`). */
const DDS = "image/vnd-ms.dds";

/**
 * Makes `texture` the compressed one in `dds`. The texture was handed out before the
 * answer came, so it is turned into what three uploads as compressed data (a
 * CompressedTexture's fields) in place. The backend flipped the image already.
 */
function useDds(texture: Texture, dds: ArrayBuffer) {
  const parsed = new DDSLoader().parse(dds, true);
  Object.assign(texture, {
    isCompressedTexture: true,
    image: { width: parsed.width, height: parsed.height },
    mipmaps: parsed.mipmaps,
    format: parsed.format,
    generateMipmaps: false,
    minFilter: parsed.mipmapCount > 1 ? LinearMipmapLinearFilter : LinearFilter,
  });
}

/**
 * Loads textures like three's `TextureLoader`, but decodes them with `createImageBitmap`,
 * off the main thread: a scene's hundred textures otherwise decode inside `texSubImage2D`
 * and freeze the window for seconds. The bitmap is flipped while decoding (WebGL ignores
 * `flipY` for bitmaps), so UVs match `TextureLoader`'s. A GPU-compressed answer (DDS)
 * is uploaded as is.
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
    const fill = async () => {
      const res = await fetch(resolved);
      if (!res.ok) {
        throw new Error(`${res.status} ${res.statusText} for ${resolved}`);
      }
      if (res.headers.get("content-type") === DDS) {
        useDds(texture, await res.arrayBuffer());
      } else {
        const bitmap = await createImageBitmap(await res.blob(), {
          imageOrientation: "flipY",
          premultiplyAlpha: "none",
          colorSpaceConversion: "none",
        });
        // Not closed on dispose: a viewer still drawing a texture the cache freed
        // re-uploads it from the bitmap. Garbage collection frees it.
        texture.image = bitmap;
      }
      texture.needsUpdate = true;
    };
    fill().then(
      () => {
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
