import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");

/** `main` and `other` both edit line 2 of a.txt and change bin.dat. */
function diverged() {
  const repo = new TestRepo();
  repo.write("a.txt", "1\n2\n3\n").write("bin.dat", "\0base").commitAll("base");
  repo.git("checkout", "-q", "-b", "other");
  repo.write("a.txt", "1\ntheirs\n3\n").write("bin.dat", "\0theirs").commitAll("theirs");
  repo.git("checkout", "-q", "main");
  repo.write("a.txt", "1\nyours\n3\n").write("bin.dat", "\0yours").commitAll("yours");
  return repo;
}

test("merge a branch with conflicts, resolve text and binary, commit, then undo", async (t) => {
  const repo = diverged();
  t.after(() => repo.remove());
  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  const branches = page.getByRole("region", { name: "Branches" });
  await branches.getByRole("button", { name: /other/ }).click({ button: "right", timeout: 10_000 });
  await page.getByRole("menuitem", { name: "Merge other into main…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText("2 files will conflict", { exact: false }).waitFor();
  await dialog.getByRole("button", { name: /Merge/ }).click();

  const banner = page.getByRole("region", { name: "Merge in progress" });
  await banner.getByText("2 of 2 files left", { exact: false }).waitFor({ timeout: 10_000 });
  assert.ok(repo.git("rev-parse", "-q", "--verify", "MERGE_HEAD").trim());

  await page.getByText("Conflict 1 of 1").waitFor();
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "merge-text.png") });
  await page.keyboard.press("Alt+ArrowDown");
  await page.keyboard.press("Control+Enter");

  const pick = page.getByRole("region", { name: "Pick a side for bin.dat" });
  await pick.waitFor({ timeout: 10_000 });
  await page.screenshot({ path: join(SHOTS, "merge-binary.png") });
  await page.keyboard.press("2");

  const message = page.getByRole("textbox", { name: "Merge commit message" });
  await message.waitFor({ timeout: 10_000 });
  assert.match(await message.inputValue(), /- a\.txt: use both\n- bin\.dat: take theirs/);
  await page.screenshot({ path: join(SHOTS, "merge-summary.png") });
  await banner.getByRole("button", { name: "Commit merge" }).click();

  await page.getByRole("status").getByText("Merge committed", { exact: false }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("show", "HEAD:a.txt"), "1\nyours\ntheirs\n3\n");
  assert.equal(repo.git("rev-list", "--parents", "-n1", "HEAD").trim().split(" ").length, 3);

  await page.keyboard.press("Control+z");
  await banner.getByText("all 2 files resolved", { exact: false }).waitFor({ timeout: 10_000 });
  assert.ok(repo.git("rev-parse", "-q", "--verify", "MERGE_HEAD").trim());
});

test("abort goes back to before and Ctrl+Z brings the merge back", async (t) => {
  const repo = diverged();
  t.after(() => repo.remove());
  const before = repo.git("rev-parse", "HEAD").trim();
  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("region", { name: "Branches" }).getByRole("button", { name: /other/ }).click({ button: "right", timeout: 10_000 });
  await page.getByRole("menuitem", { name: "Merge other into main…" }).click();
  await page.getByRole("dialog").getByRole("button", { name: /Merge/ }).click();
  const banner = page.getByRole("region", { name: "Merge in progress" });
  await banner.waitFor({ timeout: 10_000 });

  await banner.getByRole("button", { name: "Abort, back to before" }).click();
  await banner.waitFor({ state: "detached", timeout: 10_000 });
  assert.equal(repo.git("rev-parse", "HEAD").trim(), before);
  assert.equal(repo.git("status", "--porcelain"), "");

  await page.keyboard.press("Control+z");
  await banner.waitFor({ timeout: 10_000 });
});
