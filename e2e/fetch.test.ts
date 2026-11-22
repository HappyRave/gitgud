import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const posix = (p: string) => p.replaceAll("\\", "/");

test("Fetch brings a commit pushed elsewhere into the graph", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");
  const remotes = mkdtempSync(join(tmpdir(), "gitgud-e2e-remote-"));
  t.after(() => rmSync(remotes, { recursive: true, force: true }));
  const bare = join(remotes, "origin.git");
  repo.git("clone", "-q", "--bare", ".", bare);
  repo.git("remote", "add", "origin", bare);
  repo.git("fetch", "-q", "origin");

  const other = new TestRepo();
  t.after(() => other.remove());
  other.git("remote", "add", "origin", bare);
  other.git("fetch", "-q", "origin");
  other.git("checkout", "-q", "-B", "main", "origin/main");
  other.write("b.txt", "b\n").commitAll("pushed from another clone");
  other.git("push", "-q", "origin", "main");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByText("first", { exact: true }).waitFor({ timeout: 10_000 });

  await page.getByRole("button", { name: "Fetch" }).click();

  await page.getByRole("status").filter({ hasText: "Fetched" }).waitFor({ timeout: 10_000 });
  const row = page.getByRole("option").filter({ hasText: "pushed from another clone" });
  await row.waitFor();
  assert.deepEqual(await row.locator(".label").allInnerTexts(), ["origin/main"]);
});

test("Cancel stops a hanging fetch and every process it started", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("a.txt", "a\n").commitAll("first");
  const pids = join(repo.path, "pids.txt");
  const script = join(repo.path, "hang.sh");
  writeFileSync(script, `cat /proc/$$/winpid > '${posix(pids)}'\nsleep 60 &\ncat /proc/$!/winpid >> '${posix(pids)}'\nwait\n`);
  repo.git("config", "protocol.ext.allow", "always");
  repo.git("remote", "add", "origin", `ext::sh ${posix(script)}`);

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.getByText("first", { exact: true }).waitFor({ timeout: 10_000 });

  await page.keyboard.press("Control+Shift+F");
  await page.getByLabel("Fetch progress").waitFor();
  let started: string[] = [];
  for (let i = 0; i < 100 && started.length < 2; i++) {
    await page.waitForTimeout(100);
    try {
      started = readFileSync(pids, "utf8").split(/\s+/).filter(Boolean);
    } catch {
      // not written yet
    }
  }
  assert.equal(started.length, 2, "the hanging remote started");

  await page.getByLabel("Fetch progress").getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("status").filter({ hasText: "Fetch cancelled" }).waitFor({ timeout: 5_000 });
  await page.waitForTimeout(300);
  for (const pid of started) {
    const listed = execFileSync("tasklist", ["/FI", `PID eq ${pid}`, "/NH"], { encoding: "utf8" });
    assert.ok(!listed.includes(pid), `process ${pid} survived the cancel`);
  }
});
