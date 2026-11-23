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
  const query = new URLSearchParams({ repo: String(repoId), path, at: version.at });
  if ("commit" in version) {
    query.set("commit", version.commit);
  }
  query.set("v", String(bust));
  return `${windows ? "http://gitgud.localhost" : "gitgud://localhost"}/image?${query}`;
}

function isWindows() {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}
