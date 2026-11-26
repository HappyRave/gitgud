import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { AUTO_3D_DEFAULT, prefs, setAuto3dLimit, viewModeFor } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import FilePreview from "./FilePreview.svelte";

vi.mock("./ModelViewer.svelte", async () => ({ default: (await import("$lib/test/Stub.svelte")).default }));

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

beforeEach(() => {
  prefs.views = {};
});

describe("FilePreview", () => {
  it("opens a Unity Mesh .asset in 3D, read from its first object", async () => {
    mockCommands({ unity_asset_info: () => ({ binary: false, classId: 43, size: 900, objects: 0 }) });
    render(FilePreview, { repoId: 1, path: "Meshes/Hull.asset", before: HEAD, after: DISK });
    expect(await screen.findByTestId("stub")).toHaveAttribute("data-path", "Meshes/Hull.asset");
  });

  it("offers the LFS fetch when the shown .asset can't be probed", async () => {
    mockCommands({
      unity_asset_info: () => {
        throw new Error("Terrain/Island.asset's LFS object isn't downloaded");
      },
    });
    const onfetch = vi.fn(async () => {});
    const meta = { before: null, after: { size: 4096, lfs: true, hydrated: false } };
    render(FilePreview, { repoId: 1, path: "Terrain/Island.asset", before: null, after: DISK, meta, onfetch });
    expect(await screen.findByRole("alert")).toHaveTextContent("On disk can't load: Terrain/Island.asset's LFS object isn't downloaded");
    await fireEvent.click(screen.getByRole("button", { name: /Fetch LFS object/ }));
    expect(onfetch).toHaveBeenCalledWith("after", meta.after);
  });

  it("opens a ScriptableObject .asset as a data grid", async () => {
    mockCommands({
      unity_asset_info: () => ({ binary: false, classId: 114, size: 900, objects: 0 }),
      data_tables: () => ({ kind: "ScriptableObject · BoatDatabase", compared: false, tables: [] }),
    });
    render(FilePreview, { repoId: 1, path: "Data/Boats.asset", before: HEAD, after: DISK });
    expect(await screen.findByText("No table in this file.")).toBeInTheDocument();
  });

  it("gets a big scene ready while its card waits: both sides' scene info", async () => {
    const calls = mockCommands({
      unity_asset_info: () => ({ binary: false, classId: 29, size: 900, objects: 9_000 }),
      model_info: () => ({ kind: "prefab", key: null, materials: [], remaps: {}, scale: 1, scene: null, fromDisk: 0 }),
    });
    render(FilePreview, { repoId: 1, path: "Scenes/Big.unity", before: HEAD, after: DISK });
    expect(await screen.findByText("9,000 objects.")).toBeInTheDocument();
    await waitFor(() => expect(calls.filter((c) => c.cmd === "model_info")).toHaveLength(2));
    expect(calls.filter((c) => c.cmd === "model_info").map((c) => (c.args as { version: unknown }).version)).toEqual(["head", "worktree"]);
  });

  it("opens scenes and prefabs in 3D up to the Settings limit, bigger ones on request", async () => {
    let objects = 1840;
    mockCommands({ unity_asset_info: () => ({ binary: false, classId: 29, size: 900, objects }) });
    const { unmount } = render(FilePreview, { repoId: 1, path: "Scenes/Harbour.unity", before: HEAD, after: DISK });
    expect(await screen.findByTestId("stub")).toHaveAttribute("data-path", "Scenes/Harbour.unity");
    unmount();

    objects = 2_500;
    const big = render(FilePreview, { repoId: 1, path: "Scenes/Harbour.unity", before: HEAD, after: DISK });
    expect(await screen.findByText("2,500 objects.")).toBeInTheDocument();
    expect(screen.queryByTestId("stub")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Open in 3D" }));
    expect(screen.getByTestId("stub")).toBeInTheDocument();
    big.unmount();

    render(FilePreview, { repoId: 1, path: "Prefabs/Fleet.prefab", before: HEAD, after: DISK });
    expect(await screen.findByText("2,500 objects.")).toBeInTheDocument();
    setAuto3dLimit(5000);
    expect(await screen.findByTestId("stub")).toHaveAttribute("data-path", "Prefabs/Fleet.prefab");
    setAuto3dLimit(AUTO_3D_DEFAULT);
  });

  it("keeps the viewer when the parent passes the same versions as new objects", async () => {
    const calls = mockCommands({ unity_asset_info: () => ({ binary: false, classId: 29, size: 900, objects: 10 }) });
    const view = render(FilePreview, { repoId: 1, path: "Scenes/Harbour.unity", before: HEAD, after: DISK });
    const stub = await screen.findByTestId("stub");
    await view.rerender({ repoId: 1, path: "Scenes/Harbour.unity", before: { ...HEAD }, after: { ...DISK } });
    expect(screen.getByTestId("stub")).toBe(stub);
    expect(calls.filter((c) => c.cmd === "unity_asset_info")).toHaveLength(2);
  });

  it("never opens a big scene in 3D on the previous file's probe", async () => {
    mockCommands({
      unity_asset_info: ({ path }) => ({ binary: false, classId: 29, size: 900, objects: path.endsWith(".unity") ? 8_000 : 10 }),
    });
    const view = render(FilePreview, { repoId: 1, path: "Prefabs/Gun.prefab", before: HEAD, after: DISK });
    await screen.findByTestId("stub");
    let mountedScene = false;
    const watch = new MutationObserver((records) => {
      for (const r of records) {
        for (const n of r.addedNodes) {
          const el = n instanceof Element ? n : null;
          mountedScene ||= !!(el?.matches('[data-path="Scenes/Mirror.unity"]') || el?.querySelector('[data-path="Scenes/Mirror.unity"]'));
        }
        mountedScene ||= r.type === "attributes" && (r.target as Element).getAttribute("data-path") === "Scenes/Mirror.unity";
      }
    });
    watch.observe(document.body, { childList: true, subtree: true, attributes: true });
    await view.rerender({ repoId: 1, path: "Scenes/Mirror.unity", before: HEAD, after: DISK });
    expect(await screen.findByText("8,000 objects.")).toBeInTheDocument();
    watch.disconnect();
    expect(mountedScene).toBe(false);
  });

  it("offers Structure for an .asset with no visual view", async () => {
    mockCommands({ unity_asset_info: () => ({ binary: false, classId: 156, size: 900, objects: 0 }) });
    render(FilePreview, { repoId: 1, path: "Terrain/Island.asset", before: HEAD, after: DISK });
    await fireEvent.click(await screen.findByRole("button", { name: "Show Structure" }));
    expect(viewModeFor("Terrain/Island.asset")).toBe("structure");
  });

  it("says a binary Unity asset can't be read, compares sizes and opens either version", async () => {
    const calls = mockCommands({
      unity_asset_info: ({ version }) => ({ binary: true, classId: null, size: version === "head" ? 4 * 1024 * 1024 : 5 * 1024 * 1024, objects: 0 }),
      open_version: () => "C:/Temp/gitgud-open/1/Lighting (HEAD).asset",
    });
    render(FilePreview, { repoId: 1, path: "Scenes/Lighting.asset", before: HEAD, after: DISK });
    expect(await screen.findByText("Stored in Unity's binary format")).toBeInTheDocument();
    expect(screen.getByText("Size 4.0 MB → 5.0 MB")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Open HEAD in default app" }));
    expect(calls.find((c) => c.cmd === "open_version")?.args).toEqual({
      id: 1,
      path: "Scenes/Lighting.asset",
      version: "head",
      label: "HEAD",
    });
  });

  it("opens a .meta change in its asset's viewer with a note", async () => {
    mockCommands({});
    render(FilePreview, { repoId: 1, path: "Art/Boat.fbx.meta", before: HEAD, after: DISK });
    expect(screen.getByRole("note")).toHaveTextContent("Import settings changed in Boat.fbx.meta");
    expect(await screen.findByTestId("stub")).toHaveAttribute("data-path", "Art/Boat.fbx");
  });
});
