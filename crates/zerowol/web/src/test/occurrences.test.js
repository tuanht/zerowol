import { describe, it, expect } from "vitest";
import { outcomeText } from "../lib/occurrences.js";

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
