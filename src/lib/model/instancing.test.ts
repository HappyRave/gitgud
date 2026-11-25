import { BoxGeometry, Group, type InstancedMesh, Matrix4, Mesh, MeshBasicMaterial, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { instance } from "./instancing";

describe("instance", () => {
  it("draws copies of a mesh as one instanced mesh, leaving lone, hidden and turned-down ones", () => {
    const box = new BoxGeometry();
    const stone = new MeshBasicMaterial();
    const wood = new MeshBasicMaterial();
    const root = new Group();
    const at = (x: number, mat = stone) => {
      const m = new Mesh(box, mat);
      m.position.x = x;
      root.add(m);
      return m;
    };
    const copies = [at(1), at(2), at(3)];
    const lone = at(4, wood);
    const off = at(5);
    off.visible = false;
    const changed = at(6);
    changed.userData.changed = true;

    const done = instance(root, (m) => !m.userData.changed);
    expect(done.meshes).toBe(3);
    expect(done.group.children).toHaveLength(1);
    const batch = done.group.children[0] as InstancedMesh;
    expect(batch.count).toBe(3);
    const m = new Matrix4();
    batch.getMatrixAt(2, m);
    expect(new Vector3().setFromMatrixPosition(m).x).toBe(3);
    expect(copies.every((c) => !c.visible)).toBe(true);
    expect([lone.visible, changed.visible, off.visible]).toEqual([true, true, false]);

    done.undo();
    expect(copies.every((c) => c.visible)).toBe(true);
    expect(off.visible).toBe(false);
    expect(done.group.parent).toBeNull();
  });
});
