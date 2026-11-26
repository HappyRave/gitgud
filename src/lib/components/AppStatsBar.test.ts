import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { RepoCache } from "$lib/api";
import { modelCache } from "$lib/model/cache";
import { mockCommands } from "$lib/test/ipc";
import AppStatsBar from "./AppStatsBar.svelte";

const MB = 1024 * 1024;
const cache = (root: string, previewMb: number, unityMb = 0): RepoCache => ({
  root,
  previewBytes: previewMb * MB,
  previewModels: 12,
  previewTextures: 3,
  unityBytes: unityMb * MB,
  metaFiles: 2048,
});

function backend(caches: Record<string, RepoCache>, cpu = 64) {
  return mockCommands({
    app_stats: () => ({ cpu, appBytes: 120 * MB, viewBytes: 280 * MB, gitBytes: 12 * MB }),
    repo_caches: ({ roots }) => roots.filter((r) => caches[r]).map((r) => caches[r]),
    clear_repo_caches: ({ root }) => {
      caches[root] = cache(root, 0);
      return caches[root];
    },
  });
}

describe("AppStatsBar", () => {
  it("shows CPU with its level, RAM with the split on hover, and the repo's cache", async () => {
    backend({ "C:/boat": cache("C:/boat", 812, 14) });
    render(AppStatsBar, { repos: [{ root: "C:/boat", name: "boat" }] });

    const cpu = await screen.findByText("64%");
    expect(cpu).toHaveClass("hot");
    const ram = screen.getByText("412 MB");
    expect(ram.closest("[title]")).toHaveAttribute("title", "app 120 MB · view 280 MB · git 12 MB");
    expect(await screen.findByRole("button", { name: /826 MB/ })).toBeInTheDocument();
  });

  it("opens the popover, clears the repo's caches and the viewers' memory, and closes on Esc", async () => {
    const clear = vi.spyOn(modelCache, "clear");
    const calls = backend({ "C:/boat": cache("C:/boat", 812, 14) });
    render(AppStatsBar, { repos: [{ root: "C:/boat", name: "boat" }] });

    await fireEvent.click(await screen.findByRole("button", { name: /826 MB/ }));
    const pop = screen.getByRole("dialog", { name: "gitgud's caches" });
    expect(within(pop).getByText("boat")).toBeInTheDocument();
    expect(within(pop).getByText("812 MB")).toBeInTheDocument();
    expect(within(pop).getByText(/2,048 \.meta files/)).toBeInTheDocument();
    expect(within(pop).getByText(/Not touched: LFS objects/)).toBeInTheDocument();

    await fireEvent.click(within(pop).getByRole("button", { name: "Clear 826 MB" }));
    expect(await within(pop).findByText("✓ Freed 826 MB")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "clear_repo_caches").map((c) => c.args)).toEqual([{ root: "C:/boat" }]);
    expect(clear).toHaveBeenCalledOnce();
    expect(within(pop).getByRole("button", { name: "Clear" })).toBeDisabled();

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    clear.mockRestore();
  });

  it("on the Library, totals every repo and clears one without touching the others", async () => {
    const calls = backend(
      Object.fromEntries(
        ["a", "b", "c", "d", "e", "f", "g"].map((n, i) => [`C:/${n}`, cache(`C:/${n}`, (i + 1) * 100)]),
      ),
      2,
    );
    const repos = ["a", "b", "c", "d", "e", "f", "g", "gone"].map((n) => ({ root: `C:/${n}`, name: n }));
    render(AppStatsBar, { repos, all: true });

    expect(await screen.findByText("2%")).not.toHaveClass("hot");
    await fireEvent.click(await screen.findByRole("button", { name: /ALL REPOS 2\.7 GB/ }));
    const pop = screen.getByRole("dialog");
    expect(within(pop).getByText("8 repos · 2.7 GB")).toBeInTheDocument();
    expect(within(pop).getByText("g")).toBeInTheDocument();
    expect(within(pop).getByText("2 more")).toBeInTheDocument();
    expect(within(pop).queryByText("a")).not.toBeInTheDocument();

    await fireEvent.click(within(pop).getByRole("button", { name: "Clear g" }));
    expect(await within(pop).findByText("✓ Freed 700 MB")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "clear_repo_caches").map((c) => c.args)).toEqual([{ root: "C:/g" }]);
    expect(within(pop).getByRole("button", { name: /Clear all 2\.1 GB/ })).toBeEnabled();
  });

  it("says when CPU and RAM can't be read, and still offers the caches", async () => {
    mockCommands({
      app_stats: () => {
        throw "cannot read processes";
      },
      repo_caches: () => [cache("C:/boat", 0)],
    });
    render(AppStatsBar, { repos: [{ root: "C:/boat", name: "boat" }] });
    expect(await screen.findByRole("alert")).toHaveTextContent("CPU / RAM unavailable");
    expect(await screen.findByRole("button", { name: /0 B/ })).toBeInTheDocument();
  });
});
