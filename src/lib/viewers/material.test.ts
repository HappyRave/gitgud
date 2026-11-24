import { describe, expect, it } from "vitest";
import { material } from "$lib/test/material";
import { diffMaterial, materialPill } from "./material";

describe("diffMaterial", () => {
  it("lists changed properties, keywords and the shader", () => {
    const b = material({ _BaseColor: "#FFFFFF", _Smoothness: "0.45", _Old: "1" }, ["_EMISSION"]);
    const a = material({ _BaseColor: "#DFE8FF", _Smoothness: "0.45" }, ["_NORMALMAP"], { shader: "Universal Render Pipeline/Simple Lit" });
    const d = diffMaterial(b, a);
    expect(d.rows.filter((r) => r.changed).map((r) => r.key)).toEqual(["shader", "_BaseColor", "_Old"]);
    expect(d.rows.find((r) => r.key === "_Old")?.after).toBeNull();
    expect(d.keywordsAdded).toEqual(["_NORMALMAP"]);
    expect(d.keywordsRemoved).toEqual(["_EMISSION"]);
    expect(d.changed).toBe(3);
  });

  it("changes nothing for a new material and reads the render queue", () => {
    const d = diffMaterial(null, material({ _Cutoff: "0.5" }, [], { renderQueue: 2450 }));
    expect(d.changed).toBe(0);
    expect(d.rows.map((r) => r.after?.value)).toEqual(["Universal Render Pipeline/Lit", "2450", "0.5"]);
    expect(diffMaterial(null, material({}, [], { renderQueue: -1 })).rows[1].after?.value).toBe("from shader");
  });
});

it("shortens URP in the pill", () => {
  expect(materialPill(material({}, []))).toBe("Material · URP/Lit");
});
