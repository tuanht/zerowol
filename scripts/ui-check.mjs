// WARNING: this script DELETES ALL MACHINES on the target server (and removes the tokens it creates).
// Point it only at a throwaway instance with TZ unset (for the timezone-step screenshots), e.g.
// env -u TZ ZEROWOL_DATA=$(mktemp -d) ZEROWOL_PORT=18090 ZEROWOL_BIND=127.0.0.1 cargo run -p zerowol
// Usage: NODE_PATH=$(npm root -g) node scripts/ui-check.mjs http://127.0.0.1:18090
// Writes screenshots (ios/md x light/dark x empty, add, pairing, list, edit, confirm, settings,
// form-tz, form-new, form-edit (clash warning), sched-day, sched-week, plus 1280x800 list-wide,
// settings-wide, form-tz-wide, form-new-wide, form-edit-wide, sched-week-wide, sched-month-wide)
// to ./screenshots/. Leaves the schedule timezone set to the browser's zone.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const require = createRequire(import.meta.url);
let pw;
try { pw = require("playwright"); } catch {
  const root = (await import("node:child_process")).execSync("npm root -g").toString().trim();
  pw = createRequire(root + "/")("playwright");
}
const { chromium, devices } = pw;
const base = process.argv[2];
if (!base) {
  console.error("usage: node scripts/ui-check.mjs <base-url of a THROWAWAY zerowol server>");
  process.exit(2);
}
mkdirSync("screenshots", { recursive: true });
const browser = await chromium.launch();
let errors = 0;
const fail = (msg) => { errors++; console.error("FAIL:", msg); };
const CUR = ".view.tab-active .page-current";
const appTheme = (page) => page.evaluate(() => (document.documentElement.classList.contains("zw-ios") ? "ios" : "md"));
const tab = async (page, label) => {
  await page.click(`.toolbar .tab-link:has-text('${label}')`);
  await page.waitForTimeout(300);
};
const back = async (page) => {
  await page.click(`${CUR} .navbar .left .back`);
  await page.waitForTimeout(600);
};
const FORM = ".zw-sched-form.modal-in";
const SCHED = ".view.tab-active .page-current[data-name=schedules]";
const api = (page, method, path, body) =>
  page.evaluate(async ([method, path, body]) => {
    const r = await fetch("/api/v1" + path, { method, headers: { "content-type": "application/json" }, body: body && JSON.stringify(body) });
    if (!r.ok) throw new Error(method + " " + path + ": " + r.status + " " + (await r.text()));
    return r.status === 204 ? null : r.json();
  }, [method, path, body]);
const closeForm = async (page) => {
  // iOS: Cancel; Material phone: the close icon; Material wide (M3 dialog): Cancel at the bottom.
  await page.locator(`${FORM} .navbar .left a, ${FORM} .toolbar-bottom a:has-text('Cancel')`).first().click();
  await page.waitForSelector(FORM, { state: "detached" });
  await page.waitForTimeout(300);
};
// Timezone step and new-schedule form, opened from the (only) card's clock with no rules and no timezone.
async function formFromCard(page, shot, suffix) {
  await api(page, "PATCH", "/settings", { timezone: null });
  await page.click(`${CUR} .card.machine [aria-label=Schedules]`);
  await page.waitForSelector(`${FORM} button:has-text('Use ')`);
  await shot("form-tz" + suffix);
  await page.click(`${FORM} button:has-text('Use ')`);
  await page.waitForSelector(`${FORM} #s-time`);
  await shot("form-new" + suffix);
  await closeForm(page);
}
// Tomorrow in the schedule timezone, and whether it falls in next week (today is a Sunday).
const tomorrow = (page) =>
  page.evaluate(async () => {
    const { timezone } = await (await fetch("/api/v1/settings")).json();
    const t = new Date(new Intl.DateTimeFormat("en-CA", { timeZone: timezone }).format(new Date()) + "T00:00:00Z");
    const sunday = t.getUTCDay() === 0;
    t.setUTCDate(t.getUTCDate() + 1);
    return { date: t.toISOString().slice(0, 10), nextWeek: sunday };
  });
