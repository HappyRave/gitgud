const SPLIT_KEY = "gitgud.diff.split";
const LAYOUT_KEY = "gitgud.diff.layout";
const WS_KEY = "gitgud.diff.ignoreWs";
const DENSITY_KEY = "gitgud.ui.density";
const THEME_KEY = "gitgud.ui.theme";

export type DiffLayout = "auto" | "unified" | "split";
export type Density = "standard" | "compact";
export type ThemeMode = "system" | "light" | "dark";

function readStorage(key: string): string | null {
  return typeof localStorage !== "undefined" ? localStorage.getItem(key) : null;
}

function writeStorage(key: string, value: string) {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(key, value);
  }
}

const initialSplit = readStorage(SPLIT_KEY) === "1";
const storedLayout = readStorage(LAYOUT_KEY);
const initialLayout: DiffLayout =
  storedLayout === "auto" || storedLayout === "unified" || storedLayout === "split"
    ? storedLayout
    : initialSplit
      ? "split"
      : "auto";
const storedDensity = readStorage(DENSITY_KEY);
const storedTheme = readStorage(THEME_KEY);

/** App-wide view preferences, kept across launches. */
export const prefs = $state({
  /** Diffs side by side instead of unified (Ctrl+D). */
  split: initialLayout === "split" || (initialLayout === "auto" && initialSplit),
  diffLayout: initialLayout as DiffLayout,
  ignoreWhitespace: readStorage(WS_KEY) === "1",
  density: (storedDensity === "compact" ? "compact" : "standard") as Density,
  theme: (storedTheme === "light" || storedTheme === "dark" ? storedTheme : "system") as ThemeMode,
});

export function toggleSplit() {
  prefs.split = !prefs.split;
  prefs.diffLayout = prefs.split ? "split" : "unified";
  writeStorage(SPLIT_KEY, prefs.split ? "1" : "0");
  writeStorage(LAYOUT_KEY, prefs.diffLayout);
}

export function setDiffLayout(layout: DiffLayout) {
  prefs.diffLayout = layout;
  writeStorage(LAYOUT_KEY, layout);
  if (layout !== "auto") {
    prefs.split = layout === "split";
    writeStorage(SPLIT_KEY, prefs.split ? "1" : "0");
  }
}

export function toggleIgnoreWhitespace() {
  prefs.ignoreWhitespace = !prefs.ignoreWhitespace;
  writeStorage(WS_KEY, prefs.ignoreWhitespace ? "1" : "0");
}

export function setDensity(density: Density) {
  prefs.density = density;
  writeStorage(DENSITY_KEY, density);
}

export function applyTheme(theme: ThemeMode = prefs.theme) {
  if (typeof document === "undefined") {
    return;
  }
  const root = document.documentElement;
  if (theme === "light" || theme === "dark") {
    root.dataset.theme = theme;
  } else {
    delete root.dataset.theme;
  }
}

export function setTheme(theme: ThemeMode) {
  prefs.theme = theme;
  writeStorage(THEME_KEY, theme);
  applyTheme(theme);
}
