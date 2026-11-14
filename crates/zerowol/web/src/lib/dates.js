// Calendar dates as "YYYY-MM-DD" strings, for display only. Arithmetic runs on UTC
// midnights, so a DST change in the browser's zone never shifts a date.
const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

const toUTC = (d) => {
  const [y, m, dd] = d.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, dd));
};
const fromUTC = (t) => t.toISOString().slice(0, 10);
const pad = (n) => String(n).padStart(2, "0");
// 0 = Monday … 6 = Sunday
const dow = (d) => (toUTC(d).getUTCDay() + 6) % 7;

// Today in the browser's zone.
export function todayStr() {
  const n = new Date();
  return n.getFullYear() + "-" + pad(n.getMonth() + 1) + "-" + pad(n.getDate());
}
export function addDays(d, n) {
  const t = toUTC(d);
  t.setUTCDate(t.getUTCDate() + n);
  return fromUTC(t);
}
// The same day n months on, clamped to that month's last day (31 Jan + 1 → 28/29 Feb).
export function addMonths(d, n) {
  const [y, m, dd] = d.split("-").map(Number);
  const last = new Date(Date.UTC(y, m - 1 + n + 1, 0)).getUTCDate();
  return fromUTC(new Date(Date.UTC(y, m - 1 + n, Math.min(dd, last))));
}
export const weekStart = (d) => addDays(d, -dow(d));
export const weekDays = (d) => Array.from({ length: 7 }, (_, i) => addDays(weekStart(d), i));
// Monday-start, 6 rows, from the week holding the 1st of d's month.
export const monthGrid = (d) => Array.from({ length: 42 }, (_, i) => addDays(weekStart(d.slice(0, 8) + "01"), i));
export const fmtDay = (d) => DAY_NAMES[dow(d)].slice(0, 3) + " " + toUTC(d).getUTCDate();
export const fmtLong = (d) => {
  const t = toUTC(d);
  return DAY_NAMES[dow(d)] + ", " + t.getUTCDate() + " " + MONTH_NAMES[t.getUTCMonth()];
};
// "November 2026", or "Nov – Dec 2026" / "Dec 2026 – Jan 2027" for a week across months.
export function fmtRange(a, b) {
  const [ta, tb] = [toUTC(a), toUTC(b)];
  const [ma, mb, ya, yb] = [ta.getUTCMonth(), tb.getUTCMonth(), ta.getUTCFullYear(), tb.getUTCFullYear()];
  const s = (m) => MONTH_NAMES[m].slice(0, 3);
  if (ma === mb) return MONTH_NAMES[ma] + " " + ya;
  return ya === yb ? s(ma) + " – " + s(mb) + " " + yb : s(ma) + " " + ya + " – " + s(mb) + " " + yb;
}
