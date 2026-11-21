import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { launchApp, type App } from "./app.ts";

const SHOTS = join(import.meta.dirname, "out");

let app: App;

before(async () => {
  app = await launchApp();
});

after(async () => {
  await app?.close();
});

test("start page shows the system git found by the Rust core", async () => {
  const expected = execFileSync("git", ["--version"], { encoding: "utf8" }).match(/(\d+)\.(\d+)\.(\d+)/)?.[0];
  assert.ok(expected, "could not read the version of the git on PATH");

  const line = app.page.getByTestId("git-info");
  await line.waitFor({ timeout: 10_000 });
  assert.match(await line.innerText(), new RegExp(`^git ${expected.replaceAll(".", "\\.")} .+git`, "i"));

  mkdirSync(SHOTS, { recursive: true });
  await app.page.screenshot({ path: join(SHOTS, "start.png") });
});
