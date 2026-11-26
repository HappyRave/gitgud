import type { FileVersion, ModelKind } from "$lib/api";
import { type ImageAt, protocolUrl, versionQuery } from "$lib/images";

const MODEL = /\.(fbx|obj|gltf|glb|prefab|unity)$/i;

export const isModel = (path: string) => MODEL.test(path);

/** A scene is assembled like a prefab. */
export function modelKind(path: string): ModelKind | null {
  const m = MODEL.exec(path);
  if (!m) {
    return null;
  }
  const ext = m[1].toLowerCase();
  return (ext === "unity" ? "prefab" : ext) as ModelKind;
}

/** The kind of a file a prefab draws a mesh from; Unity `Mesh` assets included. */
export function meshFileKind(path: string): ModelKind | null {
  return /\.asset$/i.test(path) ? "asset" : modelKind(path);
}

export function toVersion(at: ImageAt): FileVersion {
  if ("commit" in at) {
    return at.at === "commit" ? { commit: at.commit } : { parent: at.commit };
  }
  return at.at;
}

/** The model file `path` at a version, on the `gitgud` protocol. */
export function modelUrl(repoId: number, path: string, at: ImageAt, bust = 0, windows?: boolean): string {
  return protocolUrl("model", versionQuery(repoId, path, at, bust), windows);
}

/** The file model `near` refers to as `ref` (a texture name, a glTF buffer). */
export function refUrl(repoId: number, near: string, ref: string, at: ImageAt, bust = 0, windows?: boolean): string {
  const q = versionQuery(repoId, near, at, bust);
  q.set("ref", ref);
  return protocolUrl("ref", q, windows);
}
