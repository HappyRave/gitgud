import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { BoxGeometry, Group, Mesh, MeshStandardMaterial } from "three";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { compare } from "$lib/model/compare";
import type { LoadedModel } from "$lib/model/load";
import type { ModelSide, Opened } from "$lib/model/open";
import { prefs, shaderBase } from "$lib/prefs.svelte";
import ModelViewer from "./ModelViewer.svelte";

const stages: FakeStage[] = [];

class FakeStage {
  comparison = { status: new Map(), changes: [] as ReturnType<typeof compare>["changes"] };
  mode = "side";
  wire = false;
  split = 0.5;
  playing = false;
  clip: string | null = null;
  shading = "textured";
  ontime = (_t: number, _d: number) => {};
  frame = vi.fn();
  frameAll = vi.fn();
  setClip = vi.fn((name: string | null) => (this.clip = name));
  seek = vi.fn();
  refresh = vi.fn();
  clear = vi.fn();
  cameraMoved = false;
  added() {}
  private models: [LoadedModel | null, LoadedModel | null] = [null, null];
  constructor() {
    stages.push(this);
  }
  setModels(before: LoadedModel | null, after: LoadedModel) {
    this.models = [before, after];
    this.comparison = before ? compare(before.root, after.root) : { status: new Map(), changes: [] };
    this.clip = after.clips[0]?.name ?? null;
  }
  get single() {
    return !this.models[0];
  }
  clips() {
    return this.models[1]?.clips ?? [];
  }
  setShading(s: string) {
    this.shading = s;
  }
  setOnion() {}
  dim = false;
  setDimUnchanged(d: boolean) {
    this.dim = d;
  }
  setActive() {}
  invalidate() {}
  resize() {}
  pick() {}
  dispose() {}
}

const opened = new Map<string, Opened>();
/** What each load asked to open: path and side. */
const asked: string[] = [];

vi.mock("$lib/model/stage", () => ({ ModelStage: FakeStage }));
vi.mock("$lib/model/open", async (orig) => ({
  ...(await orig<typeof import("$lib/model/open")>()),
  openSide: async (_repo: number, path: string, side: ModelSide) => {
    asked.push(`${path} ${side.label}`);
    return opened.get(side.label) ?? { model: null, error: "Assets/boat.fbx is not in that version" };
  },
}));

function boat(cabinX: number, withMast: boolean): LoadedModel {
  const root = new Group();
  const cabin = Object.assign(new Mesh(new BoxGeometry(), new MeshStandardMaterial()), { name: "Cabin" });
  cabin.position.x = cabinX;
  root.add(cabin);
  if (withMast) {
    root.add(Object.assign(new Mesh(new BoxGeometry(), new MeshStandardMaterial()), { name: "Mast" }));
  }
  return { root, clips: [{ name: "Idle_Bob", duration: 2 } as never], approximated: ["BoatAttack/Hull"], missing: [] };
}

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

beforeEach(() => {
  stages.length = 0;
  opened.clear();
  asked.length = 0;
  localStorage.clear();
  prefs.modelShading = "textured";
  prefs.modelPlaying = false;
});

