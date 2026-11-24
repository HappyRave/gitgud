import type { FileVersion, ModelKind } from "$lib/api";
import { type ImageAt, protocolUrl } from "$lib/images";

const MODEL = /\.(fbx|obj|gltf|glb|prefab)$/i;

export const isModel = (path: string) => MODEL.test(path);

export function modelKind(path: string): ModelKind | null {
  const m = MODEL.exec(path);
  return m ? (m[1].toLowerCase() as ModelKind) : null;
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

function query(repoId: number, path: string, at: ImageAt, bust: number) {
  const q = new URLSearchParams({ repo: String(repoId), path, at: at.at });
  if ("commit" in at) {
    q.set("commit", at.commit);
  }
  q.set("v", String(bust));
  return q;
}

/** The model file `path` at a version, on the `gitgud` protocol. */
export function modelUrl(repoId: number, path: string, at: ImageAt, bust = 0, windows?: boolean): string {
  return protocolUrl("model", query(repoId, path, at, bust), windows);
}

/** The file model `near` refers to as `ref` (a texture name, a glTF buffer). */
export function refUrl(repoId: number, near: string, ref: string, at: ImageAt, bust = 0, windows?: boolean): string {
  const q = query(repoId, near, at, bust);
  q.set("ref", ref);
  return protocolUrl("ref", q, windows);
}
