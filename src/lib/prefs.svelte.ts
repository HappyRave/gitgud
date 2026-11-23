const SPLIT_KEY = "gitgud.diff.split";
const LAYOUT_KEY = "gitgud.diff.layout";
const WS_KEY = "gitgud.diff.ignoreWs";
const DENSITY_KEY = "gitgud.ui.density";
const THEME_KEY = "gitgud.ui.theme";
const PANES_KEY = "gitgud.ui.panes";
const MODEL_SHADING_KEY = "gitgud.model.shading";
const MODEL_PLAYING_KEY = "gitgud.model.playing";
const MODEL_VIEW_KEY = "gitgud.model.view";

export type DiffLayout = "auto" | "unified" | "split";
export type Density = "standard" | "compact";
export type ThemeMode = "system" | "light" | "dark";

export interface PaneSizes {
  sidebarWidth: number | null;
  inspectorWidth: number | null;
  librarySidebarWidth: number | null;
  workingFilesHeight: number | null;
  commitFilesHeight: number | null;
  conflictListWidth: number | null;
}

const DEFAULT_PANES: PaneSizes = {
  sidebarWidth: null,
  inspectorWidth: null,
  librarySidebarWidth: null,
  workingFilesHeight: null,
  commitFilesHeight: null,
  conflictListWidth: null,
};

function readStorage(key: string): string | null {
  return typeof localStorage !== "undefined" ? localStorage.getItem(key) : null;
}

function writeStorage(key: string, value: string) {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(key, value);
  }
}

function readPanes(): PaneSizes {
  const raw = readStorage(PANES_KEY);
  if (!raw) {
    return { ...DEFAULT_PANES };
  }
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const pick = (k: keyof PaneSizes) => {
      const v = parsed?.[k];
      return typeof v === "number" && Number.isFinite(v) && v > 0 ? Math.round(v) : null;
    };
    return {
      sidebarWidth: pick("sidebarWidth"),
      inspectorWidth: pick("inspectorWidth"),
      librarySidebarWidth: pick("librarySidebarWidth"),
      workingFilesHeight: pick("workingFilesHeight"),
      commitFilesHeight: pick("commitFilesHeight"),
      conflictListWidth: pick("conflictListWidth"),
    };
  } catch {
    return { ...DEFAULT_PANES };
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
  panes: readPanes(),
  /** The 3D viewer's last shading (R6: remembered). */
  modelShading: (readStorage(MODEL_SHADING_KEY) === "changes" ? "changes" : "textured") as ModelShading,
  /** Whether the 3D viewer was left playing (R6: remembered). */
  modelPlaying: readStorage(MODEL_PLAYING_KEY) === "1",
  /** 3D or the text view for prefabs and text model files (3D first, then remembered). */
  modelView: (readStorage(MODEL_VIEW_KEY) === "source" ? "source" : "3d") as ModelView,
});

export type ModelShading = "textured" | "changes";
export type ModelView = "3d" | "source";

export function setModelShading(shading: ModelShading) {
  prefs.modelShading = shading;
  writeStorage(MODEL_SHADING_KEY, shading);
}

export function setModelPlaying(playing: boolean) {
  prefs.modelPlaying = playing;
  writeStorage(MODEL_PLAYING_KEY, playing ? "1" : "0");
}

export function setModelView(view: ModelView) {
  prefs.modelView = view;
  writeStorage(MODEL_VIEW_KEY, view);
}

export function setPaneSize<K extends keyof PaneSizes>(key: K, value: number | null) {
  prefs.panes[key] = value != null && Number.isFinite(value) && value > 0 ? Math.round(value) : null;
  writeStorage(PANES_KEY, JSON.stringify(prefs.panes));
}

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

export function applyDensity(density: Density = prefs.density) {
  if (typeof document === "undefined") {
    return;
  }
  const root = document.documentElement;
  if (density === "compact") {
    root.dataset.density = "compact";
  } else {
    delete root.dataset.density;
  }
}

export function setDensity(density: Density) {
  prefs.density = density;
  writeStorage(DENSITY_KEY, density);
  applyDensity(density);
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

applyTheme(prefs.theme);
applyDensity(prefs.density);
