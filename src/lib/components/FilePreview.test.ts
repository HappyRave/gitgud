import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { prefs, viewModeFor } from "$lib/prefs.svelte";
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
    mockCommands({ unity_asset_info: () => ({ binary: false, classId: 43, size: 900 }) });
    render(FilePreview, { repoId: 1, path: "Meshes/Hull.asset", before: HEAD, after: DISK });
    expect(await screen.findByTestId("stub")).toHaveAttribute("data-path", "Meshes/Hull.asset");
  });

  it("opens a ScriptableObject .asset as a data grid", async () => {
    mockCommands({
      unity_asset_info: () => ({ binary: false, classId: 114, size: 900 }),
      data_tables: () => ({ kind: "ScriptableObject · BoatDatabase", compared: false, tables: [] }),
    });
    render(FilePreview, { repoId: 1, path: "Data/Boats.asset", before: HEAD, after: DISK });
    expect(await screen.findByText("No table in this file.")).toBeInTheDocument();
  });

  it("offers Structure for an .asset with no visual view", async () => {
    mockCommands({ unity_asset_info: () => ({ binary: false, classId: 156, size: 900 }) });
    render(FilePreview, { repoId: 1, path: "Terrain/Island.asset", before: HEAD, after: DISK });
    await fireEvent.click(await screen.findByRole("button", { name: "Show Structure" }));
    expect(viewModeFor("Terrain/Island.asset")).toBe("structure");
  });

  it("says a binary Unity asset can't be read, compares sizes and opens either version", async () => {
    const calls = mockCommands({
      unity_asset_info: ({ version }) => ({ binary: true, classId: null, size: version === "head" ? 4 * 1024 * 1024 : 5 * 1024 * 1024 }),
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
