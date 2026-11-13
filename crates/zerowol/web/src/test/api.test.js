import { describe, it, expect, vi, afterEach } from "vitest";
import { api, esc, offlineMsg } from "../lib/api.js";

afterEach(() => vi.unstubAllGlobals());
const mockFetch = (status, json) =>
  vi.stubGlobal("fetch", vi.fn(async () => ({ status, ok: status >= 200 && status < 300, json: async () => json })));

describe("api", () => {
  it("esc escapes html", () => {
    const s = esc('<img src=x onerror=1>&"');
    expect(s).not.toContain("<");
    expect(s).toContain("&lt;img");
  });
  it("api throws server error message", async () => {
    mockFetch(400, { error: "invalid mac" });
    await expect(api("POST", "/machines", { a: 1 })).rejects.toMatchObject({ message: "invalid mac", status: 400 });
  });
  it("api returns null on 204", async () => {
    mockFetch(204);
    expect(await api("DELETE", "/machines/a")).toBeNull();
  });
  it("api prefixes /api/v1", async () => {
    mockFetch(200, []);
    await api("GET", "/machines");
    expect(fetch.mock.calls[0][0]).toBe("/api/v1/machines");
  });
  it("offlineMsg maps TypeError", () => {
    expect(offlineMsg(new TypeError("x"))).toBe("Can't reach zerowol");
    expect(offlineMsg(new Error("boom"))).toBe("boom");
  });
});
