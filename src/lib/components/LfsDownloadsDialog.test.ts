import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { LfsPlan, PartialState } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import LfsDownloadsDialog from "./LfsDownloadsDialog.svelte";

const GB = 1024 ** 3;
const share = (gb: number, files = 10) => ({ bytes: gb * GB, files });

function plan(state: PartialState, partial: boolean, onDisk = 33.6): LfsPlan {
  const used = state.buildScenes ? 12.8 : 0;
  return {
    partial,
    state,
    buildScenes: Array.from({ length: 12 }, (_, i) => `Assets/Scenes/S${i}.unity`),
    allScenes: ["Assets/_DEBUG/DEV_Max/Test.unity"],
    total: share(33.6),
    used: share(used),
    always: share(0.7),
    kept: share(state.kept.length ? 1.8 : 0),
    skipped: share(33.6 - used - 0.7, 5825),
    skippedOnDisk: share(partial ? 0 : 33.6 - used - 0.7, partial ? 0 : 5825),
    skippedFolders: [{ folder: "Assets/Plugins/TheThinWoman", skipped: share(4.2), total: share(4.9) }],
    skippedFiles: [
      { path: "Assets/Plugins/TheThinWoman/Textures/Skin.png", bytes: 3 * GB, onDisk: !partial },
      { path: "Assets/Plugins/TheThinWoman/Textures/Eyes.png", bytes: 0.2 * GB, onDisk: !partial },
      { path: "Assets/Plugins/TheThinWoman/Woman.fbx", bytes: 1 * GB, onDisk: !partial },
    ],
    onDisk: share(onDisk),
    missing: [],
    fetchedToRead: 0,
  };
}

function setup(partial: boolean) {
  const calls = mockCommands({
    lfs_partial_plan: ({ state }) => plan(state ?? { buildScenes: true, scenes: [], kept: [] }, partial),
    lfs_partial_apply: ({ state }) => plan(state ?? { buildScenes: true, scenes: [], kept: [] }, !!state, 13.5),
    lfs_partial_download: ({ paths }) => plan({ buildScenes: true, scenes: [], kept: paths }, true, 15.3),
  });
  const oncancel = vi.fn();
  render(LfsDownloadsDialog, { repoId: 1, repoName: "horrorescape", oncancel });
  return { calls, oncancel };
}

describe("LfsDownloadsDialog (R10-B)", () => {
  it("shows what the build's scenes use and applies it, freeing the rest", async () => {
    const { calls } = setup(false);
    expect(await screen.findByText("14 GB")).toBeInTheDocument();
    expect(screen.getByText(/of 34 GB on disk after Apply · 20 GB not used/)).toBeInTheDocument();
    expect(screen.getByText(/not used · 20 GB, 20 GB of it on disk now: Apply removes it/)).toBeInTheDocument();
    expect(screen.getByText(/Every scene in Build Settings/)).toBeInTheDocument();
    expect(screen.getByText("Assets/Plugins/TheThinWoman")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Download" })).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "Apply · free 20 GB" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "lfs_partial_apply")).toBe(true));
    expect(calls.find((c) => c.cmd === "lfs_partial_apply")!.args).toMatchObject({
      id: 1,
      state: { buildScenes: true, scenes: [], kept: [] },
    });
    // Partial now: folders can be downloaded and kept, and the repo made whole again.
    expect(await screen.findByRole("button", { name: "Download" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Download everything" })).toBeInTheDocument();
  });

  it("replans when a scene is added and downloads a folder into the kept list", async () => {
    const { calls } = setup(true);
    await screen.findByText(/Every scene in Build Settings/);
    const box = screen.getByLabelText("Add a scene or prefab");
    await fireEvent.input(box, { target: { value: "Assets/_DEBUG/DEV_Max/Test.unity" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "lfs_partial_plan").at(-1)!.args).toMatchObject({
        state: { scenes: ["Assets/_DEBUG/DEV_Max/Test.unity"] },
      }),
    );
    expect(screen.getByText("Assets/_DEBUG/DEV_Max/Test.unity")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Download" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "lfs_partial_download")!.args).toEqual({
        id: 1,
        paths: ["Assets/Plugins/TheThinWoman"],
      }),
    );
    expect(await screen.findByText("Kept by you")).toBeInTheDocument();
  });

  it("explains a bar segment on hover", async () => {
    setup(false);
    await screen.findByText(/Every scene in Build Settings/);
    expect(screen.queryByRole("tooltip")).toBeNull();
    await fireEvent.mouseEnter(screen.getByRole("img", { name: "Always kept" }));
    const tip = screen.getByRole("tooltip");
    expect(tip).toHaveTextContent("Always kept");
    expect(tip).toHaveTextContent("10 files");
    expect(tip).toHaveTextContent(/Unity loads these without a reference/);
    await fireEvent.mouseEnter(screen.getByRole("img", { name: "Not used by these scenes" }));
    expect(screen.getByRole("tooltip")).toHaveTextContent(/of it is on disk now\. Apply turns it back into pointer files/);
    await fireEvent.mouseLeave(screen.getByRole("img", { name: "Not used by these scenes" }));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("expands a folder of unused files down to the files, each downloadable once partial", async () => {
    const { calls } = setup(true);
    await screen.findByText(/Every scene in Build Settings/);
    expect(screen.getByText(/not used · .*, not downloaded/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Expand Assets/Plugins/TheThinWoman" }));
    expect(screen.getByText("Textures/")).toBeInTheDocument();
    expect(screen.getByText("Woman.fbx")).toBeInTheDocument();
    expect(screen.queryByText("Skin.png")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Expand Assets/Plugins/TheThinWoman/Textures" }));
    expect(screen.getByText("Skin.png")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Download Assets/Plugins/TheThinWoman/Textures/Skin.png" }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "lfs_partial_download")!.args).toEqual({
        id: 1,
        paths: ["Assets/Plugins/TheThinWoman/Textures/Skin.png"],
      }),
    );
    await fireEvent.click(screen.getByRole("button", { name: "Collapse Assets/Plugins/TheThinWoman" }));
    expect(screen.queryByText("Woman.fbx")).toBeNull();
  });

  it("closes on Escape when nothing runs", async () => {
    const { oncancel } = setup(false);
    await screen.findByText(/Every scene in Build Settings/);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(oncancel).toHaveBeenCalled();
  });
});
