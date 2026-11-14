import { describe, it, expect } from "vitest";
import { canSkip, dotClass, outcomeText } from "../lib/occurrences.js";

const NOW = 1_800_000_000;
const occ = (at, outcome = null, skipped = false) => ({ at, outcome, skipped });
const out = (result, detail = null) => ({ result, detail, at: NOW });

describe("outcomeText", () => {
  it.each([
    ["skipped (past)", occ(NOW - 60, null, true), "skipped"],
    ["skipped (future)", occ(NOW + 60, null, true), "skipped"],
    ["future", occ(NOW + 60), ""],
    ["future with a stale outcome", occ(NOW + 60, out("ok")), ""],
    ["past, not logged yet", occ(NOW - 60), ""],
    ["ok", occ(NOW - 60, out("ok")), "done"],
    ["already online", occ(NOW - 60, out("ok", "already online")), "done · already online"],
    ["failed with detail", occ(NOW - 60, out("failed", "agent unreachable")), "failed: agent unreachable"],
    ["failed without detail", occ(NOW - 60, out("failed")), "failed: error"],
    ["missed", occ(NOW - 60, out("missed")), "missed"],
    ["firing", occ(NOW - 60, out("firing")), "unknown"],
    ["at == now counts as past", occ(NOW, out("ok")), "done"],
  ])("%s", (_, o, want) => {
    expect(outcomeText(o, NOW)).toBe(want);
  });
});

describe("canSkip", () => {
  it.each([
    ["future", occ(NOW + 60), true],
    ["future, skipped (Don't skip)", occ(NOW + 60, null, true), true],
    ["future with an outcome", occ(NOW + 60, out("ok")), false],
    ["past, not logged", occ(NOW - 60), false],
    ["past, done", occ(NOW - 60, out("ok")), false],
    ["past, skipped", occ(NOW - 60, null, true), false],
    ["at == now", occ(NOW), false],
  ])("%s", (_, o, want) => {
    expect(canSkip(o, NOW)).toBe(want);
  });
});

describe("dotClass", () => {
  const o = (x) => ({ action: "wake", enabled: true, skipped: false, outcome: null, at: NOW + 60, ...x });
  it.each([
    ["enabled", o({}), "wake"],
    ["disabled rule", o({ enabled: false }), "off"],
    ["skipped", o({ skipped: true }), "off"],
    ["once rule that already ran", o({ enabled: false, at: NOW - 60, outcome: out("ok") }), "wake"],
  ])("%s", (_, x, want) => {
    expect(dotClass(x, NOW)).toBe(want);
  });
});
