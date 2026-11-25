import { describe, expect, it } from "vitest";
import { readMeshBundle, splitBundle } from "./meshBundle";

/** Builds a bundle the way the backend writes one. */
function bundle(entries: [ok: boolean, id: string, payload: Uint8Array][]): ArrayBuffer {
  const parts: number[] = [];
  const put = (bytes: Uint8Array) => {
    const len = new Uint8Array(new Uint32Array([bytes.length]).buffer);
    parts.push(...len, ...bytes);
  };
  for (const [ok, id, payload] of entries) {
    parts.push(ok ? 1 : 0);
    put(new TextEncoder().encode(id));
    put(payload);
  }
  return new Uint8Array(parts).buffer;
}

describe("meshBundle", () => {
  it("splits a bundle into its meshes and reasons", () => {
    const out = splitBundle(bundle([
      [true, "12", new Uint8Array([1, 2, 3])],
      [false, "-4", new TextEncoder().encode("no Mesh &-4")],
    ]));
    expect(out.map((e) => [e.ok, e.id, e.payload.byteLength])).toEqual([
      [true, "12", 3],
      [false, "-4", 11],
    ]);
    expect(splitBundle(new ArrayBuffer(0))).toEqual([]);
  });

  it("reads each GLB and keeps each reason", async () => {
    const positions = Buffer.from(new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]).buffer).toString("base64");
    const gltf = new TextEncoder().encode(
      JSON.stringify({
        asset: { version: "2.0" },
        scene: 0,
        scenes: [{ nodes: [0] }],
        nodes: [{ name: "Wall", mesh: 0 }],
        meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }],
        buffers: [{ byteLength: 36, uri: `data:application/octet-stream;base64,${positions}` }],
        bufferViews: [{ buffer: 0, byteLength: 36 }],
        accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] }],
      }),
    );
    const { meshes, transfer } = await readMeshBundle(
      bundle([
        [true, "1", gltf],
        [false, "2", new TextEncoder().encode("Room.unity has no Mesh &2")],
        [true, "3", new TextEncoder().encode("not a mesh")],
      ]),
    );
    expect(meshes[0]).toMatchObject({ id: "1", packed: { scale: 1 } });
    expect(meshes[1]).toEqual({ id: "2", error: "Room.unity has no Mesh &2" });
    expect(meshes[2]).toMatchObject({ id: "3" });
    expect("error" in meshes[2]).toBe(true);
    expect(transfer.length).toBeGreaterThan(0);
  });
});
