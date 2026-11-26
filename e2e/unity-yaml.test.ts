import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 10_000 };

function prefab(hp: string, speed: string, guid: string): string {
  return [
    "%YAML 1.1",
    "--- !u!1 &1",
    "GameObject:",
    "  m_Name: Enemy",
    "--- !u!4 &2",
    "Transform:",
    "  m_GameObject: {fileID: 1}",
    "  m_Father: {fileID: 0}",
    "--- !u!114 &3",
    "MonoBehaviour:",
    "  m_GameObject: {fileID: 1}",
    `  m_Script: {fileID: 11500000, guid: ${guid}, type: 3}`,
    `  hp: ${hp}`,
    `  speed: ${speed}`,
    "",
  ].join("\n");
}

test("unity repo setup, structural prefab diff, and a prefab conflict merged by property", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  const guid = "deadbeefdeadbeefdeadbeefdeadbeef";
  repo
    .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.12f1\n")
    .write("ProjectSettings/EditorSettings.asset", "%YAML 1.1\n--- !u!159 &1\nEditorSettings:\n  m_SerializationMode: 2\n")
    .write("Assets/Scripts/EnemyAI.cs", "class EnemyAI {}")
    .write("Assets/Scripts/EnemyAI.cs.meta", `fileFormatVersion: 2\nguid: ${guid}\n`)
    .write("Assets/Prefabs/Enemy.prefab", prefab("50", "4", guid))
    .commitAll("initial enemy");

  repo.git("checkout", "-q", "-b", "feature");
  repo.write("Assets/Prefabs/Enemy.prefab", prefab("80", "9", guid)).commitAll("feature tuning");

  repo.git("checkout", "-q", "main");
  repo.write("Assets/Prefabs/Enemy.prefab", prefab("75", "4", guid)).commitAll("main tuning");
  repo.write("Assets/Prefabs/Enemy.prefab", prefab("75", "6", guid));

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  // Unstaged prefab renders structural Unity diff with hierarchy and script resolution.
  const changes = page.getByRole("list", { name: "Changes" });
  await changes.waitFor(WAIT);
  await changes.getByRole("button", { name: "Assets/Prefabs/Enemy.prefab", exact: true }).click();
  await page.getByRole("radiogroup", { name: "View" }).getByRole("radio", { name: "Structure" }).click();
  const udiff = page.getByLabel("Structural Unity diff for Assets/Prefabs/Enemy.prefab");
  await udiff.waitFor(WAIT);
  await page.getByLabel("Filter objects or properties").fill("EnemyAI");
  await udiff.getByText("speed", { exact: true }).waitFor(WAIT);

  // Settings -> Unity Project Setup fixes missing .gitignore / .gitattributes rules.
  await page.keyboard.press("Control+,");
  const settings = page.getByRole("dialog", { name: "Settings" });
  await settings.waitFor(WAIT);
  const setupSec = settings.getByRole("region", { name: "Unity Project Setup" });
  await setupSec.waitFor(WAIT);
  await setupSec.getByRole("button", { name: "Fix Unity .gitignore / .gitattributes" }).click();
  await setupSec.getByText("Standard Unity ignores present ✓").waitFor(WAIT);
  await settings.getByRole("button", { name: "Close" }).click();

  // Discard unstaged prefab change so merge can start cleanly, then resolve via R4-A table.
  repo.git("checkout", "--", "Assets/Prefabs/Enemy.prefab");
  await changes.waitFor({ state: "detached", ...WAIT });

  await page.getByRole("region", { name: "Branches" }).getByRole("button", { name: /feature/ }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Merge feature into main…" }).click();
  await page.getByRole("dialog").getByRole("button", { name: /Merge/ }).click();

  const tableSec = page.getByLabel("Merge by property for Assets/Prefabs/Enemy.prefab");
  await tableSec.waitFor(WAIT);
  await tableSec.getByRole("table", { name: "Changes" }).getByText("hp", { exact: true }).waitFor(WAIT);
  await tableSec.getByRole("button", { name: /Pick all feature/ }).click();
  await page.keyboard.press("Control+Enter");

  const banner = page.getByRole("region", { name: "Merge in progress" });
  await banner.getByRole("button", { name: "Commit merge" }).click();
  await page.getByRole("status").getByText("Merge committed", { exact: false }).waitFor(WAIT);

  const merged = readFileSync(join(repo.path, "Assets/Prefabs/Enemy.prefab"), "utf8");
  assert.match(merged, /hp:\s*80/);
  assert.match(merged, /speed:\s*9/);
});

const LIGHTHOUSE = [
  "%YAML 1.1",
  "%TAG !u! tag:unity3d.com,2011:",
  "--- !u!1 &100",
  "GameObject:",
  "  m_Component:",
  "  - component: {fileID: 101}",
  "  - component: {fileID: 102}",
  "  m_Name: Lighthouse",
  "--- !u!4 &101",
  "Transform:",
  "  m_GameObject: {fileID: 100}",
  "  m_LocalPosition: {x: 0, y: 0, z: 0}",
  "  m_Children: []",
  "  m_Father: {fileID: 0}",
  "--- !u!108 &102",
  "Light:",
  "  m_GameObject: {fileID: 100}",
  "  m_Intensity: 1.2",
  "  m_Range: 24",
  "",
].join("\n");

test("an LFS scene conflict is merged by property and committed", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo.git("lfs", "install", "--local");
  repo
    .write(".gitattributes", "*.unity filter=lfs diff=lfs merge=lfs -text\n")
    .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.12f1\n")
    .write("Assets/Level.unity", LIGHTHOUSE)
    .commitAll("level");
  repo.git("checkout", "-q", "-b", "lighting");
  repo.write("Assets/Level.unity", LIGHTHOUSE.replace("m_Range: 24", "m_Range: 30")).commitAll("relight");
  repo.git("checkout", "-q", "main");
  repo.write("Assets/Level.unity", LIGHTHOUSE.replace("{x: 0, y: 0, z: 0}", "{x: 0, y: 0.35, z: 0}")).commitAll("raise");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;

  await page.getByRole("region", { name: "Branches" }).getByRole("button", { name: /lighting/ }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Merge lighting into main…" }).click();
  await page.getByRole("dialog").getByRole("button", { name: /Merge/ }).click();

  const offer = page.getByLabel("Merge by property", { exact: true });
  await offer.getByText("1 only main").waitFor(WAIT);
  await offer.getByText("0 clashes").waitFor(WAIT);
  await page.keyboard.press("m");

  const table = page.getByLabel("Merge by property for Assets/Level.unity");
  await table.getByText("m_LocalPosition.y", { exact: true }).waitFor(WAIT);
  await table.getByText("m_Range", { exact: true }).waitFor(WAIT);
  await table.getByText("No broken references", { exact: false }).waitFor(WAIT);
  await page.keyboard.press("Control+Enter");

  const banner = page.getByRole("region", { name: "Merge in progress" });
  await banner.getByRole("button", { name: "Commit merge" }).click();
  await page.getByRole("status").getByText("Merge committed", { exact: false }).waitFor(WAIT);

  const merged = readFileSync(join(repo.path, "Assets/Level.unity"), "utf8");
  assert.equal(merged, LIGHTHOUSE.replace("{x: 0, y: 0, z: 0}", "{x: 0, y: 0.35, z: 0}").replace("m_Range: 24", "m_Range: 30"));
  assert.match(repo.git("cat-file", "-p", "HEAD:Assets/Level.unity"), /^version https:\/\/git-lfs/);
});
