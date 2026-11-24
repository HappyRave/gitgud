import type { Mesh } from "three";
import { describe, expect, it } from "vitest";
import { FBXLoader } from "./FBXLoader.js";

/** One triangle used by a model, plus an unused geometry and an empty normals layer: both legal FBX. */
const fbx = `; FBX 7.3.0 project file
FBXHeaderExtension:  {
	FBXHeaderVersion: 1003
	FBXVersion: 7300
}
Objects:  {
	Geometry: 100, "Geometry::Tri", "Mesh" {
		Vertices: *9 {
			a: 0,0,0,1,0,0,0,1,0
		}
		PolygonVertexIndex: *3 {
			a: 0,1,-3
		}
		LayerElementNormal: 0 {
			Version: 101
			Name: ""
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
		}
	}
	Geometry: 200, "Geometry::Unused", "Mesh" {
		Vertices: *9 {
			a: 0,0,0,1,0,0,0,1,0
		}
		PolygonVertexIndex: *3 {
			a: 0,1,-3
		}
	}
	Model: 300, "Model::Tri", "Mesh" {
		Version: 232
	}
}
Connections:  {
	C: "OO",300,0
	C: "OO",100,300
}
`;

describe("vendored FBXLoader", () => {
  it("reads files with an empty normals layer and a geometry no model uses", () => {
    const bytes = new TextEncoder().encode(fbx);
    const root = new FBXLoader().parse(bytes.buffer, "");
    const meshes: Mesh[] = [];
    root.traverse((o) => {
      if ((o as Mesh).isMesh) {
        meshes.push(o as Mesh);
      }
    });
    expect(meshes.map((m) => m.name)).toEqual(["Tri"]);
    expect(meshes[0].geometry.getAttribute("position").count).toBe(3);
  });
});
