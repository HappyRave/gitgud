import { describe, expect, it } from "vitest";
import type { ModelMaterial } from "$lib/api";
import { Approximations, mergeBases } from "./approximations";

const slot = (property: string) => ({ property, path: `${property}.png`, key: null, tiling: [1, 1] as [number, number], offset: [0, 0] as [number, number] });

const mat = (shader: string, approximated: boolean, props: string[], base: string | null): ModelMaterial => ({
  name: shader,
  path: null,
  shader,
  approximated,
  unlit: false,
  color: [1, 1, 1, 1],
  baseMap: null,
  normalMap: null,
  baseMapKey: null,
  normalMapKey: null,
  tiling: [1, 1],
  offset: [0, 0],
  metallic: 0,
  smoothness: 0.5,
  missing: false,
  textures: props.map(slot),
  baseProperty: base,
  baseCandidates: props,
});

describe("Approximations", () => {
  it("collects each custom shader's texture properties and its automatic pick, ignoring known shaders", () => {
    const a = new Approximations();
    a.note(mat("Custom/Ground", true, ["_Dirt", "_Grass"], "_Dirt"));
    a.note(mat("Custom/Ground", true, ["_Grass", "_Path"], "_Grass"));
    a.note(mat("Universal Render Pipeline/Lit", false, ["_BaseMap"], "_BaseMap"));

    expect(a.shaders).toEqual(["Custom/Ground"]);
    expect(a.bases).toEqual({ "Custom/Ground": { auto: "_Dirt", slots: ["_Dirt", "_Grass", "_Path"] } });
  });

  it("merges both sides of a compare", () => {
    const merged = mergeBases(
      { "Custom/Ground": { auto: null, slots: ["_Dirt"] } },
      undefined,
      { "Custom/Ground": { auto: "_Grass", slots: ["_Grass", "_Dirt"] }, Toon: { auto: "_Ramp", slots: ["_Ramp"] } },
    );
    expect(merged).toEqual({
      "Custom/Ground": { auto: "_Grass", slots: ["_Dirt", "_Grass"] },
      Toon: { auto: "_Ramp", slots: ["_Ramp"] },
    });
  });
});
