import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");

test("opens GITGUD_OPEN and lists its working copy", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").write("b.txt", "b\n").commitAll("init");
  repo.write("a.txt", "a2\n");
  repo.write("b.txt", "b2\n").git("add", "b.txt");
  repo.write("new dir/ünï.txt", "n\n");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByTestId("head").waitFor({ timeout: 10_000 });
  assert.equal(await page.getByTestId("head").innerText(), "main");
  assert.deepEqual(await page.getByRole("list", { name: "Staged" }).getByRole("listitem").allInnerTexts(), ["Mb.txt"]);
  assert.deepEqual(await page.getByRole("list", { name: "Changes" }).getByRole("listitem").allInnerTexts(), ["Ma.txt"]);
  assert.deepEqual(await page.getByRole("list", { name: "Untracked" }).getByRole("listitem").allInnerTexts(), [
    "?new dir/ünï.txt",
  ]);

  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "status.png") });
});

test("explains a folder that is not a repository", async (t) => {
  const plain = mkdtempSync(join(tmpdir(), "gitgud-e2e-plain-"));
  t.after(() => rmSync(plain, { recursive: true, force: true }));

  const app = await launchApp({ GITGUD_OPEN: plain });
  t.after(() => app.close());

  const alert = app.page.getByRole("alert");
  await alert.waitFor({ timeout: 10_000 });
  assert.match(await alert.innerText(), /is not inside a git repository$/);
});
