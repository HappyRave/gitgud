import { modelInfo } from "$lib/api";
import { type ImageAt, imageUrl } from "$lib/images";
import { clone as cloneSkinned } from "three/examples/jsm/utils/SkeletonUtils.js";
import { loadShared } from "./shared";
import type { LoadedModel } from "./load";
import { assemblePrefab } from "./prefab";
import { LoadProgress } from "./progress";
import { meshFileKind, modelUrl, refUrl, toVersion } from "./source";

/** One version of a model, as the viewer is given it. */
export interface ModelSide {
  label: string;
  at: ImageAt;
}

export type Opened = { model: LoadedModel; error: null } | { model: null; error: string };

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const fileName = (p: string) => p.slice(p.lastIndexOf("/") + 1);

/** The LFS object isn't on this machine, so the LFS card should offer to fetch it. */
export const needsLfsFetch = (error: string | null) => !!error && /isn't downloaded/.test(error);

/**
 * Loads `path` at `side`'s version with its Unity materials, reporting each
 * file's progress under `side.label`; never throws.
 */
export async function openSide(
  repoId: number,
  path: string,
  side: ModelSide,
  bust: number,
  progress: LoadProgress = new LoadProgress(),
): Promise<Opened> {
  const kind = meshFileKind(path);
  if (!kind) {
    return { model: null, error: `${path} is not a 3D model` };
  }
  const key = (what: string) => `${side.label}:${what}`;
  try {
    const version = toVersion(side.at);
    progress.report(key("info"), { phase: "info" });
    const info = await modelInfo(repoId, path, version);
    progress.report(key("info"), { phase: "done" });
    if (info.scene) {
      const model = await assemblePrefab(info.scene, info.materials, {
        modelUrl: (p) => modelUrl(repoId, p, side.at, bust),
        meshUrl: (p, id) => `${modelUrl(repoId, p, side.at, bust)}&mesh=${encodeURIComponent(id)}`,
        refUrl: (near) => (ref) => refUrl(repoId, near, ref, side.at, bust),
        imageUrl: (p) => imageUrl(repoId, p, side.at, bust),
        info: (p) => modelInfo(repoId, p, version),
        progress: (p) => progress.reporter(key(p)),
      });
      return { model: { ...model, fromDisk: info.fromDisk }, error: null };
    }
    const shared = await loadShared({
      kind,
      url: modelUrl(repoId, path, side.at, bust),
      refUrl: (ref) => refUrl(repoId, path, ref, side.at, bust),
      imageUrl: (p) => imageUrl(repoId, p, side.at, bust),
      info,
      name: fileName(path),
      onProgress: progress.reporter(key(path)),
    });
    return { model: { ...shared, root: cloneSkinned(shared.root), fromDisk: info.fromDisk }, error: null };
  } catch (e) {
    progress.report(key(path), { phase: "done" });
    progress.report(key("info"), { phase: "done" });
    return { model: null, error: message(e) };
  }
}
