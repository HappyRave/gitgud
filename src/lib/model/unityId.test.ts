import { describe, expect, it } from "vitest";
import { unityFileId, xxh64 } from "./unityId";

describe("unityFileId", () => {
  it("matches xxHash64 reference values", () => {
    expect(xxh64("")).toBe(0xef46db3751d8e999n);
    expect(xxh64("abc")).toBe(0x44bc2cf5ad770999n);
    expect(xxh64("Nobody inspects the spammish repetition")).toBe(0xfbcea83c8a378bf1n);
  });

  it("gives the fileIDs a real project's prefabs use for FBX meshes", () => {
    expect(unityFileId("Mesh", "MagicWand_ToolGun")).toBe("8742364090748965452");
    expect(unityFileId("Mesh", "MagicWand_ToolGun_trigger")).toBe("-8767939539822045562");
  });
});
