import { formatSize } from "$lib/merge/resolve";

/** Versions the `gitgud://` image protocol serves (see `src-tauri/src/images.rs`). */
export type ImageAt =
  | { at: "head" | "index" | "worktree" | "yours" | "theirs" }
  | { at: "commit" | "parent"; commit: string };

const IMAGE = /\.(png|jpe?g|gif|webp|bmp|ico|svg|tga|psd|dds|exr|hdr|tiff?)$/i;

export const isImage = (path: string) => IMAGE.test(path);

export function imageFormatLabel(path: string): string | null {
  const match = /\.([^./\\]+)$/.exec(path);
  if (!match || !isImage(path)) {
    return null;
  }
  const ext = match[1].toUpperCase();
  if (ext === "JPEG") return "JPG";
  if (ext === "TIF") return "TIFF";
  return ext;
}

export function formatSizeDelta(before: number | null | undefined, after: number | null | undefined): string | null {
  if (before == null || after == null) {
    return null;
  }
  if (before === after) {
    return "same size";
  }
  const delta = after - before;
  return `${delta > 0 ? "+" : "-"}${formatSize(Math.abs(delta))}`;
}

/**
 * URL of `path` at a version. WebView2 reaches custom protocols as
 * `http://<scheme>.localhost`, other webviews as `<scheme>://localhost`.
 * `bust` changes the URL when the version's content may have changed.
 */
export function imageUrl(repoId: number, path: string, version: ImageAt, bust = 0, windows = isWindows()): string {
  return protocolUrl("image", versionQuery(repoId, path, version, bust), windows);
}

/** The query naming `path` at a version, for any protocol route that serves a file version. */
export function versionQuery(repoId: number, path: string, at: ImageAt, bust = 0): URLSearchParams {
  const q = new URLSearchParams({ repo: String(repoId), path, at: at.at });
  if ("commit" in at) {
    q.set("commit", at.commit);
  }
  q.set("v", String(bust));
  return q;
}

/** `route?query` on the `gitgud` custom protocol, as this webview reaches it. */
export function protocolUrl(route: string, query: URLSearchParams, windows = isWindows()): string {
  return `${windows ? "http://gitgud.localhost" : "gitgud://localhost"}/${route}?${query}`;
}

export function isWindows() {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}
