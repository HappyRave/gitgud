import { describe, expect, it } from "vitest";
import { fileUrl, formatKey, metaTarget, viewerFor, viewModes } from "./route";

describe("viewer routing", () => {
  it("picks a viewer by extension, and a .meta opens its asset's viewer", () => {
    expect(viewerFor("Art/Boat.FBX")).toBe("model");
    expect(viewerFor("Prefabs/Skiff.prefab")).toBe("model");
    expect(viewerFor("Meshes/Hull.asset")).toBe("asset");
    expect(viewerFor("Art/Boat.fbx.meta")).toBe("model");
    expect(viewerFor("UI/icon.png.meta")).toBe("atlas");
    expect(viewerFor("UI/icon.png")).toBeNull();
    expect(viewerFor("UI/HUD.spriteatlas")).toBe("table");
    expect(viewerFor("Audio/horn.wav")).toBe("audio");
    expect(viewerFor("Docs/GDD.pdf")).toBe("pdf");
    expect(viewerFor("UI/Display.woff2")).toBe("font");
    expect(viewerFor("README.md")).toBeNull();
    expect(metaTarget("a/b.wav.meta")).toBe("a/b.wav");
    expect(metaTarget("a/b.wav")).toBeNull();
  });

  it("offers Visual | Structure | Source for Unity text files and Visual only for binary ones", () => {
    expect(viewModes("Skiff.prefab")).toEqual(["visual", "structure", "source"]);
    expect(viewModes("Hull.asset")).toEqual(["visual", "structure", "source"]);
    expect(viewModes("Boat.fbx")).toEqual(["visual"]);
    expect(viewModes("Boat.gltf")).toEqual(["visual", "source"]);
    expect(viewModes("Boat.fbx.meta")).toEqual(["visual", "source"]);
    expect(viewModes("Gun.controller")).toEqual(["structure", "source"]);
    expect(viewModes("HUD.png.meta")).toEqual(["visual", "source"]);
    expect(viewModes("Horn.wav")).toEqual(["visual"]);
    expect(viewModes("stats.csv")).toEqual(["visual", "source"]);
    expect(viewModes("notes.txt")).toEqual(["source"]);
  });

  it("remembers views per format, a .meta apart from its asset", () => {
    expect(formatKey("A/Skiff.Prefab")).toBe("prefab");
    expect(formatKey("A/Boat.fbx.meta")).toBe("fbx.meta");
  });

  it("serves any file version on the protocol", () => {
    expect(fileUrl(2, "Audio/horn 1.wav", { at: "commit", commit: "c1" }, 3, true)).toBe(
      "http://gitgud.localhost/file?repo=2&path=Audio%2Fhorn+1.wav&at=commit&commit=c1&v=3",
    );
  });
});
