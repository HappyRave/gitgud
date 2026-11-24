import { type ImageAt, isImage, protocolUrl } from "$lib/images";
import { isUnityYaml } from "$lib/unity";

/** The visual viewers gitgud has beside the text diff (R7). */
export type ViewerKind = "model" | "material" | "font" | "audio" | "video" | "pdf" | "table" | "atlas" | "curves" | "scene" | "asset";

/** Visual (the viewer), Structure (Unity's structural diff) or Source (the text diff). */
export type ViewMode = "visual" | "structure" | "source";

/** File extension → viewer. A format without an entry uses the text or structural diff. */
const BY_EXT: Record<string, ViewerKind> = {
  prefab: "model",
  fbx: "model",
  obj: "model",
  gltf: "model",
  glb: "model",
  asset: "asset",
};

const BINARY = new Set<ViewerKind>(["font", "audio", "video", "pdf"]);
const BINARY_MODELS = /\.(fbx|glb)$/i;

const ext = (path: string) => path.slice(path.lastIndexOf(".") + 1).toLowerCase();

/** For `X.meta`, the asset `X` it describes; otherwise null. */
export function metaTarget(path: string): string | null {
  return /\.meta$/i.test(path) ? path.slice(0, -5) : null;
}

/**
 * The viewer for `path`, or null when it has none. A `.meta` file opens its asset's
 * viewer (the import settings changed). `.asset` needs the file's first object to
 * pick between the grid and 3D, so it comes back as "asset".
 */
export function viewerFor(path: string): ViewerKind | null {
  const target = metaTarget(path);
  if (target) {
    return isImage(target) ? null : viewerFor(target);
  }
  return BY_EXT[ext(path)] ?? null;
}

/** The views `path` offers, in switch order; one view means no switch is shown. */
export function viewModes(path: string): ViewMode[] {
  const visual = viewerFor(path) !== null;
  const target = metaTarget(path);
  if (target) {
    return visual ? ["visual", "source"] : ["source"];
  }
  if (isUnityYaml(path)) {
    return visual ? ["visual", "structure", "source"] : ["structure", "source"];
  }
  const kind = BY_EXT[ext(path)];
  if (visual && kind && (BINARY.has(kind) || BINARY_MODELS.test(path))) {
    return ["visual"];
  }
  return visual ? ["visual", "source"] : ["source"];
}

/** The key a remembered view is stored under: one per format (R7: per format). */
export const formatKey = (path: string) => ext(metaTarget(path) ?? path) + (metaTarget(path) ? ".meta" : "");

/** Any file at a version as stored (LFS resolved), on the `gitgud` protocol. */
export function fileUrl(repoId: number, path: string, at: ImageAt, bust = 0, windows?: boolean): string {
  const q = new URLSearchParams({ repo: String(repoId), path, at: at.at });
  if ("commit" in at) {
    q.set("commit", at.commit);
  }
  q.set("v", String(bust));
  return protocolUrl("file", q, windows);
}
