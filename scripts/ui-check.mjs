// Usage: NODE_PATH=$(npm root -g) node scripts/ui-check.mjs http://127.0.0.1:8090
// Writes 8 screenshots (light/dark x list, pairing, confirm, settings) to ./screenshots/.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const require = createRequire(import.meta.url);
let pw;
try { pw = require("playwright"); } catch {
  const root = (await import("node:child_process")).execSync("npm root -g").toString().trim();
  pw = createRequire(root + "/")("playwright");
}
const { chromium, devices } = pw;
const base = process.argv[2] || "http://127.0.0.1:8090";
mkdirSync("screenshots", { recursive: true });
const browser = await chromium.launch();
let errors = 0;
for (const scheme of ["light", "dark"]) {
  const ctx = await browser.newContext({ ...devices["iPhone 15"], colorScheme: scheme });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => { errors++; console.error("pageerror:", e.message); });
  page.on("console", (m) => { if (m.type() === "error") { errors++; console.error("console error:", m.text()); } });
  await page.goto(base);
  await page.evaluate(async () => {
    for (const m of await (await fetch("/api/v1/machines")).json()) await fetch("/api/v1/machines/" + m.id, { method: "DELETE" });
  });
  await page.reload();
  const shot = async (name) => {
    await page.waitForTimeout(500);
    await page.screenshot({ path: `screenshots/${scheme}-${name}.png` });
    console.log("wrote", `screenshots/${scheme}-${name}.png`);
  };
  await page.click("#add");
  await page.fill("#f-name", "Desktop");
  await page.fill("#f-mac", "D8:5E:D3:DB:F7:6B");
  await page.fill("#f-ip", "127.0.0.1");
  await page.click('label[for="f-agent"]');
  await page.click("text=Save");
  await page.waitForSelector("text=Waiting for agent");
  await shot("pairing");
  await page.click("text=Done");
  await page.waitForSelector(".group .name");
  await shot("list");
  // 127.0.0.1 has no agent, so the machine shows offline: open the confirm sheet via the app's own function.
  await page.evaluate(() => confirmPower(machines[0], "shutdown"));
  await shot("confirm");
  await page.click("text=Cancel");
  await page.click("#gear");
  await page.fill('input[placeholder="Token name"]', "home-script");
  await page.click('button:text-is("Create")');
  await page.waitForSelector("text=won't be shown again");
  await shot("settings");
  await ctx.close();
}
await browser.close();
console.log(errors ? `${errors} page error(s)` : "no page errors");
process.exit(errors ? 1 : 0);
