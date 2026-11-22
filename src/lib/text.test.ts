import { describe, expect, it } from "vitest";
import { middleTruncate } from "./text";

describe("middleTruncate", () => {
  it("leaves short text alone", () => {
    expect(middleTruncate("main", 10)).toBe("main");
    expect(middleTruncate("0123456789", 10)).toBe("0123456789");
  });

  it("keeps the start and more of the end", () => {
    const name = "feature/ocean-shader-refactor-for-urp-17-with-foam-falloff-v2-final";
    const short = middleTruncate(name, 40);
    expect(short).toHaveLength(40);
    expect(short).toBe("feature/ocean-sh…h-foam-falloff-v2-final");
  });
});
