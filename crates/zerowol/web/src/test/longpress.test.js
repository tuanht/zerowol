import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { dayTime, longPress, slotTime } from "../lib/longpress.js";
import { clockIn } from "../lib/schedules.js";

const at = (date, hm) => {
  const [h, m] = hm.split(":");
  return { date, min: +h * 60 + +m };
};
const now = at("2026-11-14", "16:42");

describe("slotTime (week)", () => {
  it("past days and past hours today are over", () => {
    expect(slotTime("2026-11-13", 20, now)).toBe(null);
    expect(slotTime("2026-11-14", 15, now)).toBe(null);
  });
  it("later hours and later days get HH:00", () => {
    expect(slotTime("2026-11-14", 17, now)).toBe("17:00");
    expect(slotTime("2026-11-15", 3, now)).toBe("03:00");
    expect(slotTime("2026-12-01", 0, now)).toBe("00:00");
  });
  it("the current hour gets the next quarter hour", () => {
    expect(slotTime("2026-11-14", 16, now)).toBe("16:45");
    expect(slotTime("2026-11-14", 16, at("2026-11-14", "16:00"))).toBe("16:15");
    expect(slotTime("2026-11-14", 16, at("2026-11-14", "16:14"))).toBe("16:15");
  });
  it("rolls to the next hour once no quarter is left", () => {
    expect(slotTime("2026-11-14", 16, at("2026-11-14", "16:45"))).toBe("17:00");
    expect(slotTime("2026-11-14", 16, at("2026-11-14", "16:59"))).toBe("17:00");
  });
  it("nothing past 23:45 today", () => {
    expect(slotTime("2026-11-14", 23, at("2026-11-14", "23:44"))).toBe("23:45");
    expect(slotTime("2026-11-14", 23, at("2026-11-14", "23:45"))).toBe(null);
  });
});

describe("dayTime (month)", () => {
  it("past days are over, later days get 08:00", () => {
    expect(dayTime("2026-11-13", now)).toBe(null);
    expect(dayTime("2026-11-15", now)).toBe("08:00");
  });
  it("today: 08:00 until it has passed, then the next quarter hour", () => {
    expect(dayTime("2026-11-14", at("2026-11-14", "07:59"))).toBe("08:00");
    expect(dayTime("2026-11-14", at("2026-11-14", "08:00"))).toBe("08:15");
    expect(dayTime("2026-11-14", now)).toBe("16:45");
  });
  it("today past 23:45: nothing", () => {
    expect(dayTime("2026-11-14", at("2026-11-14", "23:40"))).toBe("23:45");
    expect(dayTime("2026-11-14", at("2026-11-14", "23:46"))).toBe(null);
  });
});

describe("clockIn", () => {
  it("is the wall clock in the zone", () => {
    const t = new Date("2026-11-14T23:30:00Z");
    expect(clockIn("Asia/Ho_Chi_Minh", t)).toEqual({ date: "2026-11-15", min: 6 * 60 + 30 });
    expect(clockIn("UTC", t)).toEqual({ date: "2026-11-14", min: 23 * 60 + 30 });
    expect(clockIn("UTC", new Date("2026-11-14T00:05:00Z"))).toEqual({ date: "2026-11-14", min: 5 });
  });
});

describe("longPress", () => {
  beforeEach(() => vi.stubGlobal("document", new EventTarget()));
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });
  const el = () => ({ classList: { add: vi.fn(), remove: vi.fn() } });
  const ev = (x = 0, y = 0) => ({ button: 0, clientX: x, clientY: y, target: {}, stopPropagation: vi.fn(), preventDefault: vi.fn() });
  it("fires after the delay and swallows the following click only", () => {
    vi.useFakeTimers();
    const run = vi.fn();
    const lp = longPress({ find: () => el(), resolve: () => run });
    lp.pointerdown(ev());
    vi.advanceTimersByTime(499);
    expect(run).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(run).toHaveBeenCalledOnce();
    // The next click is cancelled, the one after it isn't (a stand-in document: no DOM in these tests).
    const click = () => {
      const e = new Event("click", { cancelable: true });
      document.dispatchEvent(e);
      return e.defaultPrevented;
    };
    expect(click()).toBe(true);
    expect(click()).toBe(false);
    // touch: no click came, the next press drops the swallower
    lp.pointerdown(ev());
    vi.advanceTimersByTime(500);
    document.dispatchEvent(new Event("pointerdown"));
    expect(click()).toBe(false);
  });
  it("a move past 10 px or an early release cancels it", () => {
    vi.useFakeTimers();
    const run = vi.fn();
    const lp = longPress({ find: () => el(), resolve: () => run });
    lp.pointerdown(ev(0, 0));
    lp.pointermove(ev(0, 8));
    lp.pointermove(ev(0, 12));
    vi.advanceTimersByTime(600);
    lp.pointerdown(ev());
    lp.end();
    vi.advanceTimersByTime(600);
    expect(run).not.toHaveBeenCalled();
  });
  it("an over slot gives no feedback, a non-slot doesn't start", () => {
    vi.useFakeTimers();
    const e = el();
    const lp = longPress({ find: (t) => (t.slot ? e : null), resolve: () => null });
    lp.pointerdown(ev());
    lp.pointerdown({ ...ev(), target: { slot: true } });
    vi.advanceTimersByTime(600);
    expect(e.classList.add).not.toHaveBeenCalled();
  });
});
