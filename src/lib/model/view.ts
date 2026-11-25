import { MathUtils, type Sphere, Vector3 } from "three";

/** The stage camera's vertical field of view, in degrees. */
export const FOV = 32;

/** Where the camera looks from when it frames everything: front right, a little above. */
const VIEW_DIR = new Vector3(0.62, 0.38, 0.69).normalize();

/** The camera position that frames `sphere` from the default direction. */
export function framingCamera(sphere: Sphere): Vector3 {
  const dist = (sphere.radius / Math.sin(MathUtils.degToRad(FOV / 2))) * 1.15;
  return sphere.center.clone().addScaledVector(VIEW_DIR, dist);
}
