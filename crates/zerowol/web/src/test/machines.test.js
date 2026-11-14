import { describe, it, expect, vi, afterEach } from "vitest";
import { reconcile, refresh, state } from "../lib/machines.js";

afterEach(() => vi.unstubAllGlobals());
const mk = (id, online, uptime) => ({ id, name: id, status: { online, uptime } });

describe("reconcile", () => {
  it("wake done when online", () => {
    const inf = new Map([["a", { kind: "wake", start: 0 }]]);
    expect(reconcile(inf, [mk("a", true)], 1).failed).toEqual([]);
    expect(inf.size).toBe(0);
  });
  it("poweroff done when offline", () => {
    const inf = new Map([["a", { kind: "poweroff", start: 0 }]]);
    reconcile(inf, [mk("a", false)], 1);
    expect(inf.size).toBe(0);
  });
  it("reboot needs off then on", () => {
    const inf = new Map([["a", { kind: "reboot", start: 0, up: 100 }]]);
    reconcile(inf, [mk("a", true, 100)], 1);
    expect(inf.size).toBe(1);
    reconcile(inf, [mk("a", false)], 2);
    expect(inf.get("a").off).toBe(true);
    reconcile(inf, [mk("a", true, 5)], 3);
    expect(inf.size).toBe(0);
  });
  it("reboot detects uptime reset", () => {
    const inf = new Map([["a", { kind: "reboot", start: 0, up: 100 }]]);
    reconcile(inf, [mk("a", true, 3)], 1);
    expect(inf.size).toBe(0);
  });
  it("expires after LIMIT", () => {
    const now = 1e6;
    const inf = new Map([["a", { kind: "poweroff", start: now - 90001 }]]);
    expect(reconcile(inf, [mk("a", true, 9)], now).failed).toEqual([{ id: "a", kind: "poweroff" }]);
    expect(inf.size).toBe(0);
  });
  it("drops deleted machines", () => {
    const inf = new Map([["a", { kind: "wake", start: 0 }]]);
    reconcile(inf, [], 1);
    expect(inf.size).toBe(0);
  });
});

describe("refresh", () => {
  it("sets offline on network error", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("x"); }));
    await refresh();
    expect(state.offline).toBe(true);
    vi.stubGlobal("fetch", vi.fn(async () => ({ status: 200, ok: true, json: async () => [] })));
    await refresh();
    expect(state.offline).toBe(false);
    expect(state.loaded).toBe(true);
  });
});
