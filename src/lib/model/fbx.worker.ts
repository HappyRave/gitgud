import { FBXLoader } from "./vendor/FBXLoader.js";
import { packFbx } from "./packFbx";

/** Parses FBX files off the main thread for prefabs and scenes: geometry and clips only. */
self.onmessage = (e: MessageEvent<{ id: number; bytes: ArrayBuffer }>) => {
  const { id, bytes } = e.data;
  try {
    const loader = new FBXLoader() as FBXLoader & { geometryOnly: boolean };
    loader.geometryOnly = true;
    const group = loader.parse(bytes, "");
    const units = (group.userData.unitScaleFactor as number | undefined) ?? 1;
    const { packed, transfer } = packFbx(group, units / 100, group.animations);
    self.postMessage({ id, packed }, { transfer });
  } catch (err) {
    self.postMessage({ id, error: err instanceof Error ? err.message : String(err) });
  }
};
