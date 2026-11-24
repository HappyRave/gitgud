import { fireEvent, render, screen } from "@testing-library/svelte";
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

  it("opens a ScriptableObject .asset as a data grid", async () => {
    mockCommands({
      unity_asset_info: () => ({ binary: false, classId: 114, size: 900, objects: 0 }),
      data_tables: () => ({ kind: "ScriptableObject · BoatDatabase", compared: false, tables: [] }),
    });
    render(FilePreview, { repoId: 1, path: "Data/Boats.asset", before: HEAD, after: DISK });
    expect(await screen.findByText("No table in this file.")).toBeInTheDocument();
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
