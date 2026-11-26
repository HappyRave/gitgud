import { afterEach, describe, expect, it, vi } from "vitest";
import { chosenEditor, editorName, launchItems } from "./launch";
import { prefs, setEditor, setTerminal } from "./prefs.svelte";
import { mockCommands } from "./test/ipc";

afterEach(() => {
  setTerminal("windowsTerminal");
  setEditor("vsCode", "");
});

describe("launchItems", () => {
  it("offers Explorer, the chosen terminal and editor, and the path for a file", async () => {
    const calls = mockCommands({ reveal_path: () => null, open_terminal: () => null, open_in_editor: () => null });
    const write = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText: write } });
    const items = launchItems("C:/work/repo", "Assets/Lobby.unity", vi.fn());
    expect(items.map((i) => i.label)).toEqual([
      "Show in Explorer",
      "Open Windows Terminal in its folder",
      "Open in VS Code",
      "Copy path",
    ]);
    for (const i of items) {
      i.run();
    }
    await vi.waitFor(() => expect(calls).toHaveLength(3));
    expect(calls).toEqual([
      { cmd: "reveal_path", args: { root: "C:/work/repo", path: "Assets/Lobby.unity" } },
      { cmd: "open_terminal", args: { root: "C:/work/repo", path: "Assets/Lobby.unity", terminal: "windowsTerminal" } },
      { cmd: "open_in_editor", args: { root: "C:/work/repo", path: "Assets/Lobby.unity", editor: { kind: "vsCode" } } },
    ]);
    expect(write).toHaveBeenCalledWith("C:/work/repo\\Assets\\Lobby.unity");
  });

  it("follows the settings, and reports the backend's refusal", async () => {
    mockCommands({
      open_in_editor: () => {
        throw "Rider isn't installed; pick another editor in Settings › Tools";
      },
    });
    setTerminal("gitBash");
    setEditor("rider");
    const onerror = vi.fn();
    const items = launchItems("C:/work/repo", null, onerror);
    expect(items.map((i) => i.label)).toEqual(["Show in Explorer", "Open Git Bash here", "Open in Rider"]);
    items[2].run();
    await vi.waitFor(() => expect(onerror).toHaveBeenCalledWith("Rider isn't installed; pick another editor in Settings › Tools"));
  });

  it("can't open a file that isn't on disk in the editor", () => {
    const items = launchItems("C:/r", "gone.cs", vi.fn(), { onDisk: false });
    expect(items.find((i) => i.label.startsWith("Open in"))?.disabled).toBe(true);
  });

  it("names a custom editor by its program", () => {
    setEditor("custom", '"C:\\Program Files\\Notepad++\\notepad++.exe"');
    expect(editorName()).toBe("notepad++");
    expect(chosenEditor()).toEqual({ kind: "custom", program: prefs.editorProgram });
    setEditor("custom", "");
    expect(editorName()).toBe("editor");
  });
});
