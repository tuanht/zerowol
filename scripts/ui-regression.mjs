// UI regression tests against a server started with the dev-fakes feature
// (five seeded fake machines). Usage: node scripts/ui-regression.mjs <base-url>
// Read-only except for the one self-cleaning "hostile machine name" case, which
// creates a machine through the API and deletes it again. Use
// scripts/ui-regression.sh to build, start a throwaway server and run this.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
let pw;
try { pw = require("playwright"); } catch {
  const root = (await import("node:child_process")).execSync("npm root -g").toString().trim();
  pw = createRequire(root + "/")("playwright");
}
const { chromium, devices } = pw;
const base = process.argv[2];
if (!base) { console.error("usage: node scripts/ui-regression.mjs <base-url>"); process.exit(2); }

// Everything is scoped to the active tab's current page: Framework7 keeps the
// previous page (and the other tabs) in the DOM.
const CUR = ".view.tab-active .page-current";
const BACK = `${CUR} .navbar .left .back`;
const CARDS = `${CUR} .card.machine`;
const HOSTILE = "<img src=x onerror=window.__x=1>";
const OLD_WORDING = /Restart|Shut Down|Shutting down/;
// The app's idle 5 s poll would repopulate a blank list and hide the bug, so the
// tests switch that timer off: only the repaint on returning to the list may
// fill it.
const SETTLE = 1500;
const noPoll = () => {
  const st = window.setTimeout.bind(window);
  window.setTimeout = (f, d, ...a) => (d === 5000 ? 0 : st(f, d, ...a));
};

