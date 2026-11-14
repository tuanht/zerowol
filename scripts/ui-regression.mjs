// UI regression tests against a server started with the dev-fakes feature
// (five seeded fake machines). Usage: node scripts/ui-regression.mjs <base-url>
// Cases that change data clean up after themselves: "hostile machine name"
// creates a machine and deletes it again, the schedule cases remove the rules
// they create, and "no tz prompt" restores the saved timezone. Use
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
// the list intact. `only` limits a case to one theme; `context` adds browser context options.
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
    await p.waitForSelector(`${F} .segmented`);
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
    await p.waitForSelector(`${S} .zw-strip`);
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
    await p.waitForSelector(`${S} .segmented button[aria-checked=true]:has-text('Week')`);
    await p.click(`${S} .segmented button:has-text('Day')`);
    await p.waitForSelector(`${S} .segmented button[aria-checked=true]:has-text('Day')`);
    await tab(p, "Machines");
  },
  "schedules wide: 24-hour week grid, sticky header, month grid, popovers, breakpoint keeps the date": async (p) => {
    const S = "#view-schedules .page-current[data-name=schedules]";
    // Fixed occurrences: 8 distinct hours every day (3 of them once rules on the 10th), so the layout is measured at its limit.
    // All in the future (Skip is offered only ahead of time); the 23:30 rule is switched off (a grey dot / chip).
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
            repeat_kind: once ? "once" : "daily", date: d, time: h + ":30", at: 4e9, skipped: false, enabled: i !== 7, outcome: null });
        });
        d = addDay(d, 1);
      }
      return route.fulfill({ json: out });
    });
    const vp = p.viewportSize();
    try {
      await tab(p, "Schedules");
      await p.waitForSelector(`${S} .zw-strip`);
      // Phone: move to next week, then widen: the wide Week grid shows that same week.
      await p.click(`${S} .zw-strip [aria-label='Next week']`);
      await p.waitForTimeout(400);
      const monday = +(await p.locator(`${S} .zw-day .zw-num`).first().innerText());
      // The switched-off rule's dot is grey, the others keep their action colours.
      const dots = await p.locator(`${S} .zw-day >> nth=0`).locator(".dot").evaluateAll((d) => d.map((e) => e.className));
      if (dots.join("|") !== "dot wake|dot reboot|dot poweroff|dot off") throw new Error(`week strip dots ${dots.join("|")}`);
      await p.setViewportSize({ width: 1280, height: 800 });
      await p.waitForSelector(`${S} .zw-wgrid .zw-chip`);
      await p.waitForTimeout(400);
      const segs = await p.locator(`${S} .segmented button`).allInnerTexts();
      if (segs.join("|") !== "Week|Month") throw new Error(`wide views ${segs.join("|")}`);
      const w = await p.evaluate((sel) => {
        const pc = document.querySelector(sel + " .page-content"), g = document.querySelector(sel + " .zw-wgrid");
        const r = g.getBoundingClientRect();
        return { hours: g.querySelectorAll(".zw-whour").length, heads: [...g.querySelectorAll(".zw-whead")].map((h) => [h.textContent.trim(), h.getBoundingClientRect().width]),
          right: r.right, iw: innerWidth, wide: pc.scrollWidth - pc.clientWidth };
      }, S);
      if (w.hours !== 24) throw new Error(`${w.hours} hour rows, want 24`);
      if (+w.heads[0][0].split(" ")[1] !== monday) throw new Error(`wide week starts ${w.heads[0][0]}, phone week started on the ${monday}th`);
      const hw = w.heads.map((h) => h[1]);
      if (Math.max(...hw) - Math.min(...hw) > 1) throw new Error(`unequal day columns ${JSON.stringify(hw)}`);
      // Nothing past the right edge.
      if (w.wide > 0 || w.right > w.iw) throw new Error(`week grid wider than 1280 ${JSON.stringify(w)}`);
      // Not this week: opened scrolled to its first occurrence's hour (00:30), i.e. at the top.
      // Scrolled to the bottom, the day header row stays right under the navbar, opaque.
      const st = await p.evaluate(async (sel) => {
        const pc = document.querySelector(sel + " .page-content");
        const top0 = pc.scrollTop;
        pc.scrollTop = pc.scrollHeight;
        await new Promise((r) => setTimeout(r, 400));
        const nb = document.querySelector(sel + " .navbar") || document.querySelector("#view-schedules .navbar");
        const h = document.querySelector(sel + " .zw-whead");
        return { top0, head: h.getBoundingClientRect().top, nav: nb.getBoundingClientRect().bottom, bg: getComputedStyle(h).backgroundColor };
      }, S);
      if (st.top0 !== 0) throw new Error(`week without today opened scrolled to ${st.top0}px, want its 00:30 occurrence at the top`);
      if (Math.abs(st.head - st.nav) > 1) throw new Error(`sticky day header at ${st.head}px, navbar ends at ${st.nav}px`);
      if (/rgba\(.*, 0\)|transparent/.test(st.bg)) throw new Error(`sticky day header is see-through (${st.bg})`);
      // Chips carry their minutes.
      const chip = (await p.locator(`${S} .zw-wgrid .zw-chip >> nth=0`).innerText()).trim();
      if (chip !== "00:30 Workstation · Wake") throw new Error(`week chip "${chip}"`);
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
      // Opened after its content rendered, so F7 sized and placed it for all 8 rows: inside the window.
      await p.waitForTimeout(400);
      const pr = await p.evaluate(() => { const r = document.querySelector(".zw-day-pop.modal-in").getBoundingClientRect();
        return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, ih: innerHeight, iw: innerWidth }; });
      if (pr.top < 0 || pr.left < 0 || pr.bottom > pr.ih || pr.right > pr.iw) throw new Error(`day popover outside the window ${JSON.stringify(pr)}`);
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
  "schedules wide: current-time line, long press / double-click adds a one-off, chips and day taps unchanged": {
    // A browser zone far from the schedule one (the fake seed's Europe/Berlin): the line must follow the schedule's.
    context: { timezoneId: "Pacific/Kiritimati" },
    run: async (p) => {
      const S = SCHED;
      const F = ".zw-sched-form.modal-in";
      const vp = p.viewportSize();
      const before = await scheduleIds(p);
      // A chip at 05:00 every day, to long-press.
      await createRule(p, { machine_id: "dev-ws", action: "wake", time: "05:00", repeat: { kind: "daily" }, enabled: true });
      // Holds the mouse on an element's centre (or a point in it) for 650 ms: a long press.
      const hold = async (loc, dy = 0.5) => {
        await loc.scrollIntoViewIfNeeded();
        const b = await loc.boundingBox();
        await p.mouse.move(b.x + b.width / 2, b.y + b.height * dy);
        await p.mouse.down();
        await p.waitForTimeout(650);
        await p.mouse.up();
      };
      const form = async () => {
        await p.waitForSelector(`${F} #s-time`);
        await p.waitForTimeout(700);
        return [await p.inputValue(`${F} #s-repeat`), await p.inputValue(`${F} #s-date`), await p.inputValue(`${F} #s-time`)].join(" ");
      };
      const cancel = async () => {
        await p.click(`${F} .navbar .left a, ${F} .toolbar a:has-text('Cancel')`);
        await p.waitForSelector(F, { state: "detached", timeout: 3000 });
        await p.waitForTimeout(500); // the popup's backdrop fades out after it
      };
      const closePopover = async () => {
        await p.waitForTimeout(500); // F7 binds its outside-tap close once the popover has opened
        await p.touchscreen.tap(1270, 790);
        await p.waitForSelector(".popover.modal-in", { state: "detached" });
      };
      try {
        await tab(p, "Schedules");
        await p.setViewportSize({ width: 1280, height: 800 });
        await p.waitForSelector(`${S} .zw-wgrid .zw-whead.today`);
        await p.waitForTimeout(600);
        // (a) 24 hour rows; this week: the current time (in the schedule timezone, not the browser's) in red in its
        // hour row, the solid line inside today's column, and that row scrolled into view under the sticky header.
        const zones = await p.evaluate(async () => [(await (await fetch("/api/v1/settings")).json()).timezone, Intl.DateTimeFormat().resolvedOptions().timeZone]);
        if (zones[0] === zones[1]) throw new Error(`browser and schedule timezone are both ${zones[0]}: the check proves nothing`);
        await nowInView(p);
        // (b) next week: a long press on an empty slot opens New Schedule for a one-off there at HH:00.
        await p.click(`${S} [aria-label='Next week']`);
        await p.waitForSelector(`${S} .zw-wgrid:not(:has(.zw-whead.today))`);
        await p.waitForTimeout(400);
        const slot = await emptyCell(p, 10);
        const date = await slot.getAttribute("data-date");
        await hold(slot);
        const got = await form();
        if (got !== `once ${date} 10:00`) throw new Error(`long-press form "${got}", want "once ${date} 10:00"`);
        await cancel();
        // (d) a double-click does the same.
        const slot2 = await emptyCell(p, 15);
        const date2 = await slot2.getAttribute("data-date");
        await slot2.dblclick();
        const got2 = await form();
        if (got2 !== `once ${date2} 15:00`) throw new Error(`double-click form "${got2}", want "once ${date2} 15:00"`);
        await cancel();
        // A long press on a chip: its actions popover, not the form.
        await hold(p.locator(`${S} .zw-wcell[data-hour='5'] >> nth=3`).locator(".zw-chip"));
        await p.waitForSelector(".popover.modal-in:has-text('Skip this time')");
        if (await p.locator(F).count()) throw new Error("long press on a chip opened the form");
        await closePopover();
        // Last week: every slot is over, a long press opens nothing.
        await p.click(`${S} [aria-label='Previous week']`);
        await p.click(`${S} [aria-label='Previous week']`);
        await p.waitForTimeout(800);
        await hold(await emptyCell(p, 12));
        await p.waitForTimeout(800);
        if (await p.locator(`${F}, .popover.modal-in`).count()) throw new Error("long press on a past slot opened something");
        // (c) month, next month: a long press on a day opens the form for it at 08:00 (and not its popover) ...
        await p.click(`${S} .segmented button:has-text('Month')`);
        await p.waitForSelector(`${S} .zw-mgrid`);
        await p.click(`${S} [aria-label='Next month']`);
        await p.waitForTimeout(800);
        const day = p.locator(`${S} .zw-mcell:not(.out) >> nth=14`);
        const d = await day.getAttribute("data-date");
        await hold(day, 0.8);
        const got3 = await form();
        if (got3 !== `once ${d} 08:00`) throw new Error(`month long-press form "${got3}", want "once ${d} 08:00"`);
        await p.waitForTimeout(500);
        if (await p.locator(".zw-day-pop.modal-in").count()) throw new Error("the day popover opened after the long press");
        await cancel();
        // ... while a tap still opens the day's popover.
        await day.click();
        await p.waitForSelector(".popover.modal-in.zw-day-pop");
        if (await p.locator(F).count()) throw new Error("a tap on a day opened the form");
        await closePopover();
      } finally {
        await p.setViewportSize(vp);
        await removeSchedulesExcept(p, before);
      }
      await tab(p, "Machines");
    },
  },
  "schedules wide: opened after loading in the background, the week still scrolls to the current hour": async (p) => {
    // Machines tab at wide size: a schedule saved from a card bumps the rules revision, so the Schedules page loads
    // (and mounts its week grid) while its tab is hidden; opening the tab then shows the current hour.
    const before = await scheduleIds(p);
    const vp = p.viewportSize();
    try {
      await p.setViewportSize({ width: 1280, height: 800 });
      await p.waitForTimeout(400);
      // A machine without rules: its card's button opens the form straight away.
      const card = p.locator(CARDS).filter({ hasText: "Laptop" });
      await card.locator("[aria-label=Schedules]").click();
      const F = ".zw-sched-form.modal-in";
      await p.waitForSelector(`${F} #s-time`);
      await p.waitForTimeout(700);
      await p.fill(`${F} #s-time`, "02:00");
      await p.click(`${F} .navbar .right a, ${F} .toolbar a:has-text('Save')`);
      await p.waitForSelector(F, { state: "detached", timeout: 3000 });
      await p.waitForSelector(`${SCHED} .zw-wgrid`, { state: "attached" });
      if (await p.locator(`${SCHED} .zw-wgrid`).isVisible()) throw new Error("the Schedules grid is visible under the Machines tab");
      await tab(p, "Schedules");
      await p.waitForTimeout(800);
      await nowInView(p);
    } finally {
      await p.setViewportSize(vp);
      await removeSchedulesExcept(p, before);
    }
    await tab(p, "Machines");
  },
  "add schedule from card": async (p) => {
    const before = await scheduleIds(p);
    try {
      const card = p.locator(CARDS).filter({ hasText: "Workstation" });
      await card.locator("[aria-label=Schedules]").click();
      const F = ".zw-sched-form.modal-in";
      await p.waitForSelector(`${F} #s-time`);
      await p.waitForTimeout(700);
      if ((await p.inputValue(`${F} #s-machine`)) !== "dev-ws") throw new Error("form not opened for Workstation");
      await p.click(`${F} .zw-seg-row button:has-text('Power off')`);
      await p.fill(`${F} #s-time`, "02:00");
      if ((await p.inputValue(`${F} #s-repeat`)) !== "daily") throw new Error("repeat does not default to Daily");
      await p.click(`${F} .navbar .right a`); // iOS "Add", Material "Save"
      await p.waitForSelector(F, { state: "detached", timeout: 3000 });
      await card.locator(".zw-sched-btn .badge:text-is('1')").waitFor({ timeout: SETTLE });
      await openTomorrow(p);
      const row = p.locator(`${SCHED} .list li`).filter({ hasText: "02:00 · Workstation" });
      await row.waitFor({ timeout: SETTLE });
      const pill = (await row.locator(".zw-pill").innerText()).trim();
      if (pill !== "Power off") throw new Error(`tomorrow's Workstation row shows "${pill}", want Power off`);
      await tab(p, "Machines");
    } finally {
      await removeSchedulesExcept(p, before);
    }
  },
  "skip this time": async (p) => {
    const id = await createRule(p, { machine_id: "dev-ws", action: "poweroff", time: "02:00", repeat: { kind: "daily" }, enabled: true });
    try {
      await openTomorrow(p);
      const row = p.locator(`${SCHED} .list li`).filter({ hasText: "02:00 · Workstation" });
      await row.waitFor({ timeout: SETTLE });
      if (await row.locator("s").count()) throw new Error("occurrence struck through before Skip");
      const sheetTap = async (label) => {
        await row.locator("a.item-link").click();
        const btn = p.locator(`.actions-modal.modal-in .actions-button:has-text("${label}")`);
        await btn.waitFor();
        await p.waitForTimeout(400);
        await btn.click();
        await p.waitForSelector(".actions-modal.modal-in", { state: "detached", timeout: SETTLE });
      };
      await sheetTap("Skip this time");
      await row.locator("s:has-text('Workstation')").waitFor({ timeout: SETTLE });
      if (!(await row.innerText()).includes("skipped")) throw new Error("skipped occurrence not labelled skipped");
      await sheetTap("Don't skip");
      await row.locator("s").waitFor({ state: "detached", timeout: SETTLE });
      await tab(p, "Machines");
    } finally {
      await removeSchedules(p, [id]);
    }
  },
  "schedules tab switch keeps state": async (p) => {
    await tab(p, "Schedules");
    await p.waitForSelector(`${SCHED} .segmented`);
    await p.click(`${SCHED} .segmented button:has-text('Week')`);
    await p.waitForSelector(`${SCHED} .segmented button[aria-checked=true]:has-text('Week')`);
    await tab(p, "Machines");
    await p.waitForSelector(`${CUR}[data-name=machines]`);
    await tab(p, "Schedules");
    await p.waitForSelector(`${SCHED} .segmented button[aria-checked=true]:has-text('Week')`, { timeout: SETTLE });
    if (await p.locator(`${SCHED} .zw-day.sel`).count()) throw new Error("a day is selected in Week view");
    await tab(p, "Machines");
  },
  "no tz prompt": async (p) => {
    const saved = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
    try {
      const cleared = await p.evaluate(async () => {
        const r = await fetch("/api/v1/settings", { method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify({ timezone: null }) });
        return r.json();
      });
      // The regression server runs without TZ: nothing to fall back to.
      if (cleared.timezone !== null || cleared.source !== null) throw new Error(`settings after clearing ${JSON.stringify(cleared)}`);
      await p.locator(CARDS).filter({ hasText: "Workstation" }).locator("[aria-label=Schedules]").click();
      const F = ".zw-sched-form.modal-in";
      const use = p.locator(`${F} button:has-text('Use ')`);
      await use.waitFor();
      await p.waitForTimeout(700);
      if (await p.locator(`${F} #s-time`).count()) throw new Error("form fields shown before a timezone is set");
      const zone = (await use.innerText()).trim().replace(/^Use /, "");
      await use.click();
      await p.waitForSelector(`${F} #s-time`);
      await p.waitForSelector(`${F} .zw-seg-row .segmented`);
      const now = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
      if (now.timezone !== zone || now.source !== "settings") throw new Error(`picked ${zone}, settings ${JSON.stringify(now)}`);
      await p.click(`${F} .navbar .left a`);
      await p.waitForSelector(F, { state: "detached", timeout: SETTLE });
    } finally {
      const status = await p.evaluate(async (tz) => (await fetch("/api/v1/settings", {
        method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify({ timezone: tz }),
      })).status, saved.source === "settings" ? saved.timezone : null);
      if (status !== 200) throw new Error(`cleanup: restoring the timezone returned ${status}`);
    }
  },
  "settings: timezone search -> pick": async (p) => {
    await withTimezone(p, "Europe/Berlin", async () => {
      await tab(p, "Settings");
      await p.waitForSelector(`${CUR}[data-name=settings]`);
      await p.click(`${CUR} a.item-link:has-text('Timezone')`);
      await p.waitForSelector(`${CUR}[data-name=timezone]`);
      // Empty query: Suggested first, the stored zone checked.
      await p.waitForSelector(`${CUR} .zw-zones input[value='Europe/Berlin']:checked`, { state: "attached" });
      await searchZone(p, ".view.tab-active", "ho chi");
      const first = p.locator(`${CUR} .zw-zones li`).first();
      if ((await first.locator(".item-footer").innerText()).trim() !== "Asia/Ho_Chi_Minh") {
        throw new Error(`first match for "ho chi": ${await first.innerText()}`);
      }
      await first.click();
      await p.waitForSelector(`${CUR}[data-name=settings]`);
      await p.waitForSelector(`${CUR} a.item-link:has-text('Timezone'):has-text('Ho Chi Minh')`, { timeout: SETTLE });
      const now = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
      if (now.timezone !== "Asia/Ho_Chi_Minh") throw new Error(`picked Asia/Ho_Chi_Minh, settings ${JSON.stringify(now)}`);
      await tab(p, "Machines");
    });
  },
  "no tz prompt: choose another -> search -> pick": async (p) => {
    await withTimezone(p, null, async () => {
      await p.locator(CARDS).filter({ hasText: "Workstation" }).locator("[aria-label=Schedules]").click();
      const F = ".zw-sched-form.modal-in";
      await p.locator(`${F} button:has-text('Choose another')`).click();
      await searchZone(p, F, "berlin");
      const first = p.locator(`${F} .zw-zones li`).first();
      if ((await first.locator(".item-footer").innerText()).trim() !== "Europe/Berlin") throw new Error(`first match for "berlin": ${await first.innerText()}`);
      await first.click();
      await p.waitForSelector(`${F} #s-time`);
      const now = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
      if (now.timezone !== "Europe/Berlin") throw new Error(`picked Europe/Berlin, settings ${JSON.stringify(now)}`);
      await p.click(`${F} .navbar .left a`);
      await p.waitForSelector(F, { state: "detached", timeout: SETTLE });
    });
  },
  "schedules tab without tz: choose another -> pick": async (p) => {
    await withTimezone(p, null, async () => {
      await tab(p, "Schedules");
      await p.click(`${SCHED} button:has-text('Choose another')`);
      await p.waitForSelector(".view.tab-active .page-current[data-name=timezone]");
      await searchZone(p, ".view.tab-active", "new y");
      const first = p.locator(".view.tab-active .page-current .zw-zones li").first();
      if ((await first.locator(".item-footer").innerText()).trim() !== "America/New_York") throw new Error(`first match for "new y": ${await first.innerText()}`);
      await first.click();
      await p.waitForSelector(`${SCHED} .segmented`, { timeout: SETTLE * 2 });
      const now = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
      if (now.timezone !== "America/New_York") throw new Error(`picked America/New_York, settings ${JSON.stringify(now)}`);
      await tab(p, "Machines");
    });
  },
  "schedule sheet and form float on iOS phones (inset, rounded), Material sheet stays full width, Close X on the left": async (p, theme) => {
    const before = await scheduleIds(p);
    const gaps = (sel) => p.evaluate((sel) => {
      const r = document.querySelector(sel).getBoundingClientRect();
      return { left: r.left, right: innerWidth - r.right, bottom: innerHeight - r.bottom, top: r.top, w: r.width, iw: innerWidth };
    }, sel);
    try {
      await createRule(p, { machine_id: "dev-ws", action: "wake", time: "05:00", repeat: { kind: "daily" }, enabled: true });
      await p.reload();
      await p.waitForSelector(CARDS);
      // a machine with rules: its card button opens the sheet
      await p.locator(CARDS).filter({ hasText: "Workstation" }).locator("[aria-label=Schedules]").click();
      const S = ".zw-sched-sheet.modal-in";
      await p.waitForSelector(S);
      await p.waitForTimeout(700);
      const g = await gaps(S);
      if (theme === "ios") {
        if (g.left < 6 || g.right < 6 || g.bottom < 6) throw new Error(`iOS sheet not inset: ${JSON.stringify(g)}`);
        const rad = await p.evaluate((s) => getComputedStyle(document.querySelector(s)).borderTopLeftRadius + "/" + getComputedStyle(document.querySelector(s)).borderBottomRightRadius, S);
        if (rad.startsWith("0px") || rad.endsWith("0px")) throw new Error(`iOS sheet corners not rounded: ${rad}`);
      } else if (g.left !== 0 || g.right !== 0 || g.bottom !== 0) throw new Error(`Material sheet not full width: ${JSON.stringify(g)}`);
      const x = await p.locator(`${S} .navbar .left [aria-label=Close]`).boundingBox();
      if (!x || x.x > g.iw / 4) throw new Error("no Close button on the left of the sheet's navbar");
      if (await p.locator(`${S} .navbar :text('Done')`).count()) throw new Error("the sheet still has Done");
      await p.click(`${S} .navbar .left [aria-label=Close]`);
      await p.waitForSelector(S, { state: "detached", timeout: SETTLE }).catch(() => {});
      await p.waitForFunction(() => !document.querySelector(".zw-sched-sheet.modal-in"), null, { timeout: SETTLE });
      // the form
      await p.locator(CARDS).filter({ hasText: "Laptop" }).locator("[aria-label=Schedules]").click();
      const F = ".zw-sched-form.modal-in";
      await p.waitForSelector(`${F} #s-time`);
      await p.waitForTimeout(700);
      const f = await gaps(F);
      const top = await p.evaluate(() => parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--f7-safe-area-top")) || 0);
      if (theme === "ios") {
        if (f.left < 6 || f.right < 6 || f.bottom < 6 || f.top < 6 + top) throw new Error(`iOS form not inset: ${JSON.stringify(f)}, safe top ${top}`);
      } else if (f.left !== 0 || f.right !== 0 || f.bottom !== 0 || f.top !== 0) throw new Error(`Material form not full screen: ${JSON.stringify(f)}`);
      await p.click(`${F} .navbar .left a`);
      await p.waitForFunction(() => !document.querySelector(".zw-sched-form.modal-in"), null, { timeout: SETTLE });
    } finally {
      await removeSchedulesExcept(p, before);
    }
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

const SCHED = "#view-schedules .page-current[data-name=schedules]";
// A week-grid cell at `hour` holding no chip (a long press there is on free space).
async function emptyCell(p, hour) {
  const loc = p.locator(`${SCHED} .zw-wcell[data-hour='${hour}']:not(:has(.zw-chip))`).first();
  if (!(await loc.count())) throw new Error(`no free cell at ${hour}:00`);
  return loc;
}
// The wide week grid shows this week with 24 hour rows: the current time in the schedule timezone labels its hour
// row in red, today's solid line spans today's column, and the page is scrolled to put it near the top: within
// three rows under the sticky header (unless it's before 02:00, kept at the page top, or the page is at its end).
async function nowInView(p) {
  const n = await p.evaluate(async (sel) => {
    const tz = (await (await fetch("/api/v1/settings")).json()).timezone;
    const hm = (d) => new Intl.DateTimeFormat("en-GB", { timeZone: tz, hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
    const g = document.querySelector(sel + " .zw-wgrid");
    const t = g.querySelector(".zw-wnow-t"), line = g.querySelector(".zw-wnow.today"), col = g.querySelector(".zw-whead.today");
    const r = (e) => e && e.getBoundingClientRect();
    return { hours: g.querySelectorAll(".zw-whour").length, label: t && t.textContent.trim(), ok: [hm(new Date()), hm(new Date(Date.now() - 60e3))],
      line: r(line) && { l: r(line).left, r: r(line).right, y: r(line).top }, col: r(col) && { l: r(col).left, r: r(col).right, b: r(col).bottom }, ih: innerHeight,
      row: r(g.querySelector(".zw-wcell")).height, early: t && +t.textContent.slice(0, 2) < 2,
      end: (() => { const pc = g.closest(".page-content"); return pc.scrollTop >= pc.scrollHeight - pc.clientHeight - 1; })() };
  }, SCHED);
  if (n.hours !== 24) throw new Error(`${n.hours} hour rows, want 24`);
  if (!n.ok.includes(n.label)) throw new Error(`current-time label "${n.label}", want ${n.ok[0]}`);
  if (!n.line || Math.abs(n.line.l - n.col.l) > 1.5 || Math.abs(n.line.r - n.col.r) > 1.5)
    throw new Error(`today's line ${JSON.stringify(n.line)} not across today's column ${JSON.stringify(n.col)}`);
  if (!(n.line.y > n.col.b && n.line.y < n.ih)) throw new Error(`current time not scrolled into view (line at ${n.line.y}px, header ends ${n.col.b}px)`);
  if (!n.early && !n.end && n.line.y > n.col.b + 3 * n.row) throw new Error(`current time not scrolled near the top (line at ${n.line.y}px, header ends ${n.col.b}px)`);
}
async function scheduleIds(p) {
  return p.evaluate(async () => (await (await fetch("/api/v1/schedules")).json()).map((s) => s.id));
}
async function createRule(p, body) {
  return p.evaluate(async (body) => {
    const r = await fetch("/api/v1/schedules", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
    if (!r.ok) throw new Error("schedule create failed: " + r.status + " " + (await r.text()));
    return (await r.json()).id;
  }, body);
}
async function removeSchedules(p, ids) {
  const bad = await p.evaluate(async (ids) => {
    const out = [];
    for (const id of ids) {
      const r = await fetch("/api/v1/schedules/" + id, { method: "DELETE" });
      if (!r.ok && r.status !== 404) out.push(id + ": " + r.status);
    }
    return out;
  }, ids);
  if (bad.length) throw new Error("cleanup: schedule DELETE failed " + bad.join(", "));
}
async function removeSchedulesExcept(p, keep) {
  await removeSchedules(p, (await scheduleIds(p)).filter((id) => !keep.includes(id)));
}
// Schedules tab, phone Day view, on tomorrow (the next strip cell, or the first one of next week).
async function openTomorrow(p) {
  await tab(p, "Schedules");
  await p.waitForSelector(`${SCHED} .zw-strip .zw-day.today`);
  const i = await p.evaluate((sel) => [...document.querySelectorAll(sel + " .zw-strip .zw-day")].findIndex((d) => d.classList.contains("today")), SCHED);
  if (i === 6) {
    await p.click(`${SCHED} .zw-strip [aria-label='Next week']`);
    await p.waitForTimeout(400);
  }
  await p.click(`${SCHED} .zw-strip .zw-day >> nth=${(i + 1) % 7}`);
  await p.waitForSelector(`${SCHED} .segmented button[aria-checked=true]:has-text('Day')`);
  await p.waitForSelector(`${SCHED} .zw-day.sel:not(.today)`);
}
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
// Runs `fn` with the schedule timezone set to `zone` (null clears it), then restores the saved one.
async function withTimezone(p, zone, fn) {
  const patch = (tz) => p.evaluate(async (tz) => (await fetch("/api/v1/settings", {
    method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify({ timezone: tz }),
  })).status, tz);
  const saved = await p.evaluate(async () => (await fetch("/api/v1/settings")).json());
  if ((await patch(zone)) !== 200) throw new Error(`setting the timezone to ${zone} failed`);
  try {
    await fn();
  } finally {
    const status = await patch(saved.source === "settings" ? saved.timezone : null);
    if (status !== 200) throw new Error(`cleanup: restoring the timezone returned ${status}`);
  }
}
// Types into the timezone searchbar under `scope` (on iOS the page's navbar lives in the view, not the page).
// Waits out Framework7's 400 ms iOS enable delay after the focus: a searchbar removed before it ends
// (the form's list step closes on a pick) throws from that timer, and a person can't type and pick that fast.
async function searchZone(p, scope, q) {
  const input = p.locator(`${scope} .searchbar input`);
  await input.waitFor();
  await p.waitForTimeout(500);
  await input.fill(q);
  await p.waitForTimeout(600);
}
async function tab(p, label) {
  await p.click(`.toolbar .tab-link:has-text('${label}')`);
  await p.waitForTimeout(300);
}

const browser = await chromium.launch();
let failures = 0;
for (const [theme, device] of [["md", "Pixel 7"], ["ios", "iPhone 15"]]) {
  for (const [name, c] of Object.entries(cases)) {
    const { run, only, context } = typeof c === "function" ? { run: c } : c;
    if (only && only !== theme) continue;
    const ctx = await browser.newContext({ ...devices[device], ...context });
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