describe("ModelViewer", () => {
  it("opens the file it was asked for, even when another comes before its first load starts", async () => {
    opened.set("HEAD", { model: boat(0, true), error: null });
    opened.set("On disk", { model: boat(0, true), error: null });
    const view = render(ModelViewer, { repoId: 1, path: "Assets/gun.prefab", before: HEAD, after: DISK });
    await view.rerender({ repoId: 1, path: "Assets/Scenes/Big.unity", before: HEAD, after: DISK });
    expect(await screen.findByText("On disk", {}, { timeout: 5000 })).toBeInTheDocument();
    // The first load was overtaken before it opened anything, so it opens nothing.
    expect(asked.sort()).toEqual(["Assets/Scenes/Big.unity HEAD", "Assets/Scenes/Big.unity On disk"]);
  });

  it("lets a custom shader's base map be picked from its textures, and reloads with it", async () => {
    const ground = (): LoadedModel => ({
      ...boat(0, false),
      approximated: ["Custom/Ground"],
      bases: { "Custom/Ground": { auto: "_Dirt", slots: ["_Dirt", "_Grass"] } },
    });
    opened.set("On disk", { model: ground(), error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/Cemetery.unity", after: DISK });
    await fireEvent.click(await screen.findByRole("button", { name: "1 warning" }, { timeout: 5000 }));

    const select = screen.getByRole("combobox", { name: /base map/ }) as HTMLSelectElement;
    expect(select.value).toBe("_Dirt");
    expect(asked).toEqual(["Assets/Cemetery.unity On disk"]);
    await fireEvent.change(select, { target: { value: "_Grass" } });

    expect(shaderBase("Custom/Ground")).toBe("_Grass");
    await waitFor(() => expect(asked).toEqual(["Assets/Cemetery.unity On disk", "Assets/Cemetery.unity On disk"]));
  });

  it("compares two versions and remembers the Changes shading", async () => {
    opened.set("HEAD", { model: boat(0, true), error: null });
    opened.set("On disk", { model: boat(-0.35, false), error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", before: HEAD, after: DISK });

    expect(screen.getByRole("progressbar", { name: "Loading model…" })).toBeInTheDocument();
    // The first load imports the viewer's modules, slow when the whole suite runs at once.
    expect(await screen.findByText("On disk", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByText("HEAD")).toBeInTheDocument();
    expect(screen.queryByText("BoatAttack/Hull is shown as a standard lit material")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "1 warning" }));
    expect(screen.getByText("BoatAttack/Hull is shown as a standard lit material")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "1 warning" }));
    expect(screen.queryByRole("note")).toBeNull();
    expect(screen.queryByRole("button", { name: /Cabin/ })).toBeNull();

    await fireEvent.click(screen.getByRole("radio", { name: "Changes" }));
    expect(localStorage.getItem("gitgud.model.shading")).toBe("changes");
    expect(stages[0].shading).toBe("changes");
    const cabin = screen.getByRole("button", { name: /Cabin/ });
    expect(cabin).toHaveTextContent("moved 0.35 m");
    expect(cabin).toHaveAttribute("title", "Fly to Cabin");
    expect(screen.getByRole("button", { name: /Mast/ })).toHaveTextContent("−");
    await fireEvent.click(cabin);
    expect(stages[0].frame).toHaveBeenCalledWith(expect.objectContaining({ name: "Cabin" }));

    await fireEvent.click(screen.getByRole("radio", { name: "Swipe" }));
    expect(stages[0].mode).toBe("swipe");
    expect(screen.getByRole("button", { name: "Drag to swipe" })).toBeInTheDocument();
  });

  it("builds sides with a given opener and shows the given controls instead of its own", async () => {
    const opener = vi.fn(async (_side: ModelSide): Promise<Opened> => ({ model: boat(0, false), error: null }));
    const chrome = createRawSnippet(() => ({ render: () => "<button>Sphere</button>" }));
    render(ModelViewer, { repoId: 1, path: "Assets/M/hull.mat", before: HEAD, after: DISK, opener, chrome });
    expect(await screen.findByRole("button", { name: "Sphere" })).toBeInTheDocument();
    expect(opener.mock.calls.map(([s]) => s.label)).toEqual(["HEAD", "On disk"]);
    expect(screen.queryByRole("radio", { name: "Changes" })).toBeNull();
    expect(screen.queryByText(/tris/)).toBeNull();
    expect(stages[0].shading).toBe("textured");
  });

  it("offers Merged for scenes, filters a long change list and fades what didn't change", async () => {
    const many = (dx: number) => {
      const root = new Group();
      for (let i = 0; i < 10; i++) {
        const m = Object.assign(new Mesh(new BoxGeometry(), new MeshStandardMaterial()), { name: `Buoy_${i}` });
        m.position.x = i + (i < 9 ? dx : 0);
        root.add(m);
      }
      return { root, clips: [], approximated: [], missing: [] } as LoadedModel;
    };
    opened.set("HEAD", { model: many(0), error: null });
    opened.set("On disk", { model: many(1), error: null });
    render(ModelViewer, { repoId: 1, path: "Scenes/Harbour.unity", before: HEAD, after: DISK, merged: true });
    await screen.findByText("On disk");
    await fireEvent.click(screen.getByRole("radio", { name: "Merged" }));
    expect(stages[0].mode).toBe("merged");
    await fireEvent.click(screen.getByRole("radio", { name: "Changes" }));
    await fireEvent.input(screen.getByLabelText("Filter changes"), { target: { value: "buoy_3" } });
    expect(screen.getAllByRole("button", { name: /Buoy_/ })).toHaveLength(1);
    await fireEvent.click(screen.getByRole("button", { name: "Changed only" }));
    expect(stages[0].dim).toBe(true);
  });

  it("frames a scene again once its meshes are in, unless the camera was moved meanwhile", async () => {
    const growing = (): Opened => {
      const model = boat(0, false);
      return { model, error: null, load: async () => model };
    };
    opened.set("On disk", growing());
    const view = render(ModelViewer, { repoId: 1, path: "Assets/gun.prefab", after: DISK });
    await waitFor(() => expect(stages[0]?.refresh).toHaveBeenCalled(), { timeout: 5000 });
    expect(stages[0].frameAll).toHaveBeenCalledTimes(1);

    stages[0].cameraMoved = true;
    stages[0].frameAll.mockClear();
    stages[0].refresh.mockClear();
    opened.set("On disk", growing());
    await view.rerender({ repoId: 1, path: "Assets/other.prefab", after: DISK });
    await waitFor(() => expect(stages[0].refresh).toHaveBeenCalled());
    expect(stages[0].frameAll).not.toHaveBeenCalled();
  });

  it("says when gitignored files were drawn from the disk copy", async () => {
    opened.set("HEAD", { model: { ...boat(0, false), fromDisk: 0 }, error: null });
    opened.set("On disk", { model: { ...boat(0, false), approximated: [], fromDisk: 1240 }, error: null });
    render(ModelViewer, { repoId: 1, path: "Scenes/Mirror.unity", before: HEAD, after: DISK });
    await screen.findByText("On disk");
    await fireEvent.click(screen.getByRole("button", { name: /warnings?$/ }));
    expect(screen.getByText("1,240 gitignored files come from your disk copy, the same in both versions")).toBeInTheDocument();
  });

  it("shows an added file alone, with compare and Changes off", async () => {
    opened.set("On disk", { model: boat(0, false), error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", before: HEAD, after: DISK });

    expect(await screen.findByText("On disk")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Side by side" })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "Changes" })).toBeDisabled();
    expect(screen.queryByText(/not in that version/)).toBeNull();
  });

  it("plays the clip and remembers it, and Space toggles it", async () => {
    opened.set("On disk", { model: boat(0, false), error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", after: DISK });

    await fireEvent.click(await screen.findByRole("button", { name: "Play" }));
    expect(localStorage.getItem("gitgud.model.playing")).toBe("1");
    await waitFor(() => expect(stages[0].playing).toBe(true));
    await fireEvent.keyDown(screen.getByRole("application"), { key: " " });
    expect(prefs.modelPlaying).toBe(false);
    await fireEvent.keyDown(screen.getByRole("application"), { key: "f" });
    expect(stages[0].frameAll).toHaveBeenCalled();
  });

  it("finishes loading when the same reference is missing twice and two clips share a name", async () => {
    const model = boat(0, false);
    const gone = "Quest3R_Grab › MeshFilter.m_Mesh → guid 9d33…541 (not in this version)";
    model.missing = [gone, gone];
    model.clips = [{ name: "Idle", duration: 1 } as never, { name: "Idle", duration: 2 } as never];
    opened.set("On disk", { model, error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/rig.prefab", after: DISK });

    await fireEvent.click(await screen.findByRole("button", { name: "2 warnings" }));
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getAllByText(gone)).toHaveLength(1);
  });

  it("clears the last model when the next file fails, and ignores equal versions passed anew", async () => {
    opened.set("On disk", { model: boat(0, false), error: null });
    const view = render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", after: DISK });
    await screen.findByText("On disk", {}, { timeout: 5000 });
    await view.rerender({ repoId: 1, path: "Assets/boat.fbx", after: { ...DISK, at: { ...DISK.at } } });
    expect(asked).toEqual(["Assets/boat.fbx On disk"]);

    opened.set("On disk", { model: null, error: "Assets/raft.fbx: not an FBX file" });
    await view.rerender({ repoId: 1, path: "Assets/raft.fbx", after: DISK });
    expect(await screen.findByText(/not an FBX file/)).toBeInTheDocument();
    expect(stages[0].clear).toHaveBeenCalled();
  });

  it("offers the LFS download when the model isn't on this machine", async () => {
    opened.set("On disk", { model: null, error: "Assets/boat.fbx: LFS object (38 MB) isn't downloaded — click Fetch to download" });
    const onfetch = vi.fn(async () => {});
    const meta = { before: null, after: { size: 40_000_000, lfs: true, oid: "ab12", hydrated: false } };
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", after: DISK, meta, onfetch });

    await fireEvent.click(await screen.findByRole("button", { name: /Fetch/ }));
    expect(onfetch).toHaveBeenCalledWith("after", meta.after);
  });
});
