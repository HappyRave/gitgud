import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { crc32, deflateSync } from "node:zlib";
import type { Page } from "playwright-core";
import { launchApp } from "./app.ts";
import { TestRepo } from "./repo.ts";

const WAIT = { timeout: 20_000 };
const FONTS = "C:/Windows/Fonts";
const MEDIA = "C:/Windows/SystemApps/MicrosoftWindows.Client.CoreAI_cw5n1h2txyewy/DiscoveryOverlay/Assets/TutorialMode";
const MP4 = "C:/Windows/SystemApps/Microsoft.Windows.CloudExperienceHost_cw5n1h2txyewy/media/oobe-intro.mp4";

function wav(seconds: number, amplitude: number, hz: number): Buffer {
  const rate = 22_050;
  const n = Math.floor(rate * seconds);
  const out = Buffer.alloc(44 + n * 2);
  out.write("RIFF", 0);
  out.writeUInt32LE(36 + n * 2, 4);
  out.write("WAVEfmt ", 8);
  out.writeUInt32LE(16, 16);
  out.writeUInt16LE(1, 20);
  out.writeUInt16LE(1, 22);
  out.writeUInt32LE(rate, 24);
  out.writeUInt32LE(rate * 2, 28);
  out.writeUInt16LE(2, 32);
  out.writeUInt16LE(16, 34);
  out.write("data", 36);
  out.writeUInt32LE(n * 2, 40);
  for (let i = 0; i < n; i++) {
    const env = Math.min(1, i / 2000, (n - i) / 4000);
    out.writeInt16LE(Math.round(amplitude * env * 32767 * Math.sin((i * hz * 2 * Math.PI) / rate)), 44 + i * 2);
  }
  return out;
}

/** An RGBA PNG of `w` × `h` coloured by `px(x, y)`. */
function png(w: number, h: number, px: (x: number, y: number) => [number, number, number, number]): Buffer {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      raw.set(px(x, y), y * (w * 4 + 1) + 1 + x * 4);
    }
  }
  const chunk = (type: string, data: Buffer) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/** A PDF with one page of Helvetica text per entry. */
