import { describe, it, expect } from "vitest";
import { clashes, closeScheduleForm, formState, openScheduleForm, repeatLabel } from "../lib/schedules.js";

const rule = (id, machine_id, time, repeat, enabled = true) => ({ id, machine_id, action: "wake", time, repeat, enabled });
const daily = { kind: "daily" };

describe("clashes", () => {
  it("clashes detects within two minutes", () => {
    const list = [rule("a", "m1", "02:00", daily)];
    expect(clashes(list, { machine_id: "m1", time: "02:01", repeat: daily })).toHaveLength(1);
    expect(clashes(list, { machine_id: "m1", time: "02:03", repeat: daily })).toHaveLength(0);
    expect(clashes(list, { machine_id: "m2", time: "02:01", repeat: daily })).toHaveLength(0);
  });
  it("clashes wraps midnight", () => {
    const list = [rule("a", "m1", "23:59", daily)];
    expect(clashes(list, { machine_id: "m1", time: "00:00", repeat: daily })).toHaveLength(1);
  });
  it("clashes respects days", () => {
    const list = [rule("a", "m1", "08:00", { kind: "weekdays" })];
    expect(clashes(list, { machine_id: "m1", time: "08:01", repeat: { kind: "weekends" } })).toHaveLength(0);
  });
  it("clashes ignores the rule being edited and disabled rules", () => {
    const list = [rule("a", "m1", "08:00", daily), rule("b", "m1", "08:00", daily, false)];
    expect(clashes(list, { id: "a", machine_id: "m1", time: "08:00", repeat: daily })).toHaveLength(0);
  });
  it("clashes compares one-off dates", () => {
    const list = [rule("a", "m1", "08:00", { kind: "once", date: "2026-11-16" })]; // a Monday
    expect(clashes(list, { machine_id: "m1", time: "08:00", repeat: { kind: "once", date: "2026-11-17" } })).toHaveLength(0);
    expect(clashes(list, { machine_id: "m1", time: "08:01", repeat: { kind: "weekdays" } })).toHaveLength(1);
    expect(clashes(list, { machine_id: "m1", time: "08:01", repeat: { kind: "weekly", days: ["tue"] } })).toHaveLength(0);
  });
});

describe("repeatLabel", () => {
  it("repeatLabel formats", () => {
    expect(repeatLabel({ kind: "weekly", days: ["mon", "fri"] })).toBe("Weekly · Mon, Fri");
    expect(repeatLabel({ kind: "daily" })).toBe("Daily");
    expect(repeatLabel({ kind: "once", date: "2026-11-13" })).toBe("Once · 13 Nov");
  });
});

describe("openScheduleForm", () => {
  it("a stale close does not settle a newer form", async () => {
    const first = openScheduleForm({ machineId: "m1" });
    const oldSeq = formState.seq;
    const second = openScheduleForm({ machineId: "m2" });
    expect(await first).toBe(false); // superseded
    closeScheduleForm(oldSeq, true); // the first popup's late "closed" event
    expect(formState.open).toBe(true);
    let settled = false;
    second.then(() => (settled = true));
    await Promise.resolve();
    expect(settled).toBe(false);
    closeScheduleForm(formState.seq, true);
    expect(await second).toBe(true);
    expect(formState.open).toBe(false);
  });
});
