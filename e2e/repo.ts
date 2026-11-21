import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

/** A throwaway repository for e2e tests; mirrors gitgud-core's `TestRepo`. */
export class TestRepo {
  readonly path = mkdtempSync(join(tmpdir(), "gitgud-e2e-repo-"));

  constructor() {
    this.git("init", "-q", "-b", "main");
    for (const [key, value] of [
      ["user.name", "gitgud tests"],
      ["user.email", "tests@gitgud.invalid"],
      ["core.autocrlf", "false"],
      ["commit.gpgsign", "false"],
    ]) {
      this.git("config", key, value);
    }
  }

  git(...args: string[]): string {
    return execFileSync("git", args, { cwd: this.path, encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" } });
  }

  write(rel: string, contents: string): this {
    const file = join(this.path, rel);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, contents);
    return this;
  }

  commitAll(message: string): this {
    this.git("add", "-A");
    this.git("commit", "-q", "--allow-empty", "-m", message);
    return this;
  }

  remove() {
    rmSync(this.path, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
}
