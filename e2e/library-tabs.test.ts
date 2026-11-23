import assert from "node:assert/strict";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 10_000 };

test("library folder, repo add/filter/open tab, command palette, shortcut sheet, and settings", async (t) => {
  const alpha = new TestRepo();
  const beta = new TestRepo();
  t.after(() => alpha.remove());
  t.after(() => beta.remove());
  alpha.write("a.txt", "a\n").commitAll("alpha init");
  beta.write("b.txt", "b\n").commitAll("beta init");

  const app = await launchApp();
  t.after(() => app.close());
  const { page } = app;

  const main = page.getByRole("region", { name: "Repositories" });
  await main.waitFor(WAIT);

  // Create a nested Library folder via the sidebar UI.
  await page.getByRole("navigation", { name: "Library navigation" }).getByRole("button", { name: "+ new" }).click();
  const folderInput = page.getByLabel("New folder name");
  await folderInput.fill("Studio");
  await folderInput.press("Enter");
  await page.getByRole("navigation", { name: "Library navigation" }).getByText("Studio", { exact: true }).waitFor(WAIT);

  // Register both test repos without triggering the native OS folder picker.
  await page.evaluate(
    async ({ alphaPath, betaPath }) => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } })
        .__TAURI_INTERNALS__.invoke;
      await invoke("library_add_repos", {
        entries: [
          { path: alphaPath, folderPath: "Studio", name: "alpha-game" },
          { path: betaPath, folderPath: "Studio", name: "beta-tools" },
        ],
      });
    },
    { alphaPath: alpha.path, betaPath: beta.path },
  );

  // Reload so the Library store picks up the IPC-added repos without triggering OS dialogs.
  await page.reload();
  await main.waitFor(WAIT);
  await page.getByRole("button", { name: /All repositories/ }).click();
  await page.getByText("alpha-game", { exact: true }).waitFor(WAIT);
  await page.getByText("beta-tools", { exact: true }).waitFor(WAIT);

  const filter = page.getByLabel("Filter repositories");
  await filter.fill("alpha");
  assert.equal(await main.getByText("alpha-game", { exact: true }).count(), 1);
  assert.equal(await main.getByText("beta-tools", { exact: true }).count(), 0);
  await filter.press("Enter");

  // Repo opens in a workspace tab.
  await page.getByRole("listbox", { name: "Commits" }).waitFor(WAIT);

  // Command palette (Ctrl+K).
  await page.keyboard.press("Control+k");
  const pal = page.getByRole("dialog", { name: "Command palette" });
  await pal.waitFor(WAIT);
  await page.getByLabel("Filter commands").fill("settings");
  await page.keyboard.press("Escape");
  await pal.waitFor({ state: "detached", ...WAIT });

  // Shortcut sheet (?) with 2-step Escape: clear filter first, then close.
  await page.keyboard.press("?");
  const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await sheet.waitFor(WAIT);
  const sFilter = page.getByLabel("Filter shortcuts");
  await sFilter.fill("stage");
  assert.equal(await sFilter.inputValue(), "stage");
  await page.keyboard.press("Escape");
  await sheet.waitFor(WAIT);
  assert.equal(await sFilter.inputValue(), "");
  await page.keyboard.press("Escape");
  await sheet.waitFor({ state: "detached", ...WAIT });

  // Settings modal (Ctrl+,) with 2-column section rail and diagnostics.
  await page.keyboard.press("Control+,");
  const settings = page.getByRole("dialog", { name: "Settings" });
  await settings.waitFor(WAIT);
  await settings.getByRole("navigation", { name: "Settings sections" }).waitFor(WAIT);
  await settings.getByRole("button", { name: "Close" }).click();
  await settings.waitFor({ state: "detached", ...WAIT });
});