// Each case starts on the Machines tab with the list and must end there with
// the list intact. `only` limits a case to one theme.
const cases = {
  "machine -> back": async (p) => {
    await openMachine(p);
    await back(p);
  },
  "machine -> agent install -> back, back (regression: list went blank)": async (p) => {
    await openMachine(p);
    await openPairing(p);
    await back(p);
    await back(p);
  },
  "machine -> agent install -> Done, back": async (p) => {
    await openMachine(p);
    await openPairing(p);
    await p.click(`${CUR} .navbar .right .link:has-text('Done')`);
    await p.waitForSelector(".view.tab-active .page-current[data-name=machine]");
    await p.waitForTimeout(500);
    await back(p);
  },
  "agent install via the card's Pair button -> back": async (p) => {
    await p.click(`${CARDS} button:has-text('Pair') >> nth=0`);
    await p.waitForSelector(`${CUR}[data-name=pair]`);
    await p.waitForTimeout(500);
    await back(p);
  },
  "machine -> agent install -> back, machine again -> back, back": async (p) => {
    await openMachine(p);
    await openPairing(p);
    await back(p);
    await openPairing(p);
    await back(p);
    await back(p);
  },
  "settings tab -> machines tab keeps list": async (p) => {
    await tab(p, "Settings");
    await p.waitForSelector(`${CUR}[data-name=settings]`);
    await tab(p, "Machines");
    await p.waitForSelector(`${CUR}[data-name=machines]`);
  },
  "tab switch keeps machines stack": async (p) => {
    await openMachine(p);
    await tab(p, "Settings");
    await p.waitForSelector(`${CUR}[data-name=settings]`);
    await tab(p, "Machines");
    await p.waitForSelector(`${CUR}[data-name=machine]`, { timeout: SETTLE });
    await back(p);
  },
  "android back pops within tab": {
    only: "md",
    run: async (p) => {
      const origin = new URL(base).origin;
      await openMachine(p);
      await p.goBack();
      await p.waitForSelector(`${CUR}[data-name=machines]`, { timeout: SETTLE });
      await p.waitForTimeout(450);
      if (new URL(p.url()).origin !== origin) throw new Error("left the app: " + p.url());
    },
  },
  "FAB only on the Machines root (md)": {
    only: "md",
    run: async (p) => {
      const fab = () => p.locator(".zw-fab:visible").count();
      if ((await fab()) !== 1) throw new Error("no FAB on the Machines root");
      // Material guideline: FAB sits 16dp above the bottom navigation bar
      const gap = await p.evaluate(() => document.querySelector(".toolbar.zw-nav").getBoundingClientRect().top - document.querySelector(".zw-fab").getBoundingClientRect().bottom);
      if (Math.abs(gap - 16) > 2) throw new Error(`FAB is ${gap}px above the navigation bar, want 16`);
      await openMachine(p);
      if ((await fab()) !== 0) throw new Error("FAB on the edit page");
      await back(p);
      if ((await fab()) !== 1) throw new Error("FAB missing after back");
      await p.click(".zw-fab a");
      await p.waitForSelector(`${CUR}[data-name=machine] #f-name`);
      await p.waitForTimeout(500);
      if ((await fab()) !== 0) throw new Error("FAB on the add page");
      await back(p);
      if ((await fab()) !== 1) throw new Error("FAB missing after back from add");
      await openMachine(p);
      await p.goBack(); // Android back path
      await p.waitForSelector(`${CUR}[data-name=machines]`, { timeout: SETTLE });
      await p.waitForTimeout(450);
      if ((await fab()) !== 1) throw new Error("FAB missing after system back");
    },
  },
  "resize switches nav layout": async (p, theme) => {
    const vp = p.viewportSize();
    const nav = () => p.evaluate(() => {
      // iOS: the floating glass pill is the toolbar-pane; Material: the bar itself
      const r = document.querySelector(document.documentElement.classList.contains("ios") ? ".zw-nav .toolbar-pane" : ".toolbar.zw-nav").getBoundingClientRect();
      return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, width: r.width, ih: innerHeight, iw: innerWidth };
    });
    await p.setViewportSize({ width: 375, height: 812 });
    await p.waitForTimeout(300);
    let r = await nav();
    // iOS: floating capsule a few px above the bottom edge, inset from the sides;
    // Material: navigation bar flush with the bottom edge.
    if (theme === "ios" && !(r.ih - r.bottom >= 4 && r.ih - r.bottom <= 40 && r.left >= 8 && r.iw - r.right >= 8))
      throw new Error(`375px: iOS floating tab bar not at the bottom (${JSON.stringify(r)})`);
    if (theme === "md" && Math.abs(r.bottom - r.ih) > 2) throw new Error(`375px: tab bar not at the bottom (${JSON.stringify(r)})`);
    await p.setViewportSize({ width: 1280, height: 800 });
    await p.waitForTimeout(300);
    r = await nav();
    if (theme === "ios" && !(r.top < 40)) throw new Error(`1280px: iOS capsule not at the top (${JSON.stringify(r)})`);
    if (theme === "md" && !(Math.abs(r.width - 80) <= 1 && r.left === 0)) throw new Error(`1280px: no 80px Material rail (${JSON.stringify(r)})`);
    if (theme === "md" && (await p.locator(".zw-fab").count())) {
      // M3 navigation rail: the FAB is centred in the rail
      const f = await p.evaluate(() => document.querySelector(".zw-fab").getBoundingClientRect().toJSON());
      if (Math.abs(f.left + f.width / 2 - 40) > 1) throw new Error(`1280px: FAB not centred in the rail (${JSON.stringify(f)})`);
    }
    await p.setViewportSize(vp);
    await p.waitForTimeout(300);
  },
  "hostile machine name renders as text": async (p) => {
    const id = await p.evaluate(async (name) => {
      const r = await fetch("/api/v1/machines", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name, mac: "02:00:5E:00:00:63", ip: "192.0.2.99" }),
      });
      if (!r.ok) throw new Error("create failed: " + r.status + " " + (await r.text()));
      return (await r.json()).id;
    }, HOSTILE);
    try {
      // A new machine can't be paired here, so report it as online and paired
      // to get its Power off button (and confirm) on the card.
      await p.route("**/api/v1/machines", async (route) => {
        if (route.request().method() !== "GET") return route.continue();
        const res = await route.fetch();
        const list = await res.json();
        for (const m of list) if (m.id === id) m.status = { online: true, agent: "paired", uptime: 600 };
        await route.fulfill({ response: res, json: list });
      });
      await p.reload();
      const card = p.locator(CARDS).filter({ hasText: HOSTILE });
      await card.waitFor();
      await card.locator("button:has-text('Power off')").click();
      await p.waitForSelector(".dialog.modal-in");
      const text = await p.locator(".dialog.modal-in").innerText();
      if (!text.includes("<img")) throw new Error("confirm does not show the name as text: " + JSON.stringify(text));
      if (await p.evaluate(() => window.__x !== undefined)) throw new Error("machine name was run as HTML");
      await p.click(".dialog.modal-in .dialog-button:has-text('Cancel')");
      await p.waitForTimeout(400);
      if (await p.evaluate(() => window.__x !== undefined)) throw new Error("machine name was run as HTML");
      // Toast: back to its real (offline) status, Wake it, and have the server
      // answer "already online" so the app toasts "<name> is already online".
      await p.unroute("**/api/v1/machines");
      await p.route(`**/api/v1/machines/${id}/wake`, (route) =>
        route.fulfill({ json: { result: "already online" } }));
      await p.reload();
      const offCard = p.locator(CARDS).filter({ hasText: HOSTILE });
      await offCard.locator("button:has-text('Wake')").click();
      const toast = p.locator(".toast").filter({ hasText: "is already online" });
      await toast.waitFor({ timeout: SETTLE });
      const tt = await toast.innerText();
      if (!tt.includes("<img")) throw new Error("toast does not show the name as text: " + JSON.stringify(tt));
      if (await p.evaluate(() => window.__x !== undefined)) throw new Error("machine name was run as HTML in the toast");
    } finally {
      await p.unrouteAll({ behavior: "ignoreErrors" });
      const status = await p.evaluate(
        async (id) => (await fetch("/api/v1/machines/" + id, { method: "DELETE" })).status, id);
      if (status < 200 || status >= 300) throw new Error(`cleanup: DELETE of the hostile machine returned ${status}`);
    }
    await p.reload();
    await p.waitForSelector(CARDS);
  },
  "card schedule button -> form layout -> cancel": async (p, theme) => {
    const card = p.locator(CARDS).filter({ hasText: "Media PC" });
    const clock = card.locator("[aria-label=Schedules]");
    const wakeBtn = card.locator("button:has-text('Wake')");
    const [cb, wb] = [await clock.boundingBox(), await wakeBtn.boundingBox()];
    // iOS: the icon-only cell is narrower than the labeled split cell. Both: the icon sits on the labels' centre line.
    if (theme === "ios" && !(cb.width < wb.width / 2)) throw new Error(`clock cell ${cb.width}px vs Wake ${wb.width}px`);
    const mid = async (loc) => { const b = await loc.boundingBox(); return b.y + b.height / 2; };
    const dy = (await mid(clock.locator("svg"))) - (await mid(wakeBtn.locator("svg")));
    if (Math.abs(dy) > 1.5) throw new Error(`clock icon ${dy}px off the Wake icon's centre line`);
    await clock.click();
    const F = ".zw-sched-form.modal-in";
    await p.waitForSelector(F);
    await p.waitForTimeout(700);
    if (await p.locator(`${F} .zw-pick`).isVisible()) {
      await p.click(`${F} button:has-text('Use ')`);
      await p.waitForSelector(`${F} .segmented`);
    }
    // The action segmented control spans its list row (less the row's 16px side padding).
    const [seg, row] = [await p.locator(`${F} .segmented`).boundingBox(), await p.locator(`${F} .zw-seg-row`).boundingBox()];
    if (Math.abs(seg.width - (row.width - 32)) > 1) throw new Error(`segmented ${seg.width}px in a ${row.width}px row`);
    await p.click(`${F} .navbar .left a`);
    await p.waitForSelector(".zw-sched-form.modal-in", { state: "detached", timeout: SETTLE }).catch(() => {});
    if (await p.locator(F).count()) throw new Error("form still open after Cancel");
  },
  "schedules tab: week strip layout, day/week switch": async (p) => {
    await tab(p, "Schedules");
    const S = "#view-schedules .page-current[data-name=schedules]";
    await p.waitForSelector(`${S} .zw-strip, ${S} button:has-text('Use ')`);
    if (await p.locator(`${S} button:has-text('Use ')`).count()) {
      await p.click(`${S} button:has-text('Use ')`);
      await p.waitForSelector(`${S} .zw-strip`);
    }
    // Our strip: 7 equal day cells between the arrows, inside the viewport, no horizontal overflow.
    const m = await p.evaluate((sel) => {
      const strip = document.querySelector(sel + " .zw-strip");
      const days = [...strip.querySelectorAll(".zw-day")].map((d) => d.getBoundingClientRect().width);
      const r = strip.getBoundingClientRect();
      const blk = strip.parentElement, cs = getComputedStyle(blk);
      const inner = blk.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      return { days, left: r.left, right: r.right, iw: innerWidth, over: strip.scrollWidth - strip.clientWidth,
        block: blk.classList.contains("block") ? cs.display : "missing", width: r.width, inner };
    }, S);
    if (m.days.length !== 7) throw new Error(`${m.days.length} day cells`);
    if (Math.max(...m.days) - Math.min(...m.days) > 1) throw new Error(`unequal day cells ${JSON.stringify(m.days)}`);
    if (m.left < 0 || m.right > m.iw || m.over > 0) throw new Error(`strip outside the viewport ${JSON.stringify(m)}`);
    // Our grid sits inside a plain F7 .block (whose own display is untouched) and fills its content box.
    if (m.block !== "block") throw new Error(`strip's F7 .block has display ${m.block}`);
    if (Math.abs(m.width - m.inner) > 1) throw new Error(`strip ${m.width}px in a ${m.inner}px block`);
    await p.click(`${S} .segmented button:has-text('Week')`);
    await p.waitForSelector(`${S} .segmented button.button-active:has-text('Week')`);
    await p.click(`${S} .segmented button:has-text('Day')`);
    await p.waitForSelector(`${S} .segmented button.button-active:has-text('Day')`);
    await tab(p, "Machines");
  },
  "schedules wide: week grid fits, month grid, popovers, breakpoint keeps the date": async (p) => {
    const S = "#view-schedules .page-current[data-name=schedules]";
    // Fixed occurrences: 8 distinct hours every day (3 of them once rules on the 10th), so the layout is measured at its limit.
    const HOURS = ["00", "02", "05", "07", "08", "12", "18", "23"];
    const ACTS = ["wake", "reboot", "poweroff"];
    await p.route("**/api/v1/schedules/occurrences?**", async (route) => {
      const u = new URL(route.request().url());
      // A month's range answers late, so the switch from Week shows the gap before the matching data arrives.
      if (u.searchParams.get("to") > addDay(u.searchParams.get("from"), 7)) await new Promise((r) => setTimeout(r, 800));
      const out = [];
      for (let d = u.searchParams.get("from"); d <= u.searchParams.get("to"); ) {
        HOURS.forEach((h, i) => {
          const once = d.endsWith("-10") && i < 3;
          out.push({ schedule_id: (once ? "o" : "r") + i, machine_id: "dev-ws", machine_name: "Workstation", action: ACTS[i % 3],
            repeat_kind: once ? "once" : "daily", date: d, time: h + ":30", at: 0, skipped: false, enabled: true, outcome: null });
        });
        d = addDay(d, 1);
      }
      return route.fulfill({ json: out });
    });
    const vp = p.viewportSize();
    try {
      await tab(p, "Schedules");
      await p.waitForSelector(`${S} .zw-strip, ${S} button:has-text('Use ')`);
      if (await p.locator(`${S} button:has-text('Use ')`).count()) {
        await p.click(`${S} button:has-text('Use ')`);
        await p.waitForSelector(`${S} .zw-strip`);
      }
      // Phone: move to next week, then widen: the wide Week grid shows that same week.
      await p.click(`${S} .zw-strip [aria-label='Next week']`);
      await p.waitForTimeout(400);
      const monday = +(await p.locator(`${S} .zw-day .zw-num`).first().innerText());
      await p.setViewportSize({ width: 1280, height: 800 });
      await p.waitForSelector(`${S} .zw-wgrid .zw-chip`);
      await p.waitForTimeout(400);
      const segs = await p.locator(`${S} .segmented button`).allInnerTexts();
      if (segs.join("|") !== "Week|Month") throw new Error(`wide views ${segs.join("|")}`);
      const w = await p.evaluate((sel) => {
        const pc = document.querySelector(sel + " .page-content"), g = document.querySelector(sel + " .zw-wgrid");
        const r = g.getBoundingClientRect();
        return { hours: g.querySelectorAll(".zw-whour").length, heads: [...g.querySelectorAll(".zw-whead")].map((h) => [h.textContent.trim(), h.getBoundingClientRect().width]),
          bottom: r.bottom, right: r.right, ih: innerHeight, iw: innerWidth, scroll: pc.scrollHeight - pc.clientHeight };
      }, S);
      if (w.hours !== 8) throw new Error(`${w.hours} hour rows, want 8`);
      if (+w.heads[0][0].split(" ")[1] !== monday) throw new Error(`wide week starts ${w.heads[0][0]}, phone week started on the ${monday}th`);
      const hw = w.heads.map((h) => h[1]);
      if (Math.max(...hw) - Math.min(...hw) > 1) throw new Error(`unequal day columns ${JSON.stringify(hw)}`);
      // 8 hours fit a 1280x800 window: no vertical scroll, nothing past the edges.
      if (w.scroll > 0 || w.bottom > w.ih || w.right > w.iw) throw new Error(`week grid doesn't fit 1280x800 ${JSON.stringify(w)}`);
      // A repeating chip's actions open as a popover on wide screens.
      await p.click(`${S} .zw-wgrid .zw-chip >> nth=0`);
      await p.waitForSelector(".popover.modal-in:has-text('Skip this time')");
      await p.waitForTimeout(500); // F7 binds its outside-tap close once the popover has opened
      await p.touchscreen.tap(1270, 790); // outside it; a tap, as F7 pairs touchstart with the click on touch devices
      await p.waitForSelector(".popover.modal-in", { state: "detached" });
      await p.click(`${S} .segmented button:has-text('Month')`);
      await p.waitForTimeout(200);
      // Until the month's data arrives, neither the week's occurrences in a month grid nor the empty state.
      if (await p.locator(`${S} .zw-mgrid, ${S} :text('No schedules')`).count()) throw new Error("month view drew before its data arrived");
      await p.waitForSelector(`${S} .zw-mgrid`);
      await p.waitForTimeout(400);
      const m = await p.evaluate((sel) => {
        const pc = document.querySelector(sel + " .page-content");
        const c = [...document.querySelectorAll(sel + " .zw-mcell")].map((e) => e.getBoundingClientRect());
        return { n: c.length, w: c.map((r) => r.width), h: c.map((r) => r.height), bottom: Math.max(...c.map((r) => r.bottom)),
          right: Math.max(...c.map((r) => r.right)), left: Math.min(...c.map((r) => r.left)), ih: innerHeight, iw: innerWidth,
          scroll: pc.scrollHeight - pc.clientHeight, more: document.querySelector(sel + " .zw-more")?.textContent };
      }, S);
      if (m.n !== 42) throw new Error(`${m.n} month cells`);
      const spread = (a) => Math.max(...a) - Math.min(...a);
      if (spread(m.w) > 1 || spread(m.h) > 1) throw new Error(`unequal month cells ${JSON.stringify({ w: m.w.slice(0, 7), h: m.h.slice(0, 7) })}`);
      if (m.left < 0 || m.right > m.iw || m.bottom > m.ih || m.scroll > 0) throw new Error(`month grid outside 1280x800 ${JSON.stringify(m)}`);
      if (m.more !== "+1") throw new Error(`3 once chips show "${m.more}", want "+1"`);
      // "+N" at a cell's right edge stays clear of the next day's number, with a column hairline between them.
      const gap = await p.evaluate((sel) => {
        const more = document.querySelector(sel + " .zw-more"), next = more.closest(".zw-mcell").nextElementSibling;
        return { px: next.querySelector(".zw-mnum").getBoundingClientRect().left - more.getBoundingClientRect().right, line: getComputedStyle(next).borderLeftWidth };
      }, S);
      if (gap.px < 8 || gap.line !== "1px") throw new Error(`"+N" ${gap.px}px from the next day's number, divider ${gap.line}`);
      if (await p.evaluate(() => localStorage.getItem("zw-sched-view-wide")) !== "month") throw new Error("wide view not stored");
      // A day: its agenda in a popover; an item closes it and opens that occurrence's actions.
      await p.click(`${S} .zw-mcell:not(.out) >> nth=14`);
      await p.waitForSelector(".popover.modal-in.zw-day-pop");
      const rows = await p.locator(".zw-day-pop .item-link").count();
      if (rows !== 8) throw new Error(`day popover has ${rows} rows, want 8`);
      await p.click(".zw-day-pop .item-link >> nth=1");
      await p.waitForSelector(".popover.modal-in:has-text('Skip this time')");
      if (await p.locator(".zw-day-pop.modal-in").count()) throw new Error("day popover still open under the actions");
      await p.waitForTimeout(500); // F7 binds its outside-tap close once the popover has opened
      await p.touchscreen.tap(1270, 790); // outside it; a tap, as F7 pairs touchstart with the click on touch devices
      await p.waitForSelector(".popover.modal-in", { state: "detached" });
      // Back to phone size: Day | Week again, the phone choice untouched.
      await p.setViewportSize(vp);
      await p.waitForSelector(`${S} .zw-strip`);
      const phone = await p.locator(`${S} .segmented button`).allInnerTexts();
      if (phone.join("|") !== "Day|Week") throw new Error(`phone views ${phone.join("|")}`);
    } finally {
      await p.setViewportSize(vp);
      await p.unroute("**/api/v1/schedules/occurrences?**");
    }
    await tab(p, "Machines");
  },
  "no old wording": async (p) => {
    const check = async (where) => {
      const t = await p.evaluate(() => document.body.innerText);
      const m = OLD_WORDING.exec(t);
      if (m) throw new Error(`"${m[0]}" on ${where}`);
    };
    await check("Machines");
    await openMachine(p);
    await check("the edit page");
    await back(p);
    await tab(p, "Settings");
    await p.waitForSelector(`${CUR}[data-name=settings]`);
    await check("Settings");
    await tab(p, "Machines");
    await p.waitForSelector(`${CUR}[data-name=machines]`);
  },
};

