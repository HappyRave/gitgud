import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import type { Page } from "playwright-core";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const SHOTS = join(import.meta.dirname, "out");
const WAIT = { timeout: 10_000 };

/** `feature` (HEAD): "Add camera rgiht" (cam.txt), "Engine", "fixup! Engine" on top of `main`'s base. */
function stack() {
  const repo = new TestRepo();
  repo.write("base.txt", "base\n").commitAll("base");
  repo.git("checkout", "-q", "-b", "feature");
  repo.write("cam.txt", "cam\n").commitAll("Add camera rgiht");
  repo.write("engine.txt", "1\n2\n3\n").commitAll("Engine");
  repo.write("engine.txt", "1\n2 smooth\n3\n").commitAll("fixup! Engine");
  return repo;
}

const subjects = (repo: TestRepo, range = "main..feature") => repo.git("log", "--format=%s", range).trim();

async function open(t: TestContext, repo: TestRepo) {
  t.after(() => repo.remove());
  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  const commits = page.getByRole("listbox", { name: "Commits" });
  await commits.waitFor(WAIT);
  return { page, commits };
}

/** Selects the graph row with `subject` and opens the plan from it with Shift+E. */
async function planFrom(page: Page, subject: string) {
  await page.getByRole("listbox", { name: "Commits" }).getByText(subject, { exact: true }).click(WAIT);
  await page.keyboard.press("Shift+E");
  await page.getByRole("region", { name: "Planning" }).waitFor(WAIT);
  return page.getByRole("listbox", { name: "Plan" });
}

const row = (page: Page, subject: string) => page.getByRole("listbox", { name: "Plan" }).getByRole("option").filter({ hasText: subject });
const status = (page: Page, text: string) => page.getByRole("status").getByText(text, { exact: false }).waitFor(WAIT);

async function shot(page: Page, name: string) {
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, name) });
}

test("plan: fixup applied on open, reword, dashed preview over faded commits, Start, Done, Undo and redo", async (t) => {
  const repo = stack();
  const before = repo.git("rev-parse", "feature").trim();
  const { page, commits } = await open(t, repo);

  await planFrom(page, "Add camera rgiht");
  await page.getByText("Planning a history edit of feature · nothing changes until Start", { exact: false }).waitFor();
  await row(page, "fixup! Engine").getByText('auto from "fixup!"', { exact: false }).waitFor();
  await page.getByText("Result: 3 commits → 2").waitFor(WAIT);

  await row(page, "Add camera rgiht").click();
  await page.keyboard.press("r");
  await page.getByRole("textbox", { name: /Message of/ }).fill("Add camera right");

  await commits.getByText("reworded").waitFor(WAIT);
  assert.equal(await commits.locator("li.ghost").count(), 2, "two after rows");
  assert.equal(await commits.locator("li.faded").count(), 3, "the three replaced commits fade");
  await page.getByText("✓ final files identical to now").waitFor(WAIT);
  await shot(page, "history-plan.png");

  await page.keyboard.press("Control+Enter");
  const done = page.getByRole("region", { name: "History edit done" });
  await done.getByText("Rebased feature · 3 commits → 2").waitFor(WAIT);
  await done.getByText("✓ Final files identical to before").waitFor();
  await done.getByText('Reworded', { exact: false }).waitFor();
  await shot(page, "history-done.png");
  assert.equal(subjects(repo), "Engine\nAdd camera right");
  assert.equal(repo.git("show", "feature:engine.txt"), "1\n2 smooth\n3\n");
  assert.equal(await page.getByRole("region", { name: "Planning" }).count(), 0);

  await done.getByRole("button", { name: /Undo rebase/ }).click();
  await status(page, "Undid: Rebase feature");
  assert.equal(repo.git("rev-parse", "feature").trim(), before);

  await page.keyboard.press("Control+y");
  await status(page, "Redid: Rebase feature");
  assert.equal(subjects(repo), "Engine\nAdd camera right");
});

test("plan: drop, squash with an edited message, and reorder by drag and Alt+↑", async (t) => {
  const repo = stack();
  repo.write("junk.txt", "junk\n").commitAll("junk");
  repo.write("docs.txt", "docs\n").commitAll("Add docs");
  const { page } = await open(t, repo);

  await planFrom(page, "Add camera rgiht");
  await page.getByText("Result: 5 commits → 4").waitFor(WAIT);

  await row(page, "junk").click();
  await page.keyboard.press("d");
  await row(page, "fixup! Engine").click();
  await page.keyboard.press("s");
  const message = page.getByRole("textbox", { name: /Message of/ });
  assert.equal(await message.inputValue(), "Engine\n\nfixup! Engine");
  await message.fill("Engine, smoothed");
  await page.keyboard.press("Escape");

  // Drag the oldest row onto "fixup! Engine", then Alt+↑ it twice to the top.
  await row(page, "Add camera rgiht").dragTo(row(page, "fixup! Engine"));
  await row(page, "Add camera rgiht").click();
  await page.keyboard.press("Alt+ArrowUp");
  await page.keyboard.press("Alt+ArrowUp");
  const order = await page.getByRole("listbox", { name: "Plan" }).getByRole("option").allInnerTexts();
  assert.deepEqual(
    order.map((t) => ["Add camera rgiht", "Add docs", "junk", "fixup! Engine", "Engine"].find((s) => t.includes(s))),
    ["Add camera rgiht", "Add docs", "junk", "fixup! Engine", "Engine"],
  );
  await page.getByText("Result: 5 commits → 3").waitFor(WAIT);
  await shot(page, "history-squash-drop-move.png");

  await page.getByRole("button", { name: /Start rebase/ }).click();
  await page.getByRole("region", { name: "History edit done" }).getByText("5 commits → 3").waitFor(WAIT);
  assert.equal(subjects(repo), "Add camera rgiht\nAdd docs\nEngine, smoothed");
  assert.equal(repo.git("ls-tree", "--name-only", "feature").includes("junk.txt"), false);
  assert.equal(repo.git("show", "feature:engine.txt"), "1\n2 smooth\n3\n");
});

