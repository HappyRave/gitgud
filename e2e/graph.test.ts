import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");

test("draws the history of a repo with branches, a merge and tags", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("base");
  repo.git("tag", "v0.1");
  repo.git("checkout", "-q", "-b", "feature");
  repo.write("f.txt", "1\n").commitAll("feature one");
  repo.write("f.txt", "2\n").commitAll("feature two");
  repo.git("checkout", "-q", "main");
  repo.write("m.txt", "m\n").commitAll("main work");
  repo.git("merge", "-q", "--no-ff", "-m", "Merge feature", "feature");
  repo.git("checkout", "-q", "-b", "wip-branch");
  repo.write("w.txt", "w\n").commitAll("unmerged work");
  repo.write("a.txt", "dirty\n");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const options = page.getByRole("listbox", { name: "Commits" }).getByRole("option");
  await options.first().waitFor({ timeout: 10_000 });
  const subjects = await page.locator(".subject .text").allInnerTexts();
  const expected = repo.git("log", "--topo-order", "--branches", "--tags", "HEAD", "--format=%s").trim().split("\n");
  assert.deepEqual(subjects, ["Uncommitted changes", ...expected]);

  const labels = (row: number) => options.nth(row).locator(".label").allInnerTexts();
  assert.deepEqual(await labels(1), ["wip-branch"]);
  assert.deepEqual(await labels(2), ["main"]);
  assert.deepEqual(await labels(subjects.length - 1), ["v0.1"]);

  await options.nth(3).click();
  assert.equal(await options.nth(3).getAttribute("aria-selected"), "true");

  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "graph.png") });
});

test("a 10k-commit history: first rows within 300 ms, scrolling fetches on demand", async (t) => {
  const repo = new TestRepo().generate(10_000);
  t.after(() => repo.remove());
  repo.git("commit-graph", "write", "--reachable");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByRole("listbox", { name: "Commits" }).getByRole("option").first().waitFor({ timeout: 10_000 });

  // The calls the page makes on open, timed inside the webview: walk + layout, then the first chunk.
  const ms = await page.evaluate(async (path) => {
    const invoke = (window as any).__TAURI_INTERNALS__.invoke;
    const t0 = performance.now();
    const opened = await invoke("open_repo", { path });
    await invoke("graph_rows", { id: opened.id, start: 0, count: 200 });
    const elapsed = performance.now() - t0;
    await invoke("close_repo", { id: opened.id });
    return elapsed;
  }, repo.path);
  console.log(`open + first 200 rows of 10k commits: ${ms.toFixed(1)} ms`);
  assert.ok(ms <= 300, `took ${ms.toFixed(1)} ms`);

  await page.locator(".viewport").evaluate((el) => (el.scrollTop = el.scrollHeight));
  await page.getByText("commit 1", { exact: true }).waitFor({ timeout: 10_000 });
});
