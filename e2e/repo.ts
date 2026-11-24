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
    return execFileSync("git", args, {
      cwd: this.path,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
    });
  }

  /**
   * Writes `commits` commits to `main` with `git fast-import`: mostly linear,
   * with a 3-commit side branch merged back every 50 commits. Seconds, not
   * minutes, for 10k commits.
   */
  generate(commits: number): this {
    const lines: string[] = [];
    let mark = 0;
    let time = 1_700_000_000;
    const commit = (ref: string, msg: string, parents: number[]) => {
      mark++;
      time += 60;
      lines.push(`commit ${ref}`, `mark :${mark}`, `committer gitgud tests <tests@gitgud.invalid> ${time} +0000`);
      lines.push(`data ${Buffer.byteLength(msg)}`, msg);
      parents.forEach((p, i) => lines.push(`${i === 0 ? "from" : "merge"} :${p}`));
      lines.push(`M 644 inline f${mark % 97}.txt`, `data ${String(mark).length}`, String(mark), "");
      return mark;
    };
    let head = commit("refs/heads/main", "commit 1", []);
    let made = 1;
    while (made < commits) {
      if (made % 50 === 0 && commits - made >= 4) {
        let side = head;
        for (let i = 0; i < 3; i++) {
          side = commit(`refs/heads/side${made}`, `side ${made}.${i}`, [side]);
        }
        head = commit("refs/heads/main", `merge side ${made}`, [head, side]);
        made += 4;
      } else {
        head = commit("refs/heads/main", `commit ${made + 1}`, [head]);
        made++;
      }
    }
    execFileSync("git", ["fast-import", "--quiet"], { cwd: this.path, input: lines.join("\n") + "\n" });
    this.git("checkout", "-q", "-f", "main");
    return this;
  }

  write(rel: string, contents: string | Uint8Array): this {
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