// Schedules tab (wide Week and Month, phone Day and Week) and the edit form with a clash warning, at both sizes.
async function scheduleShots(page, shot, vp) {
  const m = (await api(page, "GET", "/machines"))[0].id;
  const tm = await tomorrow(page);
  const rule = (action, time, repeat) => api(page, "POST", "/schedules", { machine_id: m, action, time, repeat, enabled: true });
  const ids = [];
  ids.push((await rule("wake", "07:30", { kind: "weekdays" })).id);
  const off = await rule("poweroff", "23:00", { kind: "daily" });
  ids.push(off.id);
  ids.push((await rule("reboot", "04:00", { kind: "weekly", days: ["sun", "wed"] })).id);
  ids.push((await rule("reboot", "12:00", { kind: "once", date: tm.date })).id);
  ids.push((await rule("wake", "12:01", { kind: "daily" })).id); // clashes with the once rule
  await api(page, "POST", `/schedules/${off.id}/skip`, { date: tm.date });
  const editOnce = async (sel, name) => {
    await page.click(sel);
    await page.waitForSelector(`${FORM} [role=status]`);
    await shot(name);
    await closeForm(page);
  };
  await tab(page, "Schedules");
  await page.waitForSelector(`${SCHED} .zw-wgrid .zw-chip`);
  if (tm.nextWeek) {
    await page.click(`${SCHED} [aria-label='Next week']`);
    await page.waitForTimeout(400);
  }
  await shot("sched-week-wide");
  await editOnce(`${SCHED} .zw-chip[title^='12:00 ']`, "form-edit-wide");
  await page.click(`${SCHED} .segmented button:has-text('Month')`);
  await page.waitForSelector(`${SCHED} .zw-mgrid`);
  await shot("sched-month-wide");
  await page.click(`${SCHED} .segmented button:has-text('Week')`);
  await page.setViewportSize(vp);
  await page.waitForSelector(`${SCHED} .zw-strip`);
  // The strip shows tomorrow's week (the wide view moved to it): tap tomorrow.
  await page.click(`${SCHED} .zw-day:has(.zw-num:text-is("${+tm.date.slice(8)}"))`);
  await page.waitForSelector(`${SCHED} .list li:has-text('12:00')`);
  await shot("sched-day");
  await editOnce(`${SCHED} .list li:has-text('12:00') a.item-link`, "form-edit");
  await page.click(`${SCHED} .segmented button:has-text('Week')`);
  await page.waitForSelector(`${SCHED} .segmented button[aria-checked=true]:has-text('Week')`);
  await shot("sched-week");
  await page.click(`${SCHED} .segmented button:has-text('Day')`);
  for (const id of ids) await api(page, "DELETE", "/schedules/" + id);
  await page.setViewportSize({ width: 1280, height: 800 });
  await tab(page, "Machines");
}
for (const [theme, device] of [["ios", "iPhone 15"], ["md", "Pixel 7"]]) {
  for (const scheme of ["light", "dark"]) {
    const ctx = await browser.newContext({ ...devices[device], colorScheme: scheme });
    const page = await ctx.newPage();
    page.on("pageerror", (e) => fail("pageerror: " + e.message));
    page.on("console", (m) => { if (m.type() === "error") fail("console error: " + m.text()); });
    await page.goto(base);
    await page.evaluate(async () => {
      for (const m of await (await fetch("/api/v1/machines")).json()) await fetch("/api/v1/machines/" + m.id, { method: "DELETE" });
    });
    await page.reload();
    await page.waitForSelector(`${CUR}[data-name=machines] .empty`);
    const got = await appTheme(page);
    if (got !== theme) fail(`${device}: expected theme ${theme}, got ${got}`);
    const shot = async (name) => {
      await page.waitForTimeout(600);
      const p = `screenshots/${theme}-${scheme}-${name}.png`;
      await page.screenshot({ path: p });
      console.log("wrote", p);
    };
    // Phone layout: tab bar at the bottom (iOS: floating capsule). Add: iOS "+" in the navbar (top right),
    // Material a FAB above the tab bar (bottom right).
    const vp = page.viewportSize();
    const nav = await page.locator(theme === "ios" ? ".zw-nav .toolbar-pane" : ".toolbar.zw-nav").boundingBox();
    const gap = vp.height - (nav.y + nav.height);
    if (!(theme === "ios" ? gap >= 4 && gap <= 40 && nav.x >= 8 : Math.abs(gap) <= 2)) fail("tab bar not at the bottom");
    const ADD = theme === "ios" ? `${CUR} .navbar [aria-label="Add Machine"]` : ".zw-fab a";
    const a = await page.locator(ADD).boundingBox();
    const addOk = theme === "ios" ? a.x > vp.width / 2 && a.y < vp.height / 4 : a.x > vp.width / 2 && a.y > vp.height / 2 && a.y < nav.y;
    if (!addOk) fail("add button position");
    if (theme === "ios" && (await page.locator(".zw-fab").count())) fail("ios shows a FAB");
    await shot("empty");
    await page.click(ADD);
    await page.waitForSelector(`${CUR}[data-name=machine] #f-name`);
    await page.waitForTimeout(400);
    await page.fill(`${CUR} #f-name`, "Desktop");
    await page.fill(`${CUR} #f-mac`, "D8:5E:D3:DB:F7:6B");
    await page.fill(`${CUR} #f-ip`, "127.0.0.1");
    await page.click(`${CUR} .toggle`);
    if (theme === "md" && (await page.locator(".zw-fab:visible").count())) fail("md shows a FAB on the add page");
    await shot("add");
    await page.click(`${CUR} .button-fill`);
    await page.waitForSelector(`${CUR}[data-name=pair] >> text=Waiting for agent`);
    await shot("pairing");
    await back(page);
    await page.waitForSelector(`${CUR} .card.machine .name`);
    await shot("list");
    await page.click(`${CUR} .card.machine .item-link`);
    await page.waitForSelector(`${CUR}[data-name=machine] #f-name`);
    if ((await page.inputValue(`${CUR} #f-name`)) !== "Desktop") fail("edit page prefill");
    if (theme === "md" && (await page.locator(".zw-fab:visible").count())) fail("md shows a FAB on the edit page");
    await shot("edit");
    await back(page);
    // 127.0.0.1 has no agent, so the machine shows offline: report it as online and
    // paired to show the Power off button and its confirm dialog.
    await page.route("**/api/v1/machines", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const res = await route.fetch();
      const list = await res.json();
      for (const m of list) m.status = { online: true, agent: "paired", hostname: "desktop", uptime: 7380 };
      await route.fulfill({ response: res, json: list });
    });
    await page.reload();
    await page.click(`${CUR} .card.machine button:has-text('Power off')`);
    await page.waitForSelector(".dialog.modal-in");
    await shot("confirm");
    await page.click(".dialog.modal-in .dialog-button:has-text('Cancel')");
    await page.waitForTimeout(400);
    await tab(page, "Settings");
    await page.waitForSelector(`${CUR}[data-name=settings]`);
    await page.fill(`${CUR} input[placeholder="Token name"]`, "home-script");
    await page.click(`${CUR} a.link:text-is("Create")`);
    await page.waitForSelector("text=won't be shown again");
    await shot("settings");
    // Schedule form from a card with no timezone saved: the timezone step, then the new-schedule form.
    await tab(page, "Machines");
    await formFromCard(page, shot, "");
    // Wide (tablet/desktop) layout, switched live: iOS capsule at the top, Material rail on the left.
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.waitForTimeout(400);
    const w = await page.locator(".toolbar.zw-nav").boundingBox();
    if (theme === "ios" ? !(w.y < 40) : !(w.x === 0 && w.width <= 80)) fail("wide navigation layout");
    await shot("settings-wide");
    await tab(page, "Machines");
    await page.waitForSelector(`${CUR}[data-name=machines] .card.machine`);
    await shot("list-wide");
    await formFromCard(page, shot, "-wide");
    await scheduleShots(page, shot, vp);
    await page.unroute("**/api/v1/machines");
    await page.evaluate(async () => {
      for (const t of await (await fetch("/api/v1/tokens")).json()) await fetch("/api/v1/tokens/" + t.id, { method: "DELETE" });
    });
    await ctx.close();
  }
}
// Forced theme override: an iPhone with the Android theme picked in Settings.
{
  const ctx = await browser.newContext({ ...devices["iPhone 15"] });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => fail("pageerror: " + e.message));
  await page.goto(base);
  await tab(page, "Settings");
  await page.waitForSelector(`${CUR}[data-name=settings]`);
  await Promise.all([page.waitForEvent("load"), page.click(`${CUR} input[value="md"] + .icon-radio`)]);
  if ((await appTheme(page)) !== "md") fail("forced md theme on iPhone");
  // Light/dark override applies live and survives a reload.
  await tab(page, "Settings");
  await page.waitForSelector(`${CUR}[data-name=settings]`);
  await page.click(`${CUR} input[value="dark"] + .icon-radio`);
  if (!(await page.evaluate(() => document.documentElement.classList.contains("dark")))) fail("forced dark scheme");
  await page.reload();
  if (!(await page.evaluate(() => document.documentElement.classList.contains("dark")))) fail("dark scheme after reload");
  await page.evaluate(() => { localStorage.removeItem("zw-scheme"); localStorage.removeItem("zw-theme"); });
  await page.reload();
  if ((await appTheme(page)) !== "ios") fail("auto theme restored on iPhone");
  await ctx.close();
}
await browser.close();
console.log(errors ? `${errors} problem(s)` : "no problems");
process.exit(errors ? 1 : 0);
