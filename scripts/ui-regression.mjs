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
      const r = document.querySelector(".toolbar.zw-nav").getBoundingClientRect();
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
    if (theme === "md" && !(r.width <= 80 && r.left === 0)) throw new Error(`1280px: no Material rail (${JSON.stringify(r)})`);
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
await browser.close();
if (failures) { console.error(`${failures} UI regression case(s) failed`); process.exit(1); }
console.log("all UI regression cases passed");
