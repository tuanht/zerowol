// WARNING: this script DELETES ALL MACHINES on the target server (and removes the tokens it creates).
// Point it only at a throwaway instance, e.g. ZEROWOL_DATA=$(mktemp -d) ZEROWOL_PORT=18090 cargo run -p zerowol
// Usage: NODE_PATH=$(npm root -g) node scripts/ui-check.mjs http://127.0.0.1:8090
// Writes screenshots (ios/md x light/dark x list, edit, pairing, confirm, settings) to ./screenshots/.
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
    const got = await page.evaluate(() => app.theme);
    if (got !== theme) fail(`${device}: expected theme ${theme}, got ${got}`);
    const shot = async (name) => {
      await page.waitForTimeout(600);
      const p = `screenshots/${theme}-${scheme}-${name}.png`;
      await page.screenshot({ path: p });
      console.log("wrote", p);
    };
    // FAB positions: settings bottom-left, add bottom-right.
    const g = await page.locator("#gear").boundingBox(), a = await page.locator("#add").boundingBox();
    const vp = page.viewportSize();
    if (!(g.x < vp.width / 2 && a.x > vp.width / 2 && g.y > vp.height / 2 && a.y > vp.height / 2)) fail("fab positions");
    await shot("empty");
    await page.click("#add");
    await page.waitForSelector(".page[data-name=machine] #f-name");
    await page.fill("#f-name", "Desktop");
    await page.fill("#f-mac", "D8:5E:D3:DB:F7:6B");
    await page.fill("#f-ip", "127.0.0.1");
    await page.click(".page[data-name=machine] .toggle");
    await shot("add");
    await page.click(".page[data-name=machine] .button-fill");
    await page.waitForSelector("text=Waiting for agent");
    await shot("pairing");
    await page.click(".page[data-name=pair] .navbar .link.back");
    await page.waitForSelector(".card.machine .name");
    await shot("list");
    await page.click(".card.machine .item-link");
    await page.waitForSelector(".page[data-name=machine] #f-name");
    if ((await page.inputValue("#f-name")) !== "Desktop") fail("edit page prefill");
    await shot("edit");
    await page.click(".page[data-name=machine] .navbar .link.back");
    await page.waitForTimeout(600);
    // 127.0.0.1 has no agent, so the machine shows offline: open the confirm dialog via the app's own function.
    await page.evaluate(() => confirmPower(machines[0], "shutdown"));
    await shot("confirm");
    await page.click(".dialog-button >> text=Cancel");
    await page.waitForTimeout(400);
    await page.click("#gear");
    await page.waitForSelector(".page[data-name=settings]");
    await page.fill('input[placeholder="Token name"]', "home-script");
    await page.click('.page[data-name=settings] a.link:text-is("Create")');
    await page.waitForSelector("text=won't be shown again");
    await shot("settings");
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
  await page.goto(base);
  await page.click("#gear");
  await page.waitForSelector(".page[data-name=settings]");
  await Promise.all([page.waitForEvent("load"), page.click('input[value="md"] + .icon-radio')]);
  if ((await page.evaluate(() => app.theme)) !== "md") fail("forced md theme on iPhone");
  await page.evaluate(() => localStorage.removeItem("zw-theme"));
  await page.reload();
  if ((await page.evaluate(() => app.theme)) !== "ios") fail("auto theme restored on iPhone");
  await ctx.close();
}
await browser.close();
console.log(errors ? `${errors} problem(s)` : "no problems");
process.exit(errors ? 1 : 0);
