import { BoxGeometry, type BufferGeometry, Group, Mesh, PlaneGeometry, SphereGeometry } from "three";
import type { MaterialView } from "$lib/api";
import type { LoadedModel } from "./load";
import { materialFrom } from "./materials";

export type Primitive = "sphere" | "cube" | "plane";

function geometry(shape: Primitive): BufferGeometry {
  switch (shape) {
    case "sphere":
      return new SphereGeometry(0.5, 64, 32);
    case "cube":
      return new BoxGeometry(0.8, 0.8, 0.8);
    case "plane":
      return new PlaneGeometry(1.2, 1.2);
  }
}

/** A sphere, cube or plane wearing `view`'s material, as the 3D stage takes a model. */
export function primitiveModel(shape: Primitive, view: MaterialView, imageUrl: (path: string) => string): LoadedModel {
  const mesh = new Mesh(geometry(shape), materialFrom(view.material, imageUrl));
  mesh.name = shape;
  const root = new Group();
  root.add(mesh);
  const m = view.material;
  return {
    root,
    clips: [],
    approximated: m.approximated ? [m.shader] : [],
    missing: m.missing ? [`${m.name}: the material or its shader is missing`] : [],
  };
}
