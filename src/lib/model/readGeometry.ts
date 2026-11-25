import { type PackedModel, packFbx } from "./packFbx";

/** Model files whose geometry a worker reads: FBX, and the GLB the backend makes of Unity meshes. */
export type GeometryKind = "fbx" | "glb";

/** Parses a model file's meshes and clips into buffers a worker can hand over. */
export async function readGeometry(bytes: ArrayBuffer, kind: GeometryKind): Promise<{ packed: PackedModel; transfer: ArrayBuffer[] }> {
  if (kind === "glb") {
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const gltf = await new GLTFLoader().parseAsync(bytes, "");
    return packFbx(gltf.scene, 1, gltf.animations);
  }
  const { FBXLoader } = await import("./vendor/FBXLoader.js");
  const loader = new FBXLoader() as InstanceType<typeof FBXLoader> & { geometryOnly: boolean };
  loader.geometryOnly = true;
  const group = loader.parse(bytes, "");
  const units = (group.userData.unitScaleFactor as number | undefined) ?? 1;
  return packFbx(group, units / 100, group.animations);
}
