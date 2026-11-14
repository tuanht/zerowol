import { describe, it, expect } from "vitest";
import { modernZone, searchZones, zoneEntries, zoneNow } from "../lib/zones.js";

const entries = zoneEntries();
const ids = (q) => searchZones(entries, q).map((e) => e.id);

describe("zoneEntries", () => {
  it("describes each zone", () => {
    const hcm = entries.find((e) => e.id === "Asia/Ho_Chi_Minh");
    expect(hcm).toMatchObject({ city: "Ho Chi Minh", region: "Asia", offset: "GMT+7" });
    expect(entries.find((e) => e.id === "America/Port_of_Spain").city).toBe("Port of Spain");
    expect(entries.find((e) => e.id === "UTC")).toMatchObject({ region: "Other", offset: "GMT" });
  });
  it("shows current IANA names, once each", () => {
    expect(entries.some((e) => e.id === "Asia/Saigon")).toBe(false);
    expect(new Set(entries.map((e) => e.id)).size).toBe(entries.length);
    expect(modernZone("Asia/Calcutta")).toBe("Asia/Kolkata");
  });
  it("adds a stored zone the browser lacks", () => {
    expect(zoneEntries("Mars/Olympus_Mons").some((e) => e.id === "Mars/Olympus_Mons")).toBe(true);
    expect(zoneEntries("Asia/Saigon").filter((e) => e.city === "Ho Chi Minh")).toHaveLength(1);
  });
  it("formats offsets and times", () => {
    expect(zoneNow("Asia/Kolkata").offset).toBe("GMT+5:30");
    expect(zoneNow("UTC", new Date(Date.UTC(2026, 0, 1, 9, 5))).time).toBe("09:05");
  });
});

describe("searchZones", () => {
  it("returns everything for an empty query", () => {
    expect(searchZones(entries, "  ")).toBe(entries);
  });
  it("puts a city prefix first, ignoring spaces and case", () => {
    expect(ids("ho chi")[0]).toBe("Asia/Ho_Chi_Minh");
    expect(ids("HoChi")[0]).toBe("Asia/Ho_Chi_Minh");
    expect(ids("new y")[0]).toBe("America/New_York");
    expect(ids("berlin")[0]).toBe("Europe/Berlin");
  });
  it("matches generic names, regions and ids", () => {
    expect(ids("pacific")).toContain("America/Los_Angeles");
    expect(ids("asia/ho")).toContain("Asia/Ho_Chi_Minh");
  });
  it("ranks city prefix over word prefix over substring", () => {
    expect(ids("york")).toContain("America/New_York"); // word in the city
    // "Pacific Time" (word prefix of the generic name) before the Pacific/ region (substring of the id)
    const r = ids("pacific");
    expect(r.indexOf("America/Los_Angeles")).toBeLessThan(r.indexOf("Pacific/Auckland"));
    expect(r.indexOf("Pacific/Auckland")).toBeGreaterThan(0);
  });
  it("matches offsets", () => {
    for (const q of ["+7", "gmt+7", "UTC+7", "utc +7"]) expect(ids(q)).toContain("Asia/Bangkok");
    expect(ids("+5:30")).toContain("Asia/Kolkata");
    expect(ids("+7")).not.toContain("Asia/Kolkata");
    expect(ids("-5")).toContain("America/Bogota"); // no DST
  });
  it("finds nothing for gibberish", () => {
    expect(ids("qxzzv")).toEqual([]);
  });
});
