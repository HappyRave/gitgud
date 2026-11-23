import { describe, expect, it } from "vitest";
import { formatSizeDelta, imageFormatLabel, imageUrl, isImage } from "./images";

describe("isImage", () => {
  it("knows the formats the webview shows", () => {
    expect(isImage("Assets/T.PNG")).toBe(true);
    expect(isImage("a.jpeg")).toBe(true);
    expect(isImage("a.psd")).toBe(true);
    expect(isImage("a.tga")).toBe(true);
    expect(isImage("a.dds")).toBe(true);
    expect(isImage("a.exr")).toBe(true);
    expect(isImage("a.hdr")).toBe(true);
    expect(isImage("a.tif")).toBe(true);
    expect(isImage("a.tiff")).toBe(true);
    expect(isImage("png")).toBe(false);
  });

  it("formats asset labels and byte size deltas", () => {
    expect(imageFormatLabel("Art/sky.exr")).toBe("EXR");
    expect(imageFormatLabel("Art/tex.tif")).toBe("TIFF");
    expect(imageFormatLabel("Art/pic.jpeg")).toBe("JPG");
    expect(imageFormatLabel("Scripts/a.cs")).toBeNull();

    expect(formatSizeDelta(1024, 2048)).toBe("+1.0 KB");
    expect(formatSizeDelta(2048, 1024)).toBe("-1.0 KB");
    expect(formatSizeDelta(512, 512)).toBe("same size");
    expect(formatSizeDelta(null, 512)).toBeNull();
  });
});

describe("imageUrl", () => {
  it("builds the Windows form with an encoded path", () => {
    expect(imageUrl(3, "a b/t&1.png", { at: "index" }, 7, true)).toBe(
      "http://gitgud.localhost/image?repo=3&path=a+b%2Ft%261.png&at=index&v=7",
    );
  });

  it("carries the commit, and uses the scheme form elsewhere", () => {
    expect(imageUrl(1, "t.png", { at: "parent", commit: "abc" }, 0, false)).toBe(
      "gitgud://localhost/image?repo=1&path=t.png&at=parent&commit=abc&v=0",
    );
  });
});
