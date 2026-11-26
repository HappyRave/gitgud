import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import TabBar from "./TabBar.svelte";

const mkTab = (i: number) => ({
  path: `C:/work/repo-${i}`,
  name: `repo-${i}`,
  dirtyCount: i === 0 ? 3 : 0,
  locksHeld: i === 1 ? 2 : 0,
});

describe("TabBar", () => {
  it("always keeps Library first, shows dirty dot and lock badge, and handles select/close/add/quick", async () => {
    const onselect = vi.fn();
    const onclose = vi.fn();
    const onadd = vi.fn();
    const onquickswitch = vi.fn();

    render(TabBar, {
      tabs: [mkTab(0), mkTab(1)],
      activePath: "C:/work/repo-0",
      onselect,
      onclose,
      onadd,
      onquickswitch,
    });

    expect(screen.getByRole("tab", { name: "Library" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tab", { name: /repo-0/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByLabelText("3 uncommitted")).toBeInTheDocument();
    expect(screen.getByLabelText("2 locks held")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("tab", { name: "Library" }));
    expect(onselect).toHaveBeenCalledWith(null);

    await fireEvent.click(screen.getByRole("button", { name: "Close repo-1" }));
    expect(onclose).toHaveBeenCalledWith("C:/work/repo-1");

    await fireEvent.click(screen.getByRole("button", { name: "New tab" }));
    expect(onadd).toHaveBeenCalled();

    await fireEvent.click(screen.getByRole("button", { name: /Ctrl\+P/ }));
    expect(onquickswitch).toHaveBeenCalled();
  });

  it("moves tabs beyond 8 into the overflow menu", async () => {
    const onselect = vi.fn();
    render(TabBar, {
      tabs: Array.from({ length: 10 }, (_, i) => mkTab(i)),
      activePath: null,
      onselect,
      onclose: vi.fn(),
      onadd: vi.fn(),
      onquickswitch: vi.fn(),
    });

    const more = screen.getByRole("button", { name: "2 more open tabs" });
    await fireEvent.click(more);
    await fireEvent.click(screen.getByRole("menuitem", { name: /repo-9/ }));
    expect(onselect).toHaveBeenCalledWith("C:/work/repo-9");

    // Reopen and verify Escape and outside click dismiss the overflow menu.
    await fireEvent.click(more);
    expect(screen.getByRole("menu", { name: "More open tabs" })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("menu", { name: "More open tabs" })).not.toBeInTheDocument();

    await fireEvent.click(more);
    expect(screen.getByRole("menu", { name: "More open tabs" })).toBeInTheDocument();
    await fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu", { name: "More open tabs" })).not.toBeInTheDocument();
  });

  it("right-click on a repo tab opens it in Explorer, a terminal or the editor, and reports a failure", async () => {
    const calls = mockCommands({
      reveal_path: () => null,
      open_in_editor: () => {
        throw "VS Code isn't installed; pick another editor in Settings › Tools";
      },
    });
    const onclose = vi.fn();
    const onerror = vi.fn();
    render(TabBar, { tabs: [mkTab(0)], activePath: null, onselect: vi.fn(), onclose, onadd: vi.fn(), onquickswitch: vi.fn(), onerror });

    await fireEvent.contextMenu(screen.getByRole("tab", { name: /repo-0/ }), { clientX: 5, clientY: 5 });
    expect(screen.getAllByRole("menuitem").map((i) => i.textContent?.trim())).toEqual([
      "Show in Explorer",
      "Open Windows Terminal here",
      "Open in VS Code",
      "Close tab Ctrl+W",
    ]);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Show in Explorer" }));
    expect(calls).toEqual([{ cmd: "reveal_path", args: { root: "C:/work/repo-0", path: null } }]);

    await fireEvent.contextMenu(screen.getByRole("tab", { name: /repo-0/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Open in VS Code" }));
    await vi.waitFor(() => expect(onerror).toHaveBeenCalledWith("VS Code isn't installed; pick another editor in Settings › Tools"));

    await fireEvent.contextMenu(screen.getByRole("tab", { name: /repo-0/ }), { clientX: 5, clientY: 5 });
    await fireEvent.click(screen.getByRole("menuitem", { name: /Close tab/ }));
    expect(onclose).toHaveBeenCalledWith("C:/work/repo-0");
  });
});
