import { supportsBc } from "$lib/model/gpuFormats";
import { formatKey, type ViewMode, viewModes } from "$lib/viewers/route";

const SPLIT_KEY = "gitgud.diff.split";
const LAYOUT_KEY = "gitgud.diff.layout";
const WS_KEY = "gitgud.diff.ignoreWs";
const DENSITY_KEY = "gitgud.ui.density";
const THEME_KEY = "gitgud.ui.theme";
const PANES_KEY = "gitgud.ui.panes";
const MODEL_SHADING_KEY = "gitgud.model.shading";
const MODEL_PLAYING_KEY = "gitgud.model.playing";
const MODEL_VIEW_KEY = "gitgud.model.view";
const VIEWS_KEY = "gitgud.views";
const VIEWER_KEY = "gitgud.viewer";
const VOLUME_KEY = "gitgud.media.volume";
const AUTO_3D_KEY = "gitgud.preview.auto3d";
const TEXTURE_MAX_KEY = "gitgud.preview.textureMax";
const SKIPPED_UPDATE_KEY = "gitgud.update.skipped";

/** Largest texture side in 3D previews; 0 keeps textures at their native size. */
export type TextureMax = 0 | 4096 | 2048 | 1024;
export const TEXTURE_MAXES: readonly TextureMax[] = [0, 4096, 2048, 1024];
/** Max's pick, 2026-11-25. */
export const TEXTURE_MAX_DEFAULT: TextureMax = 1024;

function readTextureMax(): TextureMax {
  const v = Number(readStorage(TEXTURE_MAX_KEY) ?? TEXTURE_MAX_DEFAULT);
  return TEXTURE_MAXES.find((m) => m === v) ?? TEXTURE_MAX_DEFAULT;
}

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

function readViews(): Record<string, ViewMode> {
  const out: Record<string, ViewMode> = {};
  try {
    const parsed = JSON.parse(readStorage(VIEWS_KEY) ?? "{}") as Record<string, unknown>;
    for (const [k, v] of Object.entries(parsed)) {
      if (v === "visual" || v === "structure" || v === "source") {
        out[k] = v;
      }
    }
  } catch {
    // A broken value starts over with every format on its first view.
  }
  if (!out.prefab && readStorage(MODEL_VIEW_KEY) === "source") {
    out.prefab = "structure";
  }
  return out;
}

function readViewer(): Record<string, string> {
  try {
    const parsed = JSON.parse(readStorage(VIEWER_KEY) ?? "{}") as Record<string, unknown>;
    return Object.fromEntries(Object.entries(parsed).filter((e): e is [string, string] => typeof e[1] === "string"));
  } catch {
    return {};
  }
}

/** The default for opening 3D automatically: Max's pick, 2026-11-24. */
export const AUTO_3D_DEFAULT = 2000;

function readAuto3d(): number {
  const v = Number(readStorage(AUTO_3D_KEY) ?? AUTO_3D_DEFAULT);
  return Number.isFinite(v) && v >= 0 ? Math.round(v) : AUTO_3D_DEFAULT;
}

function readVolume(): number {
  const v = Number(readStorage(VOLUME_KEY) ?? "0.8");
  return Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0.8;
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
  /** The view last chosen per file format: Visual, Structure or Source (R7: remembered per format). */
  views: readViews(),
  /** Choices inside a viewer (font Sample / All glyphs / Metrics…), per format (R7). */
  viewer: readViewer(),
  /** One volume for every sound and video, 0–1 (R7). */
  volume: readVolume(),
  /** Scenes and prefabs open in 3D by themselves up to this many objects; above it, on request. */
  auto3dLimit: readAuto3d(),
  /** Textures in 3D previews are shrunk to this many px on their longer side; 0 = native. */
  textureMax: readTextureMax(),
  /** The release "Skip this version" was pressed for; the startup check stays quiet about it. */
  skippedUpdate: readStorage(SKIPPED_UPDATE_KEY),
});

export function setSkippedUpdate(version: string) {
  prefs.skippedUpdate = version;
  writeStorage(SKIPPED_UPDATE_KEY, version);
}

export type ModelShading = "textured" | "changes";

/** The view `path` opens in: the one last chosen for its format, else its first. */
export function viewModeFor(path: string): ViewMode {
  const modes = viewModes(path);
  const saved = prefs.views[formatKey(path)];
  return saved && modes.includes(saved) ? saved : modes[0];
}

export function setViewMode(path: string, mode: ViewMode) {
  prefs.views[formatKey(path)] = mode;
  writeStorage(VIEWS_KEY, JSON.stringify(prefs.views));
}

/** The choice `name` last made in `path`'s viewer if it is still one of `options`, else the first. */
export function viewerChoice<T extends string>(path: string, name: string, options: readonly T[]): T {
  const saved = prefs.viewer[`${formatKey(path)}.${name}`];
  return options.find((o) => o === saved) ?? options[0];
}

export function setViewerChoice(path: string, name: string, value: string) {
  prefs.viewer[`${formatKey(path)}.${name}`] = value;
  writeStorage(VIEWER_KEY, JSON.stringify(prefs.viewer));
}

/** The texture property picked as the base map for materials using `shader`, if any. */
export function shaderBase(shader: string): string | null {
  return prefs.viewer[`shader:${shader}.base`] ?? null;
}

export function setShaderBase(shader: string, property: string) {
  prefs.viewer[`shader:${shader}.base`] = property;
  writeStorage(VIEWER_KEY, JSON.stringify(prefs.viewer));
}

export function setTextureMax(max: TextureMax) {
  prefs.textureMax = TEXTURE_MAXES.includes(max) ? max : TEXTURE_MAX_DEFAULT;
  writeStorage(TEXTURE_MAX_KEY, String(prefs.textureMax));
}

/**
 * `url` of a texture for a 3D preview, asking the backend to shrink it per the setting,
 * and, when `compressed`, to GPU-compress it when the GPU takes that (only our own
 * loader reads the DDS that comes back, not three's loaders).
 */
export function previewTexture(url: string, compressed = true): string {
  if (!prefs.textureMax) {
    return url;
  }
  return `${url}&max=${prefs.textureMax}${compressed && supportsBc() ? "&gpu=bc" : ""}`;
}

export function setAuto3dLimit(limit: number) {
  prefs.auto3dLimit = Number.isFinite(limit) && limit >= 0 ? Math.round(limit) : AUTO_3D_DEFAULT;
  writeStorage(AUTO_3D_KEY, String(prefs.auto3dLimit));
}

export function setVolume(volume: number) {
  prefs.volume = Math.min(1, Math.max(0, volume));
  writeStorage(VOLUME_KEY, String(prefs.volume));
}

export function setModelShading(shading: ModelShading) {
  prefs.modelShading = shading;
  writeStorage(MODEL_SHADING_KEY, shading);
}

export function setModelPlaying(playing: boolean) {
  prefs.modelPlaying = playing;
  writeStorage(MODEL_PLAYING_KEY, playing ? "1" : "0");
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
