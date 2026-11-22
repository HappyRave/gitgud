import assert from "node:assert/strict";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

/** Spec §6: a change outside gitgud is visible within 300 ms on small repos. */
const TARGET_MS = 300;

test("files changed outside gitgud show up without a refresh button", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write(".gitignore", "Library/\n").write("Assets/a.txt", "a\n").commitAll("first");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByText("Nothing to commit, working tree clean.").waitFor({ timeout: 10_000 });

  const t0 = performance.now();
  repo.write("Assets/new.txt", "outside\n");
  await page.getByRole("list", { name: "Untracked" }).getByText("Assets/new.txt").waitFor({ timeout: 5_000 });
  const ms = performance.now() - t0;
  console.log(`outside edit visible after ${ms.toFixed(0)} ms`);
  assert.ok(ms <= TARGET_MS, `took ${ms.toFixed(0)} ms`);

  // The WIP row appears on top of the graph as the tree turned dirty.
  await page.locator(".subject .text").first().filter({ hasText: "Uncommitted changes" }).waitFor({ timeout: 5_000 });

  // Ignored folders churn without effect.
  repo.write("Library/cache.bin", "noise");
  await page.waitForTimeout(500);
  assert.equal(await page.getByText("Library/cache.bin").count(), 0);
});

test("a commit made in a terminal appears in the graph", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByText("first", { exact: true }).waitFor({ timeout: 10_000 });

  repo.write("a.txt", "b\n").commitAll("from the terminal");
  repo.git("tag", "v1");

  await page.getByText("from the terminal", { exact: true }).waitFor({ timeout: 5_000 });
  const first = page.getByRole("listbox", { name: "Commits" }).getByRole("option").first();
  await first.getByText("v1", { exact: true }).waitFor({ timeout: 5_000 });
  assert.deepEqual(await first.locator(".label").allInnerTexts(), ["main", "v1"]);
});
