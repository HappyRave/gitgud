import { MathUtils, Sphere, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { FOV, fitDistance, framingCamera } from "./view";

/** Whether a camera at `cam` looking at `s.center` sees all of `s` in a view of `aspect`. */
function fits(cam: Vector3, s: Sphere, aspect: number): boolean {
  const dist = cam.distanceTo(s.center);
  const half = MathUtils.degToRad(FOV / 2);
  const horizontal = Math.atan(Math.tan(half) * aspect);
  return s.radius <= dist * Math.sin(Math.min(half, horizontal)) + 1e-9;
}

describe("framingCamera", () => {
  const sphere = new Sphere(new Vector3(1, 2, 3), 0.2);

  it("fits the sphere in a wide view by its height", () => {
    const cam = framingCamera(sphere, 2);
    expect(fits(cam, sphere, 2)).toBe(true);
    expect(cam.distanceTo(sphere.center)).toBeCloseTo(framingCamera(sphere, 1).distanceTo(sphere.center));
  });

  it("backs off for a narrow view, as one side of side by side is", () => {
    const narrow = framingCamera(sphere, 0.45);
    expect(fits(narrow, sphere, 0.45)).toBe(true);
    expect(narrow.distanceTo(sphere.center)).toBeGreaterThan(framingCamera(sphere, 1).distanceTo(sphere.center) * 1.5);
  });

  it("gives a distance that grows with the radius", () => {
    expect(fitDistance(0.4, 0.5)).toBeCloseTo(fitDistance(0.2, 0.5) * 2);
  });
});
