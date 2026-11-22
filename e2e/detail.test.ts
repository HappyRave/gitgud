import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");

test("clicking a commit shows its message, files and diff", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  const lines = Array.from({ length: 30 }, (_, i) => `line ${i + 1}`);
  repo.write("src/app.txt", lines.join("\n") + "\n").write("README.md", "# demo\n").commitAll("first");
  lines[4] = "line five";
  lines[24] = "line twenty-five";
  repo.write("src/app.txt", lines.join("\n") + "\n").write("NEW.md", "hello\n").commitAll("Tweak lines\n\nTwo edits, far apart.");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByText("Tweak lines", { exact: true }).click({ timeout: 10_000 });

  const detail = page.getByRole("region", { name: "Commit" });
  await detail.getByRole("heading", { name: "Tweak lines" }).waitFor();
  assert.match(await detail.innerText(), /Two edits, far apart\./);
  assert.deepEqual(await detail.getByRole("list", { name: "Changed files" }).getByRole("button").allInnerTexts(), [
    "ANEW.md",
    "Msrc/app.txt",
  ]);

  await detail.getByRole("button", { name: /src\/app\.txt/ }).click();
  const diff = page.getByRole("region", { name: "Diff of src/app.txt" });
  await diff.waitFor();
  const headers = (await diff.locator("tr.hunk").allInnerTexts()).map((h) => h.trim());
  const cli = repo.git("diff", "--diff-algorithm=histogram", "HEAD~1", "HEAD", "--", "src/app.txt");
  assert.deepEqual(headers, cli.match(/^@@ [^@]+ @@/gm));
  assert.deepEqual(await diff.locator("tr.added .text").allInnerTexts(), ["+line five", "+line twenty-five"]);

  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, "detail.png") });

  await page.getByRole("listbox", { name: "Commits" }).press("Escape");
  await detail.waitFor({ state: "detached" });
  await page.getByRole("form", { name: "Commit" }).waitFor();
});
