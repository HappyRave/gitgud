import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { prefs } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import SettingsModal from "./SettingsModal.svelte";

describe("SettingsModal", () => {
  it("loads diagnostics and identity and saves repo-local or global config", async () => {
    const calls = mockCommands({
      config_get_identity: ({ id }) =>
        id != null ? { userName: "Local Max", userEmail: "local@example.com" } : { userName: "Global Max", userEmail: "global@example.com" },
      config_set_identity: () => null,
      tool_diagnostics: () => ({ gitPath: "C:/Git/git.exe", gitVersion: "2.56.1", lfsVersion: "git-lfs/3.6.0" }),
    });

    render(SettingsModal, { activeRepoId: 7, activeRepoName: "boat", onclose: vi.fn() });

    expect(await screen.findByText("2.56.1")).toBeInTheDocument();
    expect(screen.getByText("git-lfs/3.6.0")).toBeInTheDocument();
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
  });
});
