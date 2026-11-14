#!/usr/bin/env node
// Gzip budget for the built web UI (crates/zerowol/web/dist), see the plan's
// size constraint: gzip -9 of app.js + app.css must stay within BUDGET bytes.
import { readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

// 184,815 (pre-Vue size + 60 KB) raised to 196,143 for the searchable timezone
// list in Phase 2 (Framework7 Searchbar + zone data); accepted by the owner 2026-11-14.
const BUDGET = 196143;
const dist = new URL("../crates/zerowol/web/dist/", import.meta.url);
let total = 0;
for (const f of ["app.js", "app.css"]) {
  const n = gzipSync(readFileSync(new URL(f, dist)), { level: 9 }).length;
  total += n;
  console.log(`${f.padEnd(8)} ${String(n).padStart(7)} bytes gzip -9`);
}
console.log(`${"total".padEnd(8)} ${String(total).padStart(7)} bytes (budget ${BUDGET})`);
if (total > BUDGET) {
  console.error(`over budget by ${total - BUDGET} bytes: ${fileURLToPath(dist)}`);
  process.exit(1);
}
