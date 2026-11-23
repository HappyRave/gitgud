import { test } from "node:test";
import { join } from "node:path";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 15_000 };

const b64 = (values: number[]) => Buffer.from(new Float32Array(values).buffer).toString("base64");

function gltf(x: number): string {
  return JSON.stringify({
    asset: { version: "2.0" },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ name: "Tri", mesh: 0, translation: [x, 0, 0] }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }],
    buffers: [{ byteLength: 36, uri: `data:application/octet-stream;base64,${b64([0, 0, 0, 1, 0, 0, 0, 1, 0])}` }],
    bufferViews: [{ buffer: 0, byteLength: 36 }],
    accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] }],
  });
}

function prefab(x: number): string {
  return [
    "%YAML 1.1",
    "%TAG !u! tag:unity3d.com,2011:",
    "--- !u!1 &1",
    "GameObject:",
    "  m_Name: Box",
    "  m_IsActive: 1",
    "  m_Component:",
    "  - component: {fileID: 4}",
    "  - component: {fileID: 33}",
    "  - component: {fileID: 23}",
    "--- !u!4 &4",
    "Transform:",
    "  m_GameObject: {fileID: 1}",
    "  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}",
    `  m_LocalPosition: {x: ${x}, y: 0, z: 0}`,
    "  m_LocalScale: {x: 1, y: 1, z: 1}",
    "  m_Father: {fileID: 0}",
    "--- !u!33 &33",
    "MeshFilter:",
    "  m_GameObject: {fileID: 1}",
    "  m_Mesh: {fileID: 10202, guid: 0000000000000000e000000000000000, type: 0}",
    "--- !u!23 &23",
    "MeshRenderer:",
    "  m_GameObject: {fileID: 1}",
    "  m_Materials:",
    "  - {fileID: 10303, guid: 0000000000000000f000000000000000, type: 0}",
    "",
  ].join("\n");
}

test("3D preview: glTF and prefab, compare modes, Changes legend", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  repo
    .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.12f1\n")
    .write("Assets/boat.gltf", gltf(0))
    .write("Assets/Box.prefab", prefab(0))
    .commitAll("models");
  repo.write("Assets/boat.gltf", gltf(1)).write("Assets/Box.prefab", prefab(2));

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  const changes = page.getByRole("list", { name: "Changes" });
  await changes.waitFor(WAIT);

  await changes.getByRole("button", { name: "Assets/boat.gltf", exact: true }).click();
  const gl = page.getByRole("application", { name: "3D preview of Assets/boat.gltf" });
  await gl.getByText("1 meshes", { exact: false }).waitFor(WAIT);
  await gl.getByText("On disk").waitFor(WAIT);
  await gl.getByRole("radio", { name: "Changes" }).click();
  await gl.getByRole("button", { name: /Tri.*moved 1\.00 m/ }).waitFor(WAIT);
  await gl.getByRole("radio", { name: "Swipe" }).click();
  await gl.getByRole("button", { name: "Drag to swipe" }).waitFor(WAIT);
  await gl.screenshot({ path: join("e2e", "out", "model-gltf.png") });

  await changes.getByRole("button", { name: "Assets/Box.prefab", exact: true }).click();
  const pf = page.getByRole("application", { name: "3D preview of Assets/Box.prefab" });
  await pf.getByText("12", { exact: true }).waitFor(WAIT);
  await pf.getByRole("button", { name: /Box.*moved 2\.00 m/ }).waitFor(WAIT);
  await pf.getByRole("radio", { name: "Onion" }).click();
  await pf.screenshot({ path: join("e2e", "out", "model-prefab.png") });
});