function pdf(pages: string[][]): Buffer {
  const objs: string[] = ["<< /Type /Catalog /Pages 2 0 R >>", ""];
  const kids: number[] = [];
  const font = 3;
  objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
  for (const lines of pages) {
    const content = lines.map((l, i) => `BT /F1 ${i === 0 ? 22 : 13} Tf 60 ${720 - i * 28} Td (${l}) Tj ET`).join("\n");
    objs.push(`<< /Length ${content.length} >>\nstream\n${content}\nendstream`);
    const contentId = objs.length;
    objs.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents ${contentId} 0 R /Resources << /Font << /F1 ${font} 0 R >> >> >>`);
    kids.push(objs.length);
  }
  objs[1] = `<< /Type /Pages /Kids [${kids.map((k) => `${k} 0 R`).join(" ")}] /Count ${kids.length} >>`;
  let out = "%PDF-1.4\n";
  const offs: number[] = [];
  objs.forEach((o, i) => {
    offs.push(out.length);
    out += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n${offs.map((o) => `${String(o).padStart(10, "0")} 00000 n \n`).join("")}`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`;
  return Buffer.from(out, "latin1");
}

const URP_LIT = "{fileID: 4800000, guid: 933532a4fcc9baf4fa0491de14d08ed7, type: 3}";
const material = (color: string, smoothness: number, keywords: string[]) =>
  `%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!21 &2100000\nMaterial:\n  m_Name: Hull\n  m_Shader: ${URP_LIT}\n  m_ValidKeywords:\n${keywords.map((k) => `  - ${k}\n`).join("")}  m_CustomRenderQueue: -1\n  m_SavedProperties:\n    m_TexEnvs:\n    - _BaseMap:\n        m_Texture: {fileID: 0}\n        m_Scale: {x: 1, y: 1}\n        m_Offset: {x: 0, y: 0}\n    m_Floats:\n    - _Metallic: 0\n    - _Smoothness: ${smoothness}\n    m_Colors:\n    - _BaseColor: ${color}\n`;

const anim = (peak: number, extra = "") =>
  `%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!74 &7400000\nAnimationClip:\n  m_Name: Bob\n  m_SampleRate: 60\n  m_EditorCurves:\n  - curve:\n      m_Curve:\n      - time: 0\n        value: 0\n        inSlope: 0\n        outSlope: 0\n      - time: 1\n        value: ${peak}\n        inSlope: 0\n        outSlope: 0\n      - time: 2\n        value: 0\n        inSlope: 0\n        outSlope: 0\n    attribute: m_LocalPosition.y\n    path: Hull\n${extra}  m_AnimationClipSettings:\n    m_StopTime: 2\n    m_LoopTime: 1\n`;

const FLAG = "  - curve:\n      m_Curve:\n      - time: 0\n        value: -10\n        inSlope: 0\n        outSlope: 0\n      - time: 1\n        value: 10\n        inSlope: 0\n        outSlope: 0\n      - time: 2\n        value: -10\n        inSlope: 0\n        outSlope: 0\n    attribute: localEulerAnglesRaw.z\n    path: Flag\n";

function scene(boxes: [string, number, number][]): string {
  const out = ["%YAML 1.1", "%TAG !u! tag:unity3d.com,2011:", "--- !u!104 &2", "RenderSettings:", "  m_Fog: 0"];
  boxes.forEach(([name, x, z], i) => {
    const id = 100 + i * 10;
    out.push(
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
    );
  });
  return `${out.join("\n")}\n`;
}

function spriteMeta(sprites: [string, number, number, number][]): string {
  const list = sprites
    .map(
      ([name, x, w, border]) =>
        `    - serializedVersion: 2\n      name: ${name}\n      rect:\n        serializedVersion: 2\n        x: ${x}\n        y: 0\n        width: ${w}\n        height: 32\n      alignment: 9\n      pivot: {x: 0.5, y: 0}\n      border: {x: ${border}, y: ${border}, z: ${border}, w: ${border}}\n`,
    )
    .join("");
  return `fileFormatVersion: 2\nguid: 5e5e0000000000000000000000000001\nTextureImporter:\n  spriteMode: 2\n  spriteSheet:\n    sprites:\n${list}`;
}

const icons = png(128, 32, (x, y) => {
  const cell = Math.floor(x / 32);
  const inside = (x % 32) - 16;
  const r = Math.hypot(inside, y - 16);
  const colors: [number, number, number][] = [
    [230, 80, 60],
    [60, 140, 230],
    [240, 190, 40],
    [70, 190, 120],
  ];
  return r < 12 ? [...colors[cell], 255] : [0, 0, 0, 0];
});

async function open(page: Page, path: string) {
  await page.getByRole("list", { name: "Changes" }).getByRole("button", { name: path, exact: true }).click();
}

const shot = (page: Page, name: string) => page.screenshot({ path: join("e2e", "out", `viewer-${name}.png`) });

test("R7 viewers on real files", async (t) => {
  const repo = new TestRepo();
  t.after(() => repo.remove());
  const media = existsSync(`${MEDIA}/TutorialModeFirstLast_Light.webm`);
  repo
    .write("ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000.0.12f1\n")
    .write("Assets/UI/Display.ttf", readFileSync(`${FONTS}/arial.ttf`))
    .write("Assets/Art/Hull.mat", material("{r: 0.8, g: 0.2, b: 0.15, a: 1}", 0.45, []))
    .write("Assets/Art/Hull.mat.meta", "guid: 3a7e0000000000000000000000000001\n")
    .write("Assets/Audio/Horn.wav", wav(2.4, 0.4, 220))
    .write("Assets/Data/BoatStats.csv", "id,name,speed,turn,cost\nskiff,Skiff,12,90,0\ntug,Harbour Tug,7,35,800\nraft,Raft,4,120,0\nbarge,Barge,5,20,300\n")
    .write("Docs/GDD.pdf", pdf([["Harbour level", "Overview of the east channel."], ["Harbour approach", "Two tugs patrol the outer buoys and turn back.", "Docking takes 40 seconds."], ["Fuel", "Pickups sit on the north pier."]]))
    .write("Assets/UI/HUD.png", icons)
    .write("Assets/UI/HUD.png.meta", spriteMeta([["anchor", 0, 32, 0], ["wheel", 32, 32, 0], ["raft", 64, 32, 0]]))
    .write("Assets/Anims/Bob.anim", anim(0.25))
    .write("Assets/Anims/Bob.anim.meta", "guid: ab000000000000000000000000000001\n")
    .write("Assets/Scenes/Harbour.unity", scene([["Pier", 0, 0], ["Crane", 3, 0], ["Shed", -3, 2], ["Old_Buoy", 2, 4]]));
  if (media) {
    repo.write("Assets/Video/Tutorial.webm", readFileSync(`${MEDIA}/TutorialModeFirstLast_Light.webm`));
  }
  repo.commitAll("assets");

  repo
    .write("Assets/UI/Display.ttf", readFileSync(`${FONTS}/arialbd.ttf`))
    .write("Assets/Art/Hull.mat", material("{r: 0.15, g: 0.3, b: 0.8, a: 1}", 0.62, ["_NORMALMAP"]))
    .write("Assets/Audio/Horn.wav", wav(2.85, 0.8, 220))
    .write("Assets/Data/BoatStats.csv", "id,name,speed,turn,cost\ntug,Harbour Tug,7,42,650\nskiff,Skiff,13.5,90,0\nbarge,Barge,5,20,300\nfoil,Hydrofoil,22,60,2400\n")
    .write("Docs/GDD.pdf", pdf([["Harbour level", "Overview of the east channel."], ["Harbour approach", "Two tugs patrol the outer buoys and follow the player.", "Docking takes 55 seconds."], ["Map", "A new page with the harbour map."], ["Fuel", "Pickups sit on the north pier."]]))
    .write("Assets/UI/HUD.png.meta", spriteMeta([["anchor", 0, 40, 6], ["wheel", 48, 32, 0], ["fuel", 96, 32, 0]]))
    .write("Assets/Anims/Bob.anim", anim(0.5, FLAG))
    .write("Assets/Scenes/Harbour.unity", scene([["Pier", 0, 0], ["Crane", 5, 0], ["Shed", -3, 2], ["New_Buoy", -2, -4]]));
  if (media) {
    repo.write("Assets/Video/Tutorial.webm", readFileSync(`${MEDIA}/TutorialModeFirstLast_Dark.webm`));
  }
  if (existsSync(MP4)) {
    repo.write("Assets/Video/Intro.mp4", readFileSync(MP4));
  }

  const app = await launchApp({ GITGUD_OPEN: repo.path });
  t.after(() => app.close());
  const { page } = app;
  await page.setViewportSize({ width: 1600, height: 1000 }).catch(() => {});
  await page.getByRole("list", { name: "Changes" }).waitFor(WAIT);

  await open(page, "Assets/UI/Display.ttf");
  await page.getByText("outlines changed", { exact: false }).waitFor(WAIT);
  await shot(page, "font");

  await open(page, "Assets/Art/Hull.mat");
  await page.getByText("+_NORMALMAP").waitFor(WAIT);
  await page.getByRole("application", { name: "3D preview of Assets/Art/Hull.mat" }).getByText("On disk").waitFor(WAIT);
  await page.waitForTimeout(1500);
  await shot(page, "material");

  await open(page, "Assets/Audio/Horn.wav");
  const au = page.getByRole("application", { name: "Sound preview of Assets/Audio/Horn.wav" });
  await au.getByText("On disk · 2.85 s").waitFor(WAIT);
  await au.getByRole("button", { name: "Play" }).click();
  await page.waitForTimeout(600);
  await au.getByRole("radio", { name: "A", exact: true }).click();
  await page.waitForTimeout(400);
  await shot(page, "audio");
  await au.getByRole("button", { name: "Pause" }).click();

  await open(page, "Assets/Data/BoatStats.csv");
  await page.getByText("rows matched by id", { exact: false }).waitFor(WAIT);
  await shot(page, "table");

  await open(page, "Docs/GDD.pdf");
  await page.getByText("pages matched by their text").waitFor(WAIT);
  await page.waitForTimeout(1500);
  await shot(page, "pdf");

  await open(page, "Assets/UI/HUD.png.meta");
  await page.getByText("Sprite sheet · 3 slices").waitFor(WAIT);
  await page.waitForTimeout(800);
  await shot(page, "sprites");

  await open(page, "Assets/Anims/Bob.anim");
  await page.getByText("Clip · 2.00 s · loop").waitFor(WAIT);
  await shot(page, "curves");

  await open(page, "Assets/Scenes/Harbour.unity");
  const sc = page.getByRole("application", { name: "3D preview of Assets/Scenes/Harbour.unity" });
  await sc.getByText("On disk").waitFor(WAIT);
  await sc.getByRole("radio", { name: "Changes" }).click();
  await sc.getByRole("radio", { name: "Merged" }).click();
  await page.waitForTimeout(800);
  await shot(page, "scene");

  if (media) {
    await open(page, "Assets/Video/Tutorial.webm");
    const vid = page.getByRole("application", { name: "Video preview of Assets/Video/Tutorial.webm" });
    await vid.getByText("Find where frames differ").waitFor(WAIT);
    await vid.getByRole("button", { name: "Play" }).click();
    await page.waitForTimeout(1200);
    await vid.getByRole("button", { name: "Pause" }).click();
    await vid.getByRole("button", { name: "Find where frames differ" }).click();
    await vid.getByText(/frames differ in|every sampled frame matches/).waitFor({ timeout: 90_000 });
    await shot(page, "video");
  }
  if (existsSync(MP4)) {
    await page.getByRole("list", { name: "Untracked" }).getByRole("button", { name: "Assets/Video/Intro.mp4", exact: true }).click();
    const mp4 = page.getByRole("application", { name: "Video preview of Assets/Video/Intro.mp4" });
    await mp4.getByText(/fps/).waitFor(WAIT);
    await mp4.getByLabel("Position").fill("500");
    await page.waitForTimeout(800);
    await shot(page, "video-mp4");
  }
});
