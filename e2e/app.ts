import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium, type Browser, type Page } from "playwright-core";

const EXE = join(import.meta.dirname, "..", "target", "debug", process.platform === "win32" ? "gitgud.exe" : "gitgud");
const CDP_PORT = 9333;
const CONNECT_TIMEOUT_MS = 30_000;

export interface App {
  page: Page;
  close(): Promise<void>;
}

/**
 * Starts the debug build (`pnpm tauri build --debug --no-bundle`) with WebView2's
 * DevTools port open and a throwaway profile, and attaches Playwright to its window.
 */
export async function launchApp(env: Record<string, string> = {}): Promise<App> {
  if (process.platform !== "win32") {
    throw new Error("e2e attaches over WebView2 CDP, which only exists on Windows");
  }
  const profile = mkdtempSync(join(tmpdir(), "gitgud-e2e-"));
  const child = spawn(EXE, [], {
    env: {
      ...process.env,
      GITGUD_LIBRARY: join(profile, "library.json"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}`,
      WEBVIEW2_USER_DATA_FOLDER: profile,
      ...env,
    },
    stdio: "inherit",
  });
  let browser: Browser | undefined;
  try {
    browser = await connect(child);
    const page = await firstPage(browser);
    return {
      page,
      async close() {
        await browser?.close();
        killTree(child);
        rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
      },
    };
  } catch (err) {
    await browser?.close();
    killTree(child);
    rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
    throw err;
  }
}

async function connect(child: ChildProcess): Promise<Browser> {
  const deadline = Date.now() + CONNECT_TIMEOUT_MS;
  let lastErr: unknown;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`gitgud exited with code ${child.exitCode} before the window opened`);
    }
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);
    } catch (err) {
      lastErr = err;
      await new Promise((r) => setTimeout(r, 250));
    }
  }
  throw new Error(`no WebView2 DevTools on port ${CDP_PORT} after ${CONNECT_TIMEOUT_MS} ms`, { cause: lastErr });
}

async function firstPage(browser: Browser): Promise<Page> {
  const context = browser.contexts()[0];
  if (!context) {
    throw new Error("WebView2 exposed no browser context");
  }
  const page = context.pages()[0] ?? (await context.waitForEvent("page"));
  await page.waitForLoadState("domcontentloaded");
  return page;
}

function killTree(child: ChildProcess) {
  if (child.exitCode === null && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  }
}
