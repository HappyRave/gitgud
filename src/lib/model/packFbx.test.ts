import {
  AnimationClip,
  BufferAttribute,
  BufferGeometry,
  Group,
  Mesh,
  MeshPhongMaterial,
  type MeshStandardMaterial,
  VectorKeyframeTrack,
} from "three";
import { describe, expect, it } from "vitest";
import { readGeometry } from "./readGeometry";
import { packFbx, unpackFbx } from "./packFbx";

const tri = (name: string) => {
  const g = new BufferGeometry();
  g.setAttribute("position", new BufferAttribute(new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]), 3));
  g.setAttribute("skinIndex", new BufferAttribute(new Uint16Array(12), 4));
  g.setIndex([0, 1, 2]);
  g.addGroup(0, 3, 1);
  return Object.assign(new Mesh(g), { name });
};

describe("packFbx", () => {
  it("keeps meshes in order with names, pivots, groups, scale and clips, and drops what prefabs don't use", () => {
    const root = new Group();
    const a = tri("Hull_Mesh");
    a.userData.originalName = "Hull";
    a.userData.transformData = { rotationPivot: [1, 2, 3] };
    const holder = new Group();
    holder.add(tri("Hull_Mesh"));
    root.add(a, holder);
    const clip = new AnimationClip("Take 001", 1, [new VectorKeyframeTrack("Hull_Mesh.position", [0, 1], [0, 0, 0, 1, 0, 0])]);

    const { packed, transfer } = packFbx(root, 0.01, [clip]);
    expect(transfer.length).toBe(4);
    const model = unpackFbx(structuredClone(packed), 2);

    const file = model.root.children[0];
    expect(file.scale.x).toBeCloseTo(0.02);
    const meshes = file.children as Mesh[];
    expect(meshes.map((m) => [m.name, m.userData.originalName ?? null])).toEqual([
      ["Hull_Mesh", "Hull"],
      ["Hull_Mesh", null],
    ]);
    expect(meshes[0].userData.transformData).toEqual({ rotationPivot: [1, 2, 3] });
    expect(meshes[0].geometry.getAttribute("position").count).toBe(3);
    expect(meshes[0].geometry.getAttribute("skinIndex")).toBeUndefined();
    expect(meshes[0].geometry.index?.count).toBe(3);
    expect(meshes[0].geometry.groups).toEqual([{ start: 0, count: 3, materialIndex: 1 }]);
    expect(model.clips[0].name).toBe("Take 001");
    expect(model.clips[0].tracks[0].name).toBe("Hull_Mesh.position");
  });

  it("keeps each mesh's material names and colours, so import remaps still find them", () => {
    const root = new Group();
    const one = tri("Ground");
    one.material = new MeshPhongMaterial({ name: "M_Ground", color: 0x336699 });
    const two = tri("Wall");
    two.material = [new MeshPhongMaterial({ name: "M_Ground" }), new MeshPhongMaterial({ name: "M_Rock" })];
    root.add(one, two);

    const meshes = unpackFbx(structuredClone(packFbx(root, 1, []).packed), 1).root.children[0].children as Mesh[];

    const first = meshes[0].material as MeshStandardMaterial;
    expect(first.name).toBe("M_Ground");
    expect(first.color.getHex()).toBe(0x336699);
    const names = (meshes[1].material as MeshStandardMaterial[]).map((m) => m.name);
    expect(names).toEqual(["M_Ground", "M_Rock"]);
  });

  it("parses an FBX in place when there are no workers", async () => {
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
    const { packed } = await readGeometry(new TextEncoder().encode(fbx).buffer, "fbx");
    expect(packed.meshes.map((m) => m.name)).toEqual(["Tri"]);
    expect(packed.scale).toBeCloseTo(0.01);
  });

  it("reads the GLB the backend makes of a Unity mesh", async () => {
    const positions = Buffer.from(new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]).buffer).toString("base64");
    const gltf = JSON.stringify({
      asset: { version: "2.0" },
      scene: 0,
      scenes: [{ nodes: [0] }],
      nodes: [{ name: "Wall", mesh: 0 }],
      meshes: [{ name: "Wall", primitives: [{ attributes: { POSITION: 0 } }] }],
      buffers: [{ byteLength: 36, uri: `data:application/octet-stream;base64,${positions}` }],
      bufferViews: [{ buffer: 0, byteLength: 36 }],
      accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] }],
    });
    const { packed, transfer } = await readGeometry(await new Response(gltf).arrayBuffer(), "glb");
    expect(packed.scale).toBe(1);
    expect(packed.meshes).toHaveLength(1);
    expect(packed.meshes[0].attributes.map((a) => a.name)).toEqual(["position"]);
    expect(transfer.length).toBeGreaterThan(0);
  });
});
