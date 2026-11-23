import assert from "node:assert/strict";
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

test("start page shows the git binary found by the Rust core", async () => {
  const line = app.page.getByTestId("git-info");
  await line.waitFor({ timeout: 10_000 });
  const text = await line.innerText();
  assert.match(text, /^git \d+\.\d+\.\d+(?: \(bundled\))? .+git/i);

  mkdirSync(SHOTS, { recursive: true });
  await app.page.screenshot({ path: join(SHOTS, "start.png") });
});
