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

test("start page shows gitgud's own CPU, RAM and caches", async () => {
  const stats = app.page.getByTestId("app-stats");
  await stats.getByText(/^\d+%$/).waitFor({ timeout: 10_000 });
  const text = await stats.innerText();
  assert.match(text, /CPU\s*\d+%\s*RAM\s*\d+(\.\d)? (MB|GB)/);
  assert.equal(await app.page.getByRole("alert").count(), 0, "git was found");

  mkdirSync(SHOTS, { recursive: true });
  await app.page.screenshot({ path: join(SHOTS, "start.png") });
});
