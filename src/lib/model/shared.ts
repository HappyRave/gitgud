import type { ModelMaterial } from "$lib/api";
import { type ModelCache, modelCache } from "./cache";
import { type LoadedModel, type LoadSource, loadModel } from "./load";
import { withBase } from "./materials";
import { prefs } from "$lib/prefs.svelte";

/**
 * Loads a model file through `cache` when the backend named its content
 * (`info.key`), so the same file at the same content is read and parsed once.
 * The result is shared: clone what goes into a scene.
 */
export function loadShared(src: LoadSource, cache: ModelCache = modelCache): Promise<LoadedModel> {
  if (!src.info?.key) {
    return loadModel(src);
  }
  const key = `${src.kind}|${src.geometryOnly ? "geo" : "full"}|${prefs.textureMax}|${JSON.stringify({ ...src.info, materials: src.info.materials.map(withBase) })}`;
  return cache.model(key, () => loadModel(src)).then((m) => {
    src.onProgress?.({ phase: "done" });
    return m;
  });
}

/** A key for the material `desc` describes, when every texture it uses is named by content. */
export function materialKey(given: ModelMaterial): string | null {
  const desc = withBase(given);
  if ((desc.baseMap && !desc.baseMapKey) || (desc.normalMap && !desc.normalMapKey)) {
    return null;
  }
  return `mat|${prefs.textureMax}|${JSON.stringify(desc)}`;
}
