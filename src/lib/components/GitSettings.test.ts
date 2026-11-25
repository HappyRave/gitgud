import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import type { GitChoice, GitInstall, GitInventory } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import GitSettings from "./GitSettings.svelte";

const install = (over: Partial<GitInstall> = {}): GitInstall => ({
  path: "C:\\Program Files\\Git\\cmd\\git.exe",
  version: "2.55.0.windows.2",
  error: null,
  lfsVersion: "3.7.0",
  lfsOk: true,
  credentialHelpers: ["manager"],
  credentialManager: "2.8.0",
  ...over,
});

const SYSTEM = install();
const EMBEDDED = install({ path: "C:\\gitgud\\resources\\git\\cmd\\git.exe", version: "2.56.1.windows.1", lfsVersion: "3.8.1" });

function inventory(choice: GitChoice, over: Partial<GitInventory> = {}): GitInventory {
  const active =
    choice.mode === "embedded"
      ? { path: EMBEDDED.path, version: "2.56.1", source: "bundled" as const }
      : choice.mode === "custom"
        ? { path: choice.path, version: "2.47.1", source: "custom" as const }
        : { path: SYSTEM.path.toLowerCase(), version: "2.55.0", source: "system" as const };
  return {
    choice,
    active,
    activeError: null,
    system: SYSTEM,
    embedded: EMBEDDED,
    custom: choice.mode === "custom" ? install({ path: choice.path, version: "2.47.1", credentialManager: null }) : null,
    minGit: "2.45.2",
    settingsPath: "C:\\Users\\Max\\AppData\\Roaming\\com.happyrave.gitgud\\git.json",
    ...over,
  };
}

const card = (name: string) => screen.getByLabelText(`${name} git`);

describe("GitSettings", () => {
  it("shows both gits side by side and marks the one in use", async () => {
    mockCommands({ git_inventory: () => inventory({ mode: "auto" }) });
    render(GitSettings, {});

    expect(await screen.findByText("2.55.0.windows.2")).toBeInTheDocument();
    expect(within(card("System")).getByText("in use")).toBeInTheDocument();
    expect(within(card("Embedded")).queryByText("in use")).not.toBeInTheDocument();
    expect(within(card("Embedded")).getByText("2.56.1.windows.1")).toBeInTheDocument();
    expect(within(card("System")).getByText("GCM 2.8.0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/Auto picks the system git when it's 2.45.2 or newer/)).toBeInTheDocument();
    expect(screen.getByText(/the gitgud CLI and MCP server use it too/)).toBeInTheDocument();
  });

  it("switches to the embedded git and to a picked custom one", async () => {
    const calls = mockCommands({
      git_inventory: () => inventory({ mode: "auto" }),
      git_set_choice: ({ choice }) => inventory(choice),
      "plugin:dialog|open": () => "D:\\tools\\git\\cmd\\git.exe",
    });
    render(GitSettings, {});
    await screen.findByText("2.55.0.windows.2");

    await fireEvent.click(screen.getByRole("button", { name: "Embedded" }));
    expect(await within(card("Embedded")).findByText("in use")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Embedded" })).toHaveAttribute("aria-pressed", "true");

    await fireEvent.click(screen.getByRole("button", { name: "Custom…" }));
    expect(within(await screen.findByLabelText("Custom git")).getByText("in use")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "git_set_choice").map((c) => c.args)).toEqual([
      { choice: { mode: "embedded" } },
      { choice: { mode: "custom", path: "D:\\tools\\git\\cmd\\git.exe" } },
    ]);
  });

  it("keeps the old choice and shows git's reason when a switch fails", async () => {
    mockCommands({
      git_inventory: () => inventory({ mode: "auto" }),
      git_set_choice: () => {
        throw "git was not found on PATH";
      },
    });
    render(GitSettings, {});
    await screen.findByText("2.55.0.windows.2");
    await fireEvent.click(screen.getByRole("button", { name: "System" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("git was not found on PATH");
    expect(screen.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "true");
  });

  it("says when no git can run", async () => {
    mockCommands({
      git_inventory: () =>
        inventory({ mode: "system" }, { active: null, activeError: "git was not found on PATH", system: null }),
    });
    render(GitSettings, {});
    expect(await screen.findByRole("alert")).toHaveTextContent("No usable git: git was not found on PATH");
    expect(within(card("System")).getByText("No git on PATH.")).toBeInTheDocument();
    expect(within(card("System")).getByRole("button", { name: "Install Git for Windows" })).toBeInTheDocument();
  });

  it("checks for updates and updates the system git after a confirm", async () => {
    const calls = mockCommands({
      git_inventory: () => inventory({ mode: "auto" }),
      git_latest: () => ({ version: "2.56.1.windows.1", systemBehind: true, embeddedBehind: false }),
      git_update_system: () => ({ kind: "installerStarted", message: "" }),
    });
    render(GitSettings, {});
    await screen.findByText("2.55.0.windows.2");

    await fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await within(card("System")).findByText("update available")).toBeInTheDocument();
    expect(within(card("Embedded")).queryByText("update available")).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Update system git…" }));
    expect(calls.some((c) => c.cmd === "git_update_system")).toBe(false);
    expect(screen.getByText(/open Git Bash windows close/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Update" }));
    expect(await screen.findByText(/installer is running/)).toBeInTheDocument();
  });

  it("checks the open repo's remote and compares config on demand", async () => {
    const calls = mockCommands({
      git_inventory: () => inventory({ mode: "auto" }),
      git_check_remote: ({ id }) => ({
        remote: `origin-${id}`,
        url: "git@example.com:a/b.git",
        ok: false,
        elapsedMs: 80,
        message: "Permission denied (publickey).",
      }),
      git_config_compare: () => [
        { scope: "system", key: "http.sslbackend", system: ["openssl"], embedded: ["schannel"], differs: true },
        { scope: "global", key: "user.name", system: ["Max"], embedded: ["Max"], differs: false },
      ],
    });
    const { container } = render(GitSettings, { activeRepoId: 3 });
    await screen.findByText("2.55.0.windows.2");

    await fireEvent.click(screen.getByRole("button", { name: "Check remote access" }));
    expect(await screen.findByText(/Permission denied \(publickey\)/)).toBeInTheDocument();
    expect(screen.getByText("origin-3")).toBeInTheDocument();

    expect(calls.some((c) => c.cmd === "git_config_compare")).toBe(false);
    const details = container.querySelector("details")!;
    details.open = true;
    await fireEvent(details, new Event("toggle"));
    expect(await screen.findByText("http.sslbackend")).toBeInTheDocument();
    expect(screen.getByText(/1 differ/)).toBeInTheDocument();
    expect(screen.queryByText("user.name")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByLabelText("Show all keys"));
    expect(screen.getByText("user.name")).toBeInTheDocument();
  });
});
