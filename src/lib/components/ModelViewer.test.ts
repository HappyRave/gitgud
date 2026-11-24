import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { BoxGeometry, Group, Mesh, MeshStandardMaterial } from "three";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { compare } from "$lib/model/compare";
import type { LoadedModel } from "$lib/model/load";
import type { ModelSide, Opened } from "$lib/model/open";
import { prefs } from "$lib/prefs.svelte";
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
  setActive() {}
  invalidate() {}
  resize() {}
  pick() {}
  dispose() {}
}

const opened = new Map<string, Opened>();

vi.mock("$lib/model/stage", () => ({ ModelStage: FakeStage }));
vi.mock("$lib/model/open", async (orig) => ({
  ...(await orig<typeof import("$lib/model/open")>()),
  openSide: async (_repo: number, _path: string, side: ModelSide) =>
    opened.get(side.label) ?? { model: null, error: "Assets/boat.fbx is not in that version" },
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
  localStorage.clear();
  prefs.modelShading = "textured";
  prefs.modelPlaying = false;
});

describe("ModelViewer", () => {
  it("compares two versions and remembers the Changes shading", async () => {
    opened.set("HEAD", { model: boat(0, true), error: null });
    opened.set("On disk", { model: boat(-0.35, false), error: null });
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", before: HEAD, after: DISK });

    expect(screen.getByRole("progressbar", { name: "Loading model…" })).toBeInTheDocument();
    expect(await screen.findByText("On disk")).toBeInTheDocument();
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

  it("offers the LFS download when the model isn't on this machine", async () => {
    opened.set("On disk", { model: null, error: "Assets/boat.fbx: LFS object (38 MB) isn't downloaded — click Fetch to download" });
    const onfetch = vi.fn(async () => {});
    const meta = { before: null, after: { size: 40_000_000, lfs: true, oid: "ab12", hydrated: false } };
    render(ModelViewer, { repoId: 1, path: "Assets/boat.fbx", after: DISK, meta, onfetch });

    await fireEvent.click(await screen.findByRole("button", { name: /Fetch/ }));
    expect(onfetch).toHaveBeenCalledWith("after", meta.after);
  });
});