test("plan: a predicted conflict onto another branch; Show the conflict, Keep the old base, Start anyway", async (t) => {
  const repo = new TestRepo();
  repo.write("a.txt", "1\n2\n3\n").commitAll("base");
  repo.git("checkout", "-q", "-b", "feature");
  repo.write("a.txt", "1\nF\n3\n").commitAll("one");
  repo.write("b.txt", "b\n").commitAll("two");
  repo.git("checkout", "-q", "main");
  repo.write("a.txt", "1\nM\n3\n").commitAll("m1");
  repo.git("checkout", "-q", "feature");
  const before = repo.git("rev-parse", "feature").trim();
  const { page, commits } = await open(t, repo);

  await planFrom(page, "one");
  await page.getByText("✓ no conflicts predicted").waitFor(WAIT);
  await page.getByRole("combobox", { name: "Onto" }).selectOption({ label: "main" });

  await page.getByText("1 conflict predicted at step 1", { exact: false }).waitFor(WAIT);
  await row(page, "one").getByText("step 1: a.txt would conflict with main").waitFor();
  await commits.getByText("⚠ conflict", { exact: false }).waitFor(WAIT);
  assert.equal(await page.getByRole("button", { name: /Start rebase/ }).count(), 0);

  await page.getByRole("button", { name: "Show the conflict" }).click();
  const shown = page.getByLabel("Predicted conflict in a.txt");
  await shown.waitFor(WAIT);
  assert.match(await shown.innerText(), /<<<<<<<[\s\S]*M[\s\S]*=======[\s\S]*F[\s\S]*>>>>>>>/);
  await shot(page, "history-conflict.png");

  await page.getByRole("button", { name: "Keep the old base" }).click();
  await page.getByText("✓ no conflicts predicted").waitFor(WAIT);

  await page.getByRole("combobox", { name: "Onto" }).selectOption({ label: "main" });
  await page.getByRole("button", { name: "Start anyway" }).click(WAIT);
  const banner = page.getByRole("region", { name: "Rebase in progress" });
  await banner.getByText("step 1 of 2", { exact: false }).waitFor(WAIT);
  await page.getByText("Conflict 1 of 1").waitFor(WAIT);

  await banner.getByRole("button", { name: "Abort, back to before" }).click();
  await status(page, "Rebase aborted");
  assert.equal(repo.git("rev-parse", "feature").trim(), before);
});

test("plan: Update dependent branches moves them, and Ctrl+Z moves them back", async (t) => {
  const repo = stack();
  repo.git("branch", "dep", "HEAD~1");
  const dep = repo.git("rev-parse", "dep").trim();
  const { page } = await open(t, repo);

  await planFrom(page, "Add camera rgiht");
  await row(page, "Add camera rgiht").click();
  await page.keyboard.press("r");
  await page.getByRole("textbox", { name: /Message of/ }).fill("Add camera right");
  await page.getByRole("checkbox", { name: /Update dependent branches \(1\)/ }).check();
  await page.getByText("✓ no conflicts predicted").waitFor(WAIT);
  await page.keyboard.press("Control+Enter");

  await page.getByRole("region", { name: "History edit done" }).waitFor(WAIT);
  assert.equal(subjects(repo, "main..dep"), "Engine\nAdd camera right");

  await page.keyboard.press("Control+z");
  await status(page, "Undid: Rebase feature");
  assert.equal(repo.git("rev-parse", "dep").trim(), dep);
});

test("plan: an Edit step stops with the banner; staging and Continue amend the commit", async (t) => {
  const repo = stack();
  const { page } = await open(t, repo);

  await planFrom(page, "Add camera rgiht");
  await row(page, "Add camera rgiht").click();
  await page.keyboard.press("e");
  await row(page, "Add camera rgiht").getByText("stops here so you can change the commit").waitFor();
  await page.getByText("✓ no conflicts predicted").waitFor(WAIT);
  await page.keyboard.press("Control+Enter");

  const banner = page.getByRole("region", { name: "Rebase in progress" });
  await banner.getByText("to edit Add camera rgiht", { exact: false }).waitFor(WAIT);
  await page.getByRole("listbox", { name: "Commits" }).waitFor();
  await shot(page, "history-edit-stop.png");
  repo.write("cam.txt", "cam, edited\n");
  repo.git("add", "cam.txt");
  await banner.getByRole("button", { name: "Continue" }).click();
  await status(page, "Rebase finished");
  assert.equal(repo.git("show", "feature~1:cam.txt"), "cam, edited\n");
  assert.equal(subjects(repo), "Engine\nAdd camera rgiht");
});

