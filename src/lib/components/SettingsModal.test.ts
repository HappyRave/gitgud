import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { BufferAttribute, BufferGeometry, Group, Mesh, MeshBasicMaterial } from "three";
import { describe, expect, it, vi } from "vitest";
import type { CacheStats } from "$lib/api";
import { modelCache } from "$lib/model/cache";
import {
  AUTO_3D_DEFAULT,
  prefs,
  previewTexture,
  setAuto3dLimit,
  setTextureMax,
  TEXTURE_MAX_DEFAULT,
} from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import SettingsModal from "./SettingsModal.svelte";

const stats = (over: Partial<CacheStats> = {}): CacheStats => ({
  guidIndexFile: "C:/work/boat/.git/gitgud/guid-index",
  guidIndexBytes: 0,
  metaFiles: 0,
  commitScans: 0,
  metaBlobs: 0,
  modelInfos: 0,
  commitGraphPath: "C:/work/boat/.git/objects/info/commit-graph",
  commitGraphBytes: null,
  lfsObjectsPath: "C:/work/boat/.git/lfs/objects",
  previewCachePath: "C:/work/boat/.git/gitgud/cache",
  previewModels: 0,
  previewTextures: 0,
  previewBytes: 0,
  ...over,
});

describe("SettingsModal", () => {
  it("checks for updates on request and says when gitgud is up to date", async () => {
    const calls = mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
      update_check: () => null,
    });
    render(SettingsModal, { onclose: () => {} });
    await fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("gitgud is up to date.")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "update_check")?.args).toEqual({ manual: true });
  });

  it("sets the 3D object limit and texture size, remembered", async () => {
    mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
    });
    render(SettingsModal, { onclose: () => {} });
    const size = (await screen.findByLabelText("Texture size in 3D")) as HTMLSelectElement;
    expect([...size.options].map((o) => o.textContent)).toEqual(["Native", "4096 px", "2048 px", "1024 px"]);
    expect(size.value).toBe("1024");
    await fireEvent.change(size, { target: { value: "0" } });
    expect(prefs.textureMax).toBe(0);
    expect(localStorage.getItem("gitgud.preview.textureMax")).toBe("0");
    expect(previewTexture("http://gitgud.localhost/image?path=a.png")).toBe("http://gitgud.localhost/image?path=a.png");
    await fireEvent.change(size, { target: { value: "2048" } });
    expect(previewTexture("http://gitgud.localhost/image?path=a.png")).toBe("http://gitgud.localhost/image?path=a.png&max=2048");
    await fireEvent.change(screen.getByLabelText("Open 3D automatically up to this many objects"), { target: { value: "5000" } });
    expect(prefs.auto3dLimit).toBe(5000);
    setTextureMax(TEXTURE_MAX_DEFAULT);
    setAuto3dLimit(AUTO_3D_DEFAULT);
  });

  it("turns the overlap radar's Windows notification off and on, on by default", async () => {
    mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
    });
    render(SettingsModal, { onclose: () => {} });
    const box = (await screen.findByLabelText(/Windows notification when a file you're changing won't merge/)) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await fireEvent.click(box);
    expect(prefs.overlapNotify).toBe(false);
    expect(localStorage.getItem("gitgud.overlap.notify")).toBe("0");
    await fireEvent.click(box);
    expect(prefs.overlapNotify).toBe(true);
    expect(localStorage.getItem("gitgud.overlap.notify")).toBe("1");
    localStorage.removeItem("gitgud.overlap.notify");
  });

  it("loads diagnostics and identity and saves repo-local or global config", async () => {
    let graph: number | null = null;
    const calls = mockCommands({
      config_get_identity: ({ id }) =>
        id != null ? { userName: "Local Max", userEmail: "local@example.com" } : { userName: "Global Max", userEmail: "global@example.com" },
      config_set_identity: () => null,
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
      has_commit_graph: () => false,
      write_commit_graph: () => {
        graph = 4096;
        return null;
      },
      cache_stats: () => stats({ commitGraphBytes: graph }),
      git_inventory: () => ({
        choice: { mode: "auto" },
        active: { path: "C:/Git/cmd/git.exe", version: "2.56.1", source: "system" },
        activeError: null,
        system: null,
        embedded: null,
        custom: null,
        minGit: "2.45.2",
        settingsPath: null,
      }),
    });

    render(SettingsModal, { activeRepoId: 7, activeRepoName: "boat", onclose: vi.fn() });

    expect(await screen.findByText("2.56.1")).toBeInTheDocument();
    expect((screen.getByLabelText("user.name") as HTMLInputElement).value).toBe("Global Max");

    await fireEvent.click(screen.getByRole("button", { name: "boat" }));
    expect((await screen.findByDisplayValue("Local Max")) as HTMLInputElement).toBeInTheDocument();

    await fireEvent.input(screen.getByLabelText("user.name"), { target: { value: "New Name" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save identity" }));
    expect(await screen.findByText("Saved for boat")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "config_set_identity")).toEqual({
      cmd: "config_set_identity",
      args: { id: 7, name: "New Name", email: "local@example.com" },
    });

    await fireEvent.change(screen.getByLabelText("Theme"), { target: { value: "dark" } });
    expect(prefs.theme).toBe("dark");
    await fireEvent.change(screen.getByLabelText("Density"), { target: { value: "compact" } });
    expect(prefs.density).toBe("compact");

    expect(await screen.findByText("Not built")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Build" }));
    expect(await screen.findByText("4.0 KB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rebuild" })).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "write_commit_graph")).toBe(true);
  });

  it("shows where each cache lives and clears the Unity and 3D caches", async () => {
    const calls = mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: null }),
      has_commit_graph: () => true,
      cache_stats: () => stats({ metaFiles: 20654, commitScans: 3, modelInfos: 12, guidIndexBytes: 2 * 1024 * 1024, commitGraphBytes: 512 }),
      clear_unity_caches: () => stats(),
    });
    const geo = new BufferGeometry();
    geo.setAttribute("position", new BufferAttribute(new Float32Array(3 * 1024), 3));
    const root = new Group();
    root.add(new Mesh(geo, new MeshBasicMaterial()));
    await modelCache.model("boat", async () => ({ root, clips: [], approximated: [], missing: [] }));
    const dispose = vi.spyOn(geo, "dispose");

    render(SettingsModal, { activeRepoId: 7, activeRepoName: "boat", onclose: vi.fn() });
    const rail = within(screen.getByRole("navigation", { name: "Settings sections" }));
    expect(rail.getByText("App")).toBeInTheDocument();
    expect(rail.getByText("boat")).toBeInTheDocument();
    const railText = screen.getByRole("navigation", { name: "Settings sections" }).textContent ?? "";
    expect(railText.indexOf("Tools")).toBeLessThan(railText.indexOf("Repository"));
    expect(railText.indexOf("Repository")).toBeLessThan(railText.indexOf("Caches"));
    const section = await screen.findByRole("region", { name: "Caches" });
    const caches = within(section);
    expect(await caches.findByText(/20,654 \.meta files · 3 commits scanned · 12 models read/)).toBeInTheDocument();
    expect(caches.getByText("C:/work/boat/.git/gitgud/guid-index")).toBeInTheDocument();
    expect(caches.getByText(/\(2\.0 MB\)/)).toBeInTheDocument();
    expect(caches.getByText("C:/work/boat/.git/objects/info/commit-graph")).toBeInTheDocument();
    expect(caches.getByText(/1 models · 0 materials · ~12\.0 KB/)).toBeInTheDocument();

    const [clearUnity, clear3d] = caches.getAllByRole("button", { name: "Clear" });
    await fireEvent.click(clearUnity);
    expect(await caches.findByText(/0 \.meta files/)).toBeInTheDocument();
    expect(caches.getByText("(not written yet)", { exact: false })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "clear_unity_caches")?.args).toEqual({ id: 7 });

    await fireEvent.click(clear3d);
    expect(dispose).toHaveBeenCalled();
    expect(caches.getByText(/0 models · 0 materials · ~0 B/)).toBeInTheDocument();
    expect(clear3d).toBeDisabled();
  });

  it("shows the 3D preview cache on disk and clears it", async () => {
    const calls = mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: null }),
      has_commit_graph: () => true,
      cache_stats: () => stats({ previewModels: 2, previewTextures: 112, previewBytes: 48 * 1024 * 1024 }),
      clear_preview_cache: () => stats(),
    });
    render(SettingsModal, { activeRepoId: 7, activeRepoName: "boat", onclose: vi.fn() });
    const caches = within(await screen.findByRole("region", { name: "Caches" }));
    expect(await caches.findByText(/2 scenes & prefabs · 112 textures · 48\.0 MB/)).toBeInTheDocument();
    expect(caches.getByText("C:/work/boat/.git/gitgud/cache")).toBeInTheDocument();
    const clear = caches.getAllByRole("button", { name: "Clear" })[2];
    await fireEvent.click(clear);
    expect(await caches.findByText(/0 scenes & prefabs · 0 textures · 0 B/)).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "clear_preview_cache")?.args).toEqual({ id: 7 });
    expect(clear).toBeDisabled();
  });

  it("displays AI providers, prunes LFS cache, writes agent instructions, refreshes index, and shows latency", async () => {
    let pruned = false;
    const calls = mockCommands({
      config_get_identity: () => ({ userName: "Max", userEmail: "max@example.com" }),
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
      has_commit_graph: () => true,
      ai_providers: () => ({ cliAvailable: ["claude", "codex"], configuredEnvKey: false }),
      lfs_cache_stats: () =>
        pruned
          ? { objectCount: 1, totalBytes: 2048, prunableCount: 0, prunableBytes: 0 }
          : { objectCount: 3, totalBytes: 4096, prunableCount: 2, prunableBytes: 2048 },
      lfs_prune: () => {
        pruned = true;
        return { objectCount: 1, totalBytes: 2048, prunableCount: 0, prunableBytes: 0 };
      },
      agent_setup: ({ target, withHooks }) => {
        const base = target === "cursor" ? ".cursor/rules/gitgud.mdc" : ".claude/skills/gitgud/SKILL.md";
        return target === "claude" && withHooks ? `${base} + .claude/settings.local.json` : base;
      },
      refresh_index: () => ({ id: 7, root: "C:/work/boat", rows: 1, graphVersion: 0 }),
      recent_timings: () => [{ op: "status", elapsedMs: 12, at: 1_700_000_000 }],
      cache_stats: () => stats(),
    });

    render(SettingsModal, { activeRepoId: 7, activeRepoName: "boat", onclose: vi.fn() });

    expect(await screen.findByText("claude, codex")).toBeInTheDocument();
    expect(await screen.findByText(/3 objects · 4\.0 KB · 2 prunable/)).toBeInTheDocument();

    expect(screen.getByText("C:/work/boat/.git/lfs/objects")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Prune" }));
    expect(await screen.findByText(/1 object · 2\.0 KB/)).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "lfs_prune")?.args).toEqual({ id: 7 });

    await fireEvent.change(screen.getByLabelText("Agent target"), { target: { value: "cursor" } });
    expect(screen.queryByRole("checkbox", { name: /Install Claude Stop hook/ })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Write agent instructions" }));
    expect(await screen.findByText("Wrote .cursor/rules/gitgud.mdc")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "agent_setup")?.args).toEqual({ id: 7, target: "cursor" });

    await fireEvent.change(screen.getByLabelText("Agent target"), { target: { value: "claude" } });
    await fireEvent.click(screen.getByRole("checkbox", { name: /Install Claude Stop hook/ }));
    await fireEvent.click(screen.getByRole("button", { name: "Write agent instructions" }));
    expect(await screen.findByText("Wrote .claude/skills/gitgud/SKILL.md + .claude/settings.local.json")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "agent_setup").map((c) => c.args)).toEqual([
      { id: 7, target: "cursor" },
      { id: 7, target: "claude", withHooks: true },
    ]);

    await fireEvent.click(screen.getByRole("button", { name: "Refresh index" }));
    expect(await screen.findByText("Refreshed")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "refresh_index")?.args).toEqual({ id: 7 });

    expect(await screen.findByText(/Local latency \(1 recent ops\)/)).toBeInTheDocument();
    expect(screen.getByText("12 ms")).toBeInTheDocument();
  });
});
