import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 10_000 };
const ROOT = join(import.meta.dirname, "..");
const CLI_BIN = join(ROOT, "target", "debug", process.platform === "win32" ? "gitgud-cli.exe" : "gitgud-cli");

function runCli(repoPath: string, args: string[]): string {
  if (existsSync(CLI_BIN)) {
    return execFileSync(CLI_BIN, ["--repo", repoPath, "--json", ...args], {
      cwd: repoPath,
      encoding: "utf8",
    });
  }
  return execFileSync("cargo", ["run", "--quiet", "-p", "gitgud-cli", "--", "--repo", repoPath, "--json", ...args], {
    cwd: ROOT,
    encoding: "utf8",
  });
}

test("agent proposal inbox review, feedback, approval, and stacked branches", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.write("main.txt", "base\n").commitAll("base");
  repo.git("checkout", "-q", "-b", "stack-a");
  repo.write("a.txt", "a\n").commitAll("stack level a");
  repo.git("checkout", "-q", "-b", "stack-b");
  repo.write("b.txt", "b\n").commitAll("stack level b");

  // Create an isolated hidden-ref agent proposal via gitgud-cli.
  repo.write("agent.txt", "proposed line\n");
  const raw = runCli(repo.path, ["propose", "-m", "feat: agent patch", "--why", "tests pass", "--agent", "claude", "--path", "agent.txt"]);
  const created = JSON.parse(raw) as { id: string; status: string };
  assert.equal(created.status, "pending");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  // Sidebar displays Stacked branches and Inbox sections.
  await page.getByRole("region", { name: "Stacks" }).getByText("stack-b", { exact: false }).waitFor(WAIT);
  const inbox = page.getByRole("region", { name: "Inbox" });
  await inbox.waitFor(WAIT);
  await inbox.getByRole("button", { name: /feat: agent patch/ }).click();

  const review = page.getByRole("region", { name: "Proposal review" });
  await review.waitFor(WAIT);

  // Request changes first and confirm persisted status.
  await page.getByLabel("Change request feedback").fill("add edge test");
  await review.getByRole("button", { name: "Request changes" }).click();
  await review.getByLabel("Review comments").getByText("add edge test", { exact: true }).waitFor(WAIT);

  // Edit commit message and approve onto stack-b.
  await page.getByLabel("Proposal commit message").fill("feat: reviewed agent patch");
  await review.getByRole("button", { name: "Approve & commit" }).click();
  await page.getByRole("status").getByText(/Proposal approved and committed/i).waitFor(WAIT);

  assert.equal(repo.git("log", "-1", "--format=%s").trim(), "feat: reviewed agent patch");
});