test("plan: R on an older commit opens it preset to Reword; Ctrl+Z is off while planning; Esc cancels", async (t) => {
  const repo = stack();
  const { page, commits } = await open(t, repo);

  // Something to undo, so a Ctrl+Z that got through would show.
  await commits.getByText("fixup! Engine", { exact: true }).click(WAIT);
  await page.keyboard.press("f");
  await status(page, 'Folded "fixup! Engine" into its parent');
  const folded = repo.git("rev-parse", "HEAD").trim();

  await commits.getByText("Add camera rgiht", { exact: true }).click();
  await page.keyboard.press("r");
  const message = page.getByRole("textbox", { name: /Message of/ });
  await message.waitFor(WAIT);
  assert.equal(await message.inputValue(), "Add camera rgiht");
  assert.equal(await row(page, "Add camera rgiht").getAttribute("aria-selected"), "true");

  // The plan opens with focus on its list.
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(500);
  await page.getByRole("region", { name: "Planning" }).waitFor();
  assert.equal(repo.git("rev-parse", "HEAD").trim(), folded, "Ctrl+Z undid nothing while planning");

  await page.getByRole("listbox", { name: "Plan" }).press("Escape");
  await page.getByRole("region", { name: "Planning" }).waitFor({ state: "detached", ...WAIT });
  assert.equal(await commits.locator("li.ghost").count(), 0);
  await commits.getByText("Engine", { exact: true }).waitFor(WAIT);
});

test("quick edits: F from the row menu, R renames the tip inline, D drops, Ctrl+Z steps back", async (t) => {
  const repo = stack();
  const { page, commits } = await open(t, repo);

  await commits.getByText("fixup! Engine", { exact: true }).click({ button: "right", ...WAIT });
  await page.getByRole("menuitem", { name: /Fixup into parent/ }).click();
  await status(page, 'Folded "fixup! Engine" into its parent');
  assert.equal(subjects(repo), "Engine\nAdd camera rgiht");
  assert.equal(repo.git("show", "feature:engine.txt"), "1\n2 smooth\n3\n");

  await commits.getByText("Engine", { exact: true }).click(WAIT);
  await page.keyboard.press("r");
  const input = page.getByRole("textbox", { name: "New commit subject" });
  await input.fill("Engine, smoothed");
  await shot(page, "history-rename.png");
  await input.press("Enter");
  await status(page, "Reworded");
  assert.equal(repo.git("log", "-1", "--format=%s").trim(), "Engine, smoothed");

  await commits.getByText("Add camera rgiht", { exact: true }).click(WAIT);
  await page.keyboard.press("d");
  await status(page, 'Dropped "Add camera rgiht"');
  assert.equal(subjects(repo), "Engine, smoothed");
  assert.equal(repo.git("ls-tree", "--name-only", "feature").includes("cam.txt"), false);

  await page.keyboard.press("Control+z");
  await status(page, "Undid: Drop");
  assert.equal(subjects(repo), "Engine, smoothed\nAdd camera rgiht");
  await page.keyboard.press("Control+z");
  await status(page, "Undid: Reword");
  await page.keyboard.press("Control+z");
  await status(page, "Undid: Fixup");
  assert.equal(subjects(repo), "fixup! Engine\nEngine\nAdd camera rgiht");
});

test("refusals: history with a merge commit, and the first commit, say why", async (t) => {
  const repo = new TestRepo();
  repo.write("a.txt", "a\n").commitAll("root");
  repo.git("checkout", "-q", "-b", "side");
  repo.write("s.txt", "s\n").commitAll("side work");
  repo.git("checkout", "-q", "main");
  repo.git("merge", "-q", "--no-ff", "-m", "Merge side", "side");
  repo.write("b.txt", "b\n").commitAll("after");
  const head = repo.git("rev-parse", "HEAD").trim();
  const { page, commits } = await open(t, repo);

  await commits.getByText("side work", { exact: true }).click(WAIT);
  await page.keyboard.press("Shift+E");
  await page.getByRole("alert").getByText("history with merge commits can't be edited yet", { exact: false }).waitFor(WAIT);
  assert.equal(await page.getByRole("region", { name: "Planning" }).count(), 0);

  await commits.getByText("Merge side", { exact: true }).click();
  await page.keyboard.press("d");
  await page.getByRole("alert").getByText("merge commits", { exact: false }).waitFor(WAIT);

  await commits.getByText("root", { exact: true }).click();
  await page.keyboard.press("Shift+E");
  await page.getByRole("alert").getByText("the first commit can't be edited yet", { exact: false }).waitFor(WAIT);
  assert.equal(repo.git("rev-parse", "HEAD").trim(), head);
});
