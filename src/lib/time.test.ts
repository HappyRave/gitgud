import { describe, expect, it } from "vitest";
import { age, clock, fullDate } from "./time";

const at = (y: number, m: number, d: number, h = 12, min = 0) => new Date(y, m - 1, d, h, min).getTime() / 1000;

describe("age", () => {
  const now = at(2026, 11, 22, 12);

  it("counts minutes and hours for today's work", () => {
    expect(age(now - 20, now)).toBe("now");
    expect(age(now - 5 * 60, now)).toBe("5 min");
    expect(age(now - 3 * 3600 - 59, now)).toBe("3 h");
  });

  it("gives a day and month this year, and the year before that", () => {
    expect(age(at(2026, 10, 23), now)).toBe("23 Oct");
    expect(age(at(2024, 2, 1), now)).toBe("1 Feb 2024");
  });

  it("treats future times (clock skew) as now", () => {
    expect(age(now + 500, now)).toBe("now");
  });
});

describe("clock and fullDate", () => {
  it("pad to two digits", () => {
    expect(clock(at(2026, 1, 2, 3, 4))).toBe("03:04");
    expect(fullDate(at(2026, 1, 2, 3, 4))).toBe("2026-01-02 03:04");
  });
});
