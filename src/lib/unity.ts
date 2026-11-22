const UNITY_YAML_EXTS = ["unity", "prefab", "asset", "mat", "anim", "controller", "overridecontroller"];

export function isUnityYaml(path: string): boolean {
  const dot = path.lastIndexOf(".");
  if (dot < 0) {
    return false;
  }
  return UNITY_YAML_EXTS.includes(path.slice(dot + 1).toLowerCase());
}

/** Matches simple `.gitattributes` LFS patterns like `*.psd` or `Assets/*.fbx` against a repo path. */
export function matchesLfsPattern(path: string, pattern: string): boolean {
  const normPath = path.replaceAll("\\", "/").toLowerCase();
  const pat = pattern.trim().replaceAll("\\", "/").toLowerCase();
  if (!pat) {
    return false;
  }
  if (pat.startsWith("*.")) {
    return normPath.endsWith(pat.slice(1));
  }
  const regex = new RegExp(
    "^" +
      pat
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replaceAll("*", "[^/]*")
        .replaceAll("?", ".") +
      "$",
  );
  if (pat.includes("/")) {
    return regex.test(normPath);
  }
  const base = normPath.slice(normPath.lastIndexOf("/") + 1);
  return regex.test(base);
}
