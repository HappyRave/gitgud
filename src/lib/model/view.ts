import { MathUtils, type Sphere, Vector3 } from "three";

/** The stage camera's vertical field of view, in degrees. */
export const FOV = 32;

/** Where the camera looks from when it frames everything: front right, a little above. */
const VIEW_DIR = new Vector3(0.62, 0.38, 0.69).normalize();

/** How far from a sphere of `radius` the camera sees all of it in a view `aspect` wide (width / height). */
export function fitDistance(radius: number, aspect: number): number {
  const half = MathUtils.degToRad(FOV / 2);
  const horizontal = Math.atan(Math.tan(half) * aspect);
  return radius / Math.sin(Math.min(half, horizontal));
}

/** The camera position that frames `sphere` from the default direction in a view `aspect` wide. */
export function framingCamera(sphere: Sphere, aspect = 1): Vector3 {
  return sphere.center.clone().addScaledVector(VIEW_DIR, fitDistance(sphere.radius, aspect) * 1.15);
}
