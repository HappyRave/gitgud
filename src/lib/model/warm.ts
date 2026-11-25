import { warmTextures } from "$lib/api";
import { prefs } from "$lib/prefs.svelte";
import { supportsBc } from "./gpuFormats";
import type { ModelSide } from "./open";
import { warmSceneInfo } from "./sceneInfo";
import { toVersion } from "./source";

/**
 * Gets a big scene or prefab ready while its "Open in 3D" card shows: each side's
 * scene info, then the textures its materials use, prepared as the 3D view will ask.
 */
export function warmScene(repoId: number, path: string, sides: ModelSide[]) {
  for (const side of sides) {
    const version = toVersion(side.at);
    warmSceneInfo(repoId, path, version).then(
      (info) => {
        const paths = [...new Set(info.materials.flatMap((m) => [m.baseMap, m.normalMap]).filter((p): p is string => !!p))];
        if (paths.length > 0 && prefs.textureMax > 0) {
          warmTextures(repoId, version, paths, prefs.textureMax, supportsBc()).catch((e) => console.error("Texture warm-up failed", e));
        }
      },
      // Opening asks again and shows why.
      () => {},
    );
  }
}
