import { type FileVersion, type ModelInfo, modelInfo } from "$lib/api";

/** Scene infos asked for ahead of opening; a big scene's is megabytes, so few wait. */
const KEEP = 4;
const warmed = new Map<string, Promise<ModelInfo>>();

const keyOf = (repoId: number, path: string, version: FileVersion) => `${repoId}\n${path}\n${JSON.stringify(version)}`;

/** Starts `model_info` for a scene about to be opened (its "Open in 3D" card shows). */
export function warmSceneInfo(repoId: number, path: string, version: FileVersion): Promise<ModelInfo> {
  const key = keyOf(repoId, path, version);
  let info = warmed.get(key);
  if (!info) {
    info = modelInfo(repoId, path, version);
    info.catch(() => warmed.delete(key));
    warmed.set(key, info);
    if (warmed.size > KEEP) {
      warmed.delete(warmed.keys().next().value!);
    }
  }
  return info;
}

/**
 * `model_info` for opening: the warmed one when there is one (taken, so a later
 * opening asks again and never sees a stale work tree), else asked now.
 */
export function sceneInfo(repoId: number, path: string, version: FileVersion): Promise<ModelInfo> {
  const key = keyOf(repoId, path, version);
  const info = warmed.get(key);
  warmed.delete(key);
  return info ?? modelInfo(repoId, path, version);
}