function addDay(d, n) {
  const t = new Date(d + "T00:00:00Z");
  t.setUTCDate(t.getUTCDate() + n);
  return t.toISOString().slice(0, 10);
}
async function openMachine(p) {
  await p.click(`${CARDS} a.item-link >> nth=0`);
  await p.waitForSelector(`${CUR}[data-name=machine]`);
  await p.waitForTimeout(500); // let the push transition finish
}
async function openPairing(p) {
  await p.click(`${CUR} >> text=Agent install command`);
  await p.waitForSelector(`${CUR}[data-name=pair]`);
  await p.waitForTimeout(500);
}
async function back(p) {
  await p.click(BACK);
  await p.waitForTimeout(450);
}
async function tab(p, label) {
  await p.click(`.toolbar .tab-link:has-text('${label}')`);
  await p.waitForTimeout(300);
}

const browser = await chromium.launch();
let failures = 0;
for (const [theme, device] of [["md", "Pixel 7"], ["ios", "iPhone 15"]]) {
  for (const [name, c] of Object.entries(cases)) {
    const { run, only } = typeof c === "function" ? { run: c } : c;
    if (only && only !== theme) continue;
    const ctx = await browser.newContext({ ...devices[device] });
    await ctx.addInitScript((t) => localStorage.setItem("zw-theme", t), theme);
    await ctx.addInitScript(noPoll);
    const page = await ctx.newPage();
    const errs = [];
    page.on("pageerror", (e) => errs.push("pageerror: " + e.message));
    const label = `[${theme}] ${name}`;
    try {
      await page.goto(base);
      await page.waitForSelector(CARDS);
      const total = await page.locator(CARDS).count();
      if (total !== 5) throw new Error(`expected 5 machines at start, got ${total}`);
      await run(page, theme);
      await page.waitForSelector("#view-machines.tab-active .page-current[data-name=machines]", { timeout: SETTLE });
      await page.waitForFunction(
        ([sel, n]) => document.querySelectorAll(sel).length === n, [CARDS, total], { timeout: SETTLE });
      if (await page.locator(`${CUR} .foot`).isHidden()) throw new Error("footer hint is hidden although machines exist");
      if (errs.length) throw new Error(errs.join("; "));
      console.log("PASS ", label);
    } catch (e) {
      failures++;
      console.error("FAIL ", label, "-", e.message.split("\n")[0]);
    }
    await ctx.close();
  }
}
// iPhone home-screen app: iOS blurs a band below the status bar whichever theme is picked,
// so both themes must push the navbar title down by the extra 16px gap (compared with Safari).
async function largeTitleTop(theme, standalone) {
  const ctx = await browser.newContext({ ...devices["iPhone 15"] });
  await ctx.addInitScript((t) => localStorage.setItem("zw-theme", t), theme);
  await ctx.addInitScript(noPoll);
  if (standalone) await ctx.addInitScript(() => Object.defineProperty(Navigator.prototype, "standalone", { get: () => true }));
  const page = await ctx.newPage();
  try {
    await page.goto(base);
    await page.waitForSelector(CARDS);
    return await page.evaluate((cur) => document.querySelector(cur + " .navbar .title-large-text").getBoundingClientRect().top, CUR);
  } finally {
    await ctx.close();
  }
}
for (const theme of ["ios", "md"]) {
  const label = `[${theme}] iPhone home-screen app: title clear of the status-bar blur`;
  try {
    const shift = (await largeTitleTop(theme, true)) - (await largeTitleTop(theme, false));
    if (Math.abs(shift - 16) > 1) throw new Error(`title moved ${shift}px down in the home-screen app, want 16`);
    console.log("PASS ", label);
  } catch (e) {
    failures++;
    console.error("FAIL ", label, "-", e.message.split("\n")[0]);
  }
}
await browser.close();
if (failures) { console.error(`${failures} UI regression case(s) failed`); process.exit(1); }
console.log("all UI regression cases passed");
