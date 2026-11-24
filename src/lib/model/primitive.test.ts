import { type Mesh, MeshStandardMaterial } from "three";
import { describe, expect, it } from "vitest";
import { material } from "$lib/test/material";
import { primitiveModel } from "./primitive";

describe("primitiveModel", () => {
  it("dresses the shape in the material and notes an approximated shader", () => {
    const view = material({}, []);
    view.material.approximated = true;
    view.material.shader = "BoatAttack/Hull";
    const m = primitiveModel("cube", view, (p) => p);
    const mesh = m.root.children[0] as Mesh;
    expect(mesh.name).toBe("cube");
    expect(mesh.material).toBeInstanceOf(MeshStandardMaterial);
    expect(m.approximated).toEqual(["BoatAttack/Hull"]);
    expect(m.missing).toEqual([]);
  });
});
