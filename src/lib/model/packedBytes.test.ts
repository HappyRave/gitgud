import { describe, expect, it } from "vitest";
import type { PackedModel } from "./packFbx";
import { readPacked, writePacked } from "./packedBytes";

const model = (): PackedModel => ({
  scale: 0.01,
  clips: [{ name: "Take 001", duration: 1, tracks: [], uuid: "u", blendMode: 2500 }],
  meshes: [
    {
      name: "Hull_Mesh",
      originalName: "Hull",
      transformData: { rotationPivot: [1, 2, 3] },
      attributes: [
        { name: "position", array: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1.5, 0]), itemSize: 3, normalized: false },
        { name: "color", array: new Uint8Array([255, 0, 7]), itemSize: 3, normalized: true },
      ],
      index: new Uint16Array([0, 1, 2]),
      groups: [{ start: 0, count: 3, materialIndex: 0 }],
      materials: [{ name: "M_Hull", color: 0x336699 }],
    },
    {
      name: "Mast",
      originalName: null,
      transformData: null,
      attributes: [{ name: "position", array: new Float32Array([5, 6, 7]), itemSize: 3, normalized: false }],
      index: null,
      groups: [],
      materials: [],
    },
  ],
});

describe("writePacked / readPacked", () => {
  it("round-trips a packed model through one buffer, arrays as views of it", () => {
    const buf = writePacked(model());
    const back = readPacked(buf);
    expect(back).toEqual(model());
    expect(back.meshes[0].attributes[0].array).toBeInstanceOf(Float32Array);
    expect(back.meshes[0].index).toBeInstanceOf(Uint16Array);
    expect(back.meshes[0].attributes[0].array.buffer).toBe(buf);
    expect(back.meshes[0].attributes[0].array.byteOffset % 8).toBe(0);
  });

  it("refuses bytes it didn't write", () => {
    expect(() => readPacked(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9]).buffer)).toThrow("not a packed model");
  });
});
