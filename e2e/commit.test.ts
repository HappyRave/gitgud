import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import type { Page } from "playwright-core";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");

/** Row actions show on hover, as a mouse user would reveal them. */
async function rowAction(page: Page, group: string, label: string) {
  const button = page.getByRole("list", { name: group }).getByRole("button", { name: label });
  await page.getByRole("list", { name: group }).getByRole("listitem").filter({ has: page.getByRole("button", { name: label }) }).hover();
  await button.click();
}

const subjects = (page: Page) => page.locator(".subject .text").allInnerTexts();

test("stage a file, commit it, and see it in the graph", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");
  repo.write("a.txt", "a2\n").write("notes.txt", "keep untracked\n");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByRole("list", { name: "Changes" }).waitFor({ timeout: 10_000 });

  await rowAction(page, "Changes", "Stage a.txt");
  await page.getByRole("list", { name: "Staged" }).waitFor();
  assert.equal(repo.git("diff", "--cached", "--name-only").trim(), "a.txt");

  await page.getByRole("textbox", { name: "Commit message" }).fill("Second commit\n\nfrom the real window");
  await page.getByRole("button", { name: "Commit 1 file" }).click();

  await page.getByText("Second commit", { exact: true }).waitFor({ timeout: 10_000 });
  assert.deepEqual(await subjects(page), ["Uncommitted changes", "Second commit", "first"]);
  assert.equal(repo.git("log", "-1", "--format=%B").trim(), "Second commit\n\nfrom the real window");
  assert.equal(await page.getByRole("textbox", { name: "Commit message" }).inputValue(), "");
  assert.equal(await page.getByRole("list", { name: "Staged" }).count(), 0);
  assert.match(await page.getByRole("list", { name: "Untracked" }).innerText(), /notes\.txt/);

  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "commit.png") });
});

test("a pre-commit hook rejection shows git's message and keeps the commit message", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");
  writeFileSync(join(repo.path, ".git/hooks/pre-commit"), "#!/bin/sh\necho 'lint: no trailing spaces' >&2\nexit 1\n");
  repo.write("a.txt", "a \n").git("add", "a.txt");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const box = page.getByRole("textbox", { name: "Commit message" });
  await box.waitFor({ timeout: 10_000 });
  await box.fill("will be rejected");
  await box.press("Control+Enter");

  const alert = page.getByRole("alert");
  await alert.waitFor({ timeout: 10_000 });
  assert.equal(await alert.locator(".exp").innerText(), "lint: no trailing spaces");
  assert.equal(await box.inputValue(), "will be rejected");
  assert.equal(repo.git("rev-list", "--count", "HEAD").trim(), "1");
});

test("stage one hunk of a file, commit it, keep the other hunk", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  const lines = Array.from({ length: 40 }, (_, i) => `line ${i + 1}`);
  repo.write("f.txt", lines.join("\n") + "\n").commitAll("first");
  lines[2] = "line three";
  lines[36] = "line thirty-seven";
  repo.write("f.txt", lines.join("\n") + "\n");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page
    .getByRole("list", { name: "Changes" })
    .getByRole("button", { name: "f.txt", exact: true })
    .click({ timeout: 10_000 });
  const diff = page.getByRole("region", { name: "Unstaged changes in f.txt" });
  const stageButtons = diff.getByRole("button", { name: /^Stage hunk/ });
  await stageButtons.first().waitFor();
  assert.equal(await stageButtons.count(), 2);

  await stageButtons.first().click();
  await page.getByRole("list", { name: "Staged" }).waitFor();
  assert.deepEqual(
    repo.git("diff", "--cached", "-U0").match(/^[-+][^-+].*$/gm),
    ["-line 3", "+line three"],
  );
  // The diff on screen now holds only the hunk that is left.
  await diff.getByRole("button", { name: /^Stage hunk/ }).nth(1).waitFor({ state: "detached" });

  await page.getByRole("textbox", { name: "Commit message" }).fill("Only the first hunk");
  await page.getByRole("button", { name: "Commit 1 file" }).click();
  await page.getByText("Only the first hunk", { exact: true }).waitFor({ timeout: 10_000 });
  assert.deepEqual(repo.git("diff", "-U0").match(/^[-+][^-+].*$/gm), ["-line 37", "+line thirty-seven"]);

  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "hunk.png") });
});

test("unstage everything, then the commit button is disabled", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").write("b.txt", "b\n").commitAll("first");
  repo.write("a.txt", "a2\n").write("b.txt", "b2\n").git("add", "-A");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("button", { name: "Unstage all (Staged)" }).click({ timeout: 10_000 });
  await page.getByRole("list", { name: "Changes" }).waitFor();
  assert.equal(repo.git("diff", "--cached", "--name-only").trim(), "");
  await page.getByRole("textbox", { name: "Commit message" }).fill("nothing staged");
  assert.equal(await page.getByRole("button", { name: "Commit", exact: true }).isDisabled(), true);
});
