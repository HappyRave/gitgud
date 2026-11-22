import assert from "node:assert/strict";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

test("rebase onto a branch, resolve the stopped step, continue, then undo it all", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "1\n2\n3\n").commitAll("base");
  repo.git("checkout", "-q", "-b", "feature");
  repo.write("a.txt", "1\nF\n3\n").commitAll("f1");
  repo.write("b.txt", "b\n").commitAll("f2");
  repo.git("checkout", "-q", "main");
  repo.write("a.txt", "1\nM\n3\n").commitAll("m1");
  repo.git("checkout", "-q", "feature");
  const before = repo.git("rev-parse", "feature").trim();

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("region", { name: "Branches" }).getByRole("button", { name: /main/ }).click({ button: "right", timeout: 10_000 });
  await page.getByRole("menuitem", { name: "Rebase feature onto main…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText("Step 1", { exact: false }).waitFor();
  await dialog.getByRole("button", { name: /Rebase/ }).click();

  const banner = page.getByRole("region", { name: "Rebase in progress" });
  await banner.getByText("step 1 of 2", { exact: false }).waitFor({ timeout: 10_000 });
  await page.getByText("Conflict 1 of 1").waitFor();
  await page.keyboard.press("Alt+ArrowDown");
  await page.keyboard.press("Control+Enter");

  await page.getByRole("button", { name: "Continue rebase" }).click({ timeout: 10_000 });
  await page.getByRole("status").getByText("Rebase finished", { exact: false }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("log", "--format=%s", "main..feature").trim(), "f2\nf1");
  assert.equal(repo.git("show", "feature:a.txt"), "1\nM\nF\n3\n");

  for (let i = 0; i < 3; i++) {
    await page.keyboard.press("Control+z");
    await page.waitForTimeout(500);
  }
  await page.getByRole("status").getByText("Undid: Rebase onto main", { exact: false }).waitFor({ timeout: 10_000 });
  assert.equal(repo.git("rev-parse", "feature").trim(), before);
  assert.equal(repo.git("symbolic-ref", "HEAD").trim(), "refs/heads/feature");
});
