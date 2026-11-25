import { Group, InstancedMesh, type Material, type Mesh, type Object3D } from "three";

/** What `instance` did, so it can be undone when the shading or the models change. */
export interface Instanced {
  /** The instanced meshes, to add to the scene beside the model. */
  group: Group;
  /** How many meshes they stand for. */
  meshes: number;
  undo: () => void;
}

function shownInScene(o: Object3D): boolean {
  for (let p: Object3D | null = o; p; p = p.parent) {
    if (!p.visible) {
      return false;
    }
  }
  return true;
}

/**
 * Draws the meshes under `root` that share geometry and material(s) as one
 * InstancedMesh per kind: a scene's thousands of meshes are mostly copies, and each
 * would be its own draw call. Meshes `eligible` turns down, hidden ones and lone
 * ones stay as they are. The originals are hidden, not removed: comparing, picking
 * and framing still use them.
 */
export function instance(root: Object3D, eligible: (mesh: Mesh) => boolean): Instanced {
  root.updateMatrixWorld(true);
  const kinds = new Map<string, Mesh[]>();
  root.traverse((o) => {
    const mesh = o as Mesh;
    if (!mesh.isMesh || (mesh as { isSkinnedMesh?: boolean }).isSkinnedMesh || (mesh as { isInstancedMesh?: boolean }).isInstancedMesh) {
      return;
    }
    if (!eligible(mesh) || !shownInScene(mesh)) {
      return;
    }
    const mats = [mesh.material].flat() as Material[];
    const key = `${mesh.geometry.uuid}|${mats.map((m) => m.uuid).join("+")}`;
    kinds.set(key, [...(kinds.get(key) ?? []), mesh]);
  });

  const group = new Group();
  group.name = "(instanced)";
  const hidden: Mesh[] = [];
  for (const list of kinds.values()) {
    if (list.length < 2) {
      continue;
    }
    const first = list[0];
    const batch = new InstancedMesh(first.geometry, first.material, list.length);
    list.forEach((m, i) => {
      batch.setMatrixAt(i, m.matrixWorld);
      m.visible = false;
      hidden.push(m);
    });
    batch.instanceMatrix.needsUpdate = true;
    batch.computeBoundingSphere();
    batch.castShadow = true;
    batch.raycast = () => {};
    group.add(batch);
  }
  return {
    group,
    meshes: hidden.length,
    undo: () => {
      for (const m of hidden) {
        m.visible = true;
      }
      for (const batch of group.children as InstancedMesh[]) {
        batch.dispose();
      }
      group.removeFromParent();
    },
  };
}
