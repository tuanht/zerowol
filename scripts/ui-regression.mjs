// UI regression tests against a server started with the dev-fakes feature
// (five seeded fake machines). Usage: node scripts/ui-regression.mjs <base-url>
// Read-only: it never creates, edits or deletes machines. Use scripts/ui-regression.sh
// to build, start a throwaway server and run this.
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

const BACK = ".page-current .navbar .left .back";
const CARDS = "#list .card";
// The app's idle 5 s poll would repopulate a blank list and hide the bug, so the
// tests switch that timer off: only the repaint on returning to the home page
// may fill the list.
const SETTLE = 1500;
const noPoll = () => {
  const st = window.setTimeout.bind(window);
  window.setTimeout = (f, d, ...a) => (d === 5000 ? 0 : st(f, d, ...a));
};

// Each case starts on the home screen and must end on it with the list intact.
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
    await p.click(".page-current .navbar .right .back"); // "Done"
    await p.waitForSelector(".page-current[data-name=machine]");
    await back(p);
  },
  "agent install via the card's Pair button -> back": async (p) => {
    await p.click("#list .card button:has-text('Pair') >> nth=0");
    await p.waitForSelector(".page-current[data-name=pair]");
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
  "settings -> back": async (p) => {
    await p.click("#gear:visible, #gear-top:visible");
    await p.waitForSelector(".page-current[data-name=settings]");
    await back(p);
  },
};

async function openMachine(p) {
  await p.click(`${CARDS} a.item-link >> nth=0`);
  await p.waitForSelector(".page-current[data-name=machine]");
  await p.waitForTimeout(500); // let the push transition finish
}
async function openPairing(p) {
  await p.click("text=Agent install command");
  await p.waitForSelector(".page-current[data-name=pair]");
  await p.waitForTimeout(500);
}
async function back(p) {
  await p.click(BACK);
  await p.waitForTimeout(450);
}

const browser = await chromium.launch();
let failures = 0;
for (const [theme, device] of [["md", "Pixel 7"], ["ios", "iPhone 15"]]) {
  for (const [name, run] of Object.entries(cases)) {
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
      await run(page);
      await page.waitForSelector(".page-current[data-name=home]", { timeout: SETTLE });
      await page.waitForFunction((n) => document.querySelectorAll("#list .card").length === n, total, { timeout: SETTLE });
      if (await page.locator("#foot").isHidden()) throw new Error("footer hint is hidden although machines exist");
      if (errs.length) throw new Error(errs.join("; "));
      console.log("ok   ", label);
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
