import { describe, it, expect } from "vitest";
import { addDays, fmtDay, fmtLong, monthGrid, weekDays, weekStart } from "../lib/dates.js";

describe("dates", () => {
  it("weekStart is monday", () => {
    expect(weekStart("2026-11-15")).toBe("2026-11-09"); // a Sunday
    expect(weekStart("2026-11-09")).toBe("2026-11-09");
  });
  it("addDays crosses month", () => {
    expect(addDays("2026-11-30", 1)).toBe("2026-12-01");
    expect(addDays("2026-01-01", -1)).toBe("2025-12-31");
  });
  it("monthGrid shape", () => {
    const g = monthGrid("2026-11-13");
    expect(g).toHaveLength(42);
    expect(g[0]).toBe("2026-10-26");
    expect(g).toContain("2026-11-30");
  });
  it("weekDays are seven days from monday", () => {
    const w = weekDays("2026-11-13");
    expect(w).toHaveLength(7);
    expect(w[0]).toBe("2026-11-09");
    expect(w[6]).toBe("2026-11-15");
  });
  it("addDays is DST-safe", () => {
    const tz = process.env.TZ;
    process.env.TZ = "Europe/Berlin";
    try {
      expect(addDays("2027-03-27", 1)).toBe("2027-03-28");
      expect(addDays("2027-03-28", 1)).toBe("2027-03-29");
      expect(addDays("2027-10-31", -1)).toBe("2027-10-30");
    } finally {
      if (tz === undefined) delete process.env.TZ;
      else process.env.TZ = tz;
    }
  });
  it("formats days", () => {
    expect(fmtDay("2026-11-13")).toBe("Fri 13"); // 2026-11-15 is a Sunday
    expect(fmtLong("2026-11-12")).toBe("Thursday, 12 November");
  });
});
