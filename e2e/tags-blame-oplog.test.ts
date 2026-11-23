import assert from "node:assert/strict";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 10_000 };

test("create annotated tag, inspect file history & blame, and open op-log", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("player.cs", "class Player { int hp = 1; }\n").commitAll("init player");
  repo.write("player.cs", "class Player { int hp = 10; }\n").commitAll("buff player hp");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("listbox", { name: "Commits" }).waitFor(WAIT);

  // Create annotated tag via Sidebar -> TagDialog.
  await page.getByRole("region", { name: "Tags" }).getByRole("button", { name: "New tag" }).click();
  const tagDlg = page.getByRole("dialog", { name: "New tag" });
  await tagDlg.waitFor(WAIT);
  await page.getByLabel("Tag name").fill("v1.0.0");
  await page.getByLabel("Annotation message").fill("first release");
  await tagDlg.getByRole("button", { name: /Create tag/ }).click();
  await tagDlg.waitFor({ state: "detached", ...WAIT });

  await page.getByRole("region", { name: "Tags" }).getByText("v1.0.0", { exact: true }).waitFor(WAIT);
  assert.match(repo.git("tag", "-n1"), /v1\.0\.0\s+first release/);

  // Inspect commit -> File History & Blame.
  await page.getByRole("listbox", { name: "Commits" }).getByText("buff player hp", { exact: true }).click(WAIT);
  const commitPane = page.getByRole("region", { name: "Commit" });
  await commitPane.waitFor(WAIT);
  await commitPane.getByRole("button", { name: "History", exact: true }).click();

  const inspector = page.getByLabel("File history and blame for player.cs");
  await inspector.waitFor(WAIT);
  await inspector.getByRole("list", { name: "File commits" }).getByText("init player", { exact: true }).waitFor(WAIT);
  await inspector.getByRole("button", { name: "Blame", exact: true }).click();
  await inspector.getByRole("table", { name: "Blame lines" }).getByText("class Player", { exact: false }).waitFor(WAIT);
  await inspector.getByRole("button", { name: "Close file inspector" }).click();
  await inspector.waitFor({ state: "detached", ...WAIT });

  // Modify and stage player.cs so OpLog records an action, then open Op Log via Ctrl+3.
  repo.write("player.cs", "class Player { int hp = 20; }\n");
  const changes = page.getByRole("list", { name: "Changes" });
  await changes.waitFor(WAIT);
  const stageBtn = changes.getByRole("button", { name: "Stage player.cs", exact: true });
  await changes.getByRole("listitem").filter({ has: page.getByRole("button", { name: "Stage player.cs", exact: true }) }).hover();
  await stageBtn.click();

  await page.keyboard.press("Control+3");
  const actions = page.getByRole("list", { name: "Actions" });
  await actions.waitFor(WAIT);
  await actions.getByText(/Stage player\.cs/).waitFor(WAIT);
  await page.getByRole("region", { name: "Op log" }).getByText(/Outside gitgud \(reflog\)/).waitFor(WAIT);
});
