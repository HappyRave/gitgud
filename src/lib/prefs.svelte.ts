const KEY = "gitgud.diff.split";

/** App-wide view preferences, kept across launches. */
export const prefs = $state({
  /** Diffs side by side instead of unified (Ctrl+D). */
  split: typeof localStorage !== "undefined" && localStorage.getItem(KEY) === "1",
});

export function toggleSplit() {
  prefs.split = !prefs.split;
  localStorage.setItem(KEY, prefs.split ? "1" : "0");
}
