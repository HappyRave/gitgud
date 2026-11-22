import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

test("Ctrl+Z undoes a commit and a stage, Ctrl+Y redoes them", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");
  const first = repo.git("rev-parse", "HEAD").trim();
  repo.write("a.txt", "changed\n");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("button", { name: "Stage all (Changes)" }).click({ timeout: 10_000 });
  await page.getByRole("textbox", { name: "Commit message" }).fill("to be undone");
  await page.getByRole("button", { name: "Commit 1 file" }).click();
  await page.getByText("to be undone", { exact: true }).waitFor({ timeout: 10_000 });

  // Focus leaves the text field, as after clicking the graph.
  await page.getByRole("listbox", { name: "Commits" }).focus();
  await page.keyboard.press("Control+z");
  await page.getByRole("status").filter({ hasText: 'Undid: Commit "to be undone"' }).waitFor();
  await page.getByText("to be undone", { exact: true }).waitFor({ state: "detached" });
  assert.equal(repo.git("rev-parse", "HEAD").trim(), first);
  assert.equal(repo.git("diff", "--cached", "--name-only").trim(), "a.txt");

  await page.keyboard.press("Control+z");
  await page.getByRole("status").filter({ hasText: "Undid: Stage a.txt" }).waitFor();
  assert.equal(repo.git("diff", "--cached", "--name-only").trim(), "");

  await page.keyboard.press("Control+y");
  await page.keyboard.press("Control+Shift+Z");
  await page.getByText("to be undone", { exact: true }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("log", "-1", "--format=%s").trim(), "to be undone");
});

test("Discard throws an edit away and Ctrl+Z brings it back", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "original\n").commitAll("first");
  repo.write("a.txt", "an edit worth keeping\n").write("scratch.txt", "untracked notes\n");
  const read = (p: string) => readFileSync(join(repo.path, p), "utf8");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const changes = page.getByRole("list", { name: "Changes" });
  await changes.getByRole("listitem").first().hover({ timeout: 10_000 });
  await changes.getByRole("button", { name: "Discard a.txt" }).click();
  await page.getByRole("list", { name: "Changes" }).waitFor({ state: "detached" });
  assert.equal(read("a.txt"), "original\n");

  const untracked = page.getByRole("list", { name: "Untracked" });
  await untracked.getByRole("listitem").first().hover();
  await untracked.getByRole("button", { name: "Discard scratch.txt" }).click();
  await untracked.waitFor({ state: "detached" });
  assert.throws(() => read("scratch.txt"));

  await page.getByRole("listbox", { name: "Commits" }).focus();
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  await page.getByRole("status").filter({ hasText: "Undid: Discard a.txt" }).waitFor({ timeout: 10_000 });
  assert.equal(read("a.txt"), "an edit worth keeping\n");
  assert.equal(read("scratch.txt"), "untracked notes\n");
});

test("the op log panel lists actions and restores an old discard", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").write("b.txt", "b\n").commitAll("first");
  repo.write("a.txt", "work I will regret discarding\n").write("b.txt", "b2\n");
  const read = (p: string) => readFileSync(join(repo.path, p), "utf8");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const changes = page.getByRole("list", { name: "Changes" });
  await changes.getByRole("listitem").first().hover({ timeout: 10_000 });
  await changes.getByRole("button", { name: "Discard a.txt" }).click();
  await changes.getByRole("listitem").first().hover();
  await changes.getByRole("button", { name: "Stage b.txt" }).click();
  await page.getByRole("list", { name: "Staged" }).waitFor();
  assert.equal(read("a.txt"), "a\n");

  await page.getByRole("listbox", { name: "Commits" }).focus();
  await page.keyboard.press("Control+3");
  const panel = page.getByRole("region", { name: "Op log" });
  await panel.waitFor();
  assert.deepEqual(await panel.locator("li .t").allInnerTexts(), ["Stage b.txt", "Discard a.txt"]);

  await panel.getByRole("button", { name: /Discard a\.txt/ }).click();
  await panel.getByRole("button", { name: "Restore these files" }).click();
  await panel.locator("li .t").first().filter({ hasText: 'Restore files from "Discard a.txt"' }).waitFor({ timeout: 10_000 });
  assert.equal(read("a.txt"), "work I will regret discarding\n");
  assert.equal(repo.git("diff", "--cached", "--name-only").trim(), "b.txt", "the later stage stayed");

  await page.screenshot({ path: join(import.meta.dirname, "out", "oplog.png") });
});

test("double-clicking a branch checks it out and Ctrl+Z switches back", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "main version\n").commitAll("on main");
  repo.git("checkout", "-q", "-b", "feature/login");
  repo.write("a.txt", "login version\n").write("login.txt", "new\n").commitAll("on login");
  repo.git("checkout", "-q", "main");
  const read = (p: string) => readFileSync(join(repo.path, p), "utf8");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const branches = page.getByRole("region", { name: "Branches" });
  await branches.getByRole("button", { name: /login/ }).dblclick({ timeout: 10_000 });
  await branches.getByRole("button", { name: /login/ }).filter({ hasText: "✓" }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("branch", "--show-current").trim(), "feature/login");
  assert.equal(read("a.txt"), "login version\n");

  await page.getByRole("listbox", { name: "Commits" }).focus();
  await page.keyboard.press("Control+z");
  await page.getByRole("status").filter({ hasText: "Undid: Check out feature/login" }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("branch", "--show-current").trim(), "main");
  assert.equal(read("a.txt"), "main version\n");
  assert.throws(() => read("login.txt"));

  await page.screenshot({ path: join(import.meta.dirname, "out", "sidebar.png") });
});
