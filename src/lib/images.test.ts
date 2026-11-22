import { describe, expect, it } from "vitest";
import { imageUrl, isImage } from "./images";

describe("isImage", () => {
  it("knows the formats the webview shows", () => {
    expect(isImage("Assets/T.PNG")).toBe(true);
    expect(isImage("a.jpeg")).toBe(true);
    expect(isImage("a.psd")).toBe(true);
    expect(isImage("a.tga")).toBe(true);
    expect(isImage("png")).toBe(false);
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
