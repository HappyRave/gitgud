import assert from "node:assert/strict";
import { test } from "node:test";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 30_000 };

function box(id: number, name: string, x: number, z: number): string[] {
  return [
    `--- !u!1 &${id}`,
    "GameObject:",
    `  m_Name: ${name}`,
    "  m_IsActive: 1",
    "  m_Component:",
    `  - component: {fileID: ${id + 1}}`,
    `  - component: {fileID: ${id + 2}}`,
    `  - component: {fileID: ${id + 3}}`,
    `--- !u!4 &${id + 1}`,
    "Transform:",
    `  m_GameObject: {fileID: ${id}}`,
    "  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}",
    `  m_LocalPosition: {x: ${x}, y: 0, z: ${z}}`,
    "  m_LocalScale: {x: 1, y: 1, z: 1}",
    "  m_Father: {fileID: 0}",
    `--- !u!33 &${id + 2}`,
    "MeshFilter:",
    `  m_GameObject: {fileID: ${id}}`,
    "  m_Mesh: {fileID: 10202, guid: 0000000000000000e000000000000000, type: 0}",
    `--- !u!23 &${id + 3}`,
    "MeshRenderer:",
    `  m_GameObject: {fileID: ${id}}`,
    "  m_Materials:",
    "  - {fileID: 10303, guid: 0000000000000000f000000000000000, type: 0}",
  ];
}

/** A Unity YAML file of `count` boxes on a grid, the first one moved by `shift`. */
function boxes(count: number, shift: number): string {
  const out = ["%YAML 1.1", "%TAG !u! tag:unity3d.com,2011:"];
  for (let i = 0; i < count; i++) {
    out.push(...box(100 + i * 10, `Box_${i}`, (i % 50) * 2 + (i === 0 ? shift : 0), Math.floor(i / 50) * 2));
  }
  return `${out.join("\n")}\n`;
}

test("a big scene is built once per side: the card's warm-up and the opening share it", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo
    .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.12f1\n")
    .write("Assets/Gun.prefab", boxes(3, 0))
    .write("Assets/Scenes/Big.unity", boxes(2_100, 0))
    .commitAll("scene");
  repo.write("Assets/Gun.prefab", boxes(3, 1)).write("Assets/Scenes/Big.unity", boxes(2_100, 3)).commitAll("Move things");

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  let sceneInfos = 0;
  page.context().on("request", (r) => {
    if (r.url().includes("model_info") && (r.postData() ?? "").includes("Big.unity")) {
      sceneInfos++;
    }
  });
  await page.getByText("Move things", { exact: true }).click(WAIT);
  const files = page.getByRole("region", { name: "Commit" }).getByRole("list", { name: "Changed files" });

  await files.getByRole("button", { name: /Gun\.prefab/ }).click();
  await page.getByRole("application", { name: "3D preview of Assets/Gun.prefab" }).waitFor(WAIT);
  await files.getByRole("button", { name: /Big\.unity/ }).click();
  await page.getByRole("button", { name: "Open in 3D" }).waitFor(WAIT);
  await page.waitForTimeout(1500);
  await page.getByRole("button", { name: "Open in 3D" }).click();
  await page.getByRole("application", { name: "3D preview of Assets/Scenes/Big.unity" }).getByText(" meshes · ", { exact: false }).waitFor(WAIT);

  assert.equal(sceneInfos, 2, "the card's warm-up and the opening share one build per side");
});
