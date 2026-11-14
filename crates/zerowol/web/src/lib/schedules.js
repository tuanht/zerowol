import { reactive } from "vue";
import { api } from "./api.js";
import { DAYS, DAY_LABEL, MONTHS, REPEAT } from "./words.js";
import { modernZone, zoneNow } from "./zones.js";

// ---------- API ----------
export const listSchedules = () => api("GET", "/schedules");
export const createSchedule = (body) => api("POST", "/schedules", body);
export const updateSchedule = (id, patch) => api("PATCH", "/schedules/" + id, patch);
export const deleteSchedule = (id) => api("DELETE", "/schedules/" + id);
export const skip = (id, date) => api("POST", "/schedules/" + id + "/skip", { date });
export const unskip = (id, date) => api("DELETE", "/schedules/" + id + "/skip/" + date);
export const getSettings = () => api("GET", "/settings");
export const fetchOccurrences = (from, to) =>
  api("GET", "/schedules/occurrences?from=" + encodeURIComponent(from) + "&to=" + encodeURIComponent(to));

// `rev` goes up whenever rules change (form save/delete, skip/unskip), so views showing occurrences reload.
export const schedState = reactive({ list: [], settings: null, rev: 0 });

export async function reloadSchedules() {
  const [list, settings] = await Promise.all([listSchedules(), getSettings()]);
  schedState.list = list;
  schedState.settings = settings;
}

export async function setTimezone(name) {
  await api("PATCH", "/settings", { timezone: name });
  schedState.settings = await getSettings();
}

// ---------- pure helpers ----------
const mins = (t) => {
  const [h, m] = t.split(":");
  return +h * 60 + +m;
};
const ymd = (d) => d.split("-").map(Number);
const dayNum = (d) => {
  const [y, m, dd] = ymd(d);
  return Date.UTC(y, m - 1, dd) / 864e5;
};
// 0 = Monday … 6 = Sunday (1970-01-01, day 0, was a Thursday).
const weekday = (d) => (dayNum(d) + 3) % 7;

const KIND_DAYS = { daily: [0, 1, 2, 3, 4, 5, 6], weekdays: [0, 1, 2, 3, 4], weekends: [5, 6] };
function daysOf(repeat) {
  if (repeat.kind === "once") return [weekday(repeat.date)];
  if (repeat.kind === "weekly") return (repeat.days || []).map((d) => DAYS.indexOf(d)).filter((i) => i >= 0);
  return KIND_DAYS[repeat.kind] || [];
}

const WEEK = 7 * 1440;
function near(a, b, limit) {
  if (a.repeat.kind === "once" && b.repeat.kind === "once")
    return Math.abs(dayNum(a.repeat.date) * 1440 + mins(a.time) - dayNum(b.repeat.date) * 1440 - mins(b.time)) <= limit;
  // Minute-of-week positions; the difference wraps around the week (and so around midnight).
  const pa = daysOf(a.repeat).map((d) => d * 1440 + mins(a.time));
  const pb = daysOf(b.repeat).map((d) => d * 1440 + mins(b.time));
  return pa.some((x) => pb.some((y) => {
    const d = Math.abs(x - y) % WEEK;
    return Math.min(d, WEEK - d) <= limit;
  }));
}

// Other enabled rules on the same machine that act within ±`minutes` of the draft on an overlapping day.
export function clashes(list, draft, minutes = 2) {
  return list.filter(
    (s) => s.enabled && s.id !== draft.id && s.machine_id === draft.machine_id && near(s, draft, minutes),
  );
}

export function repeatLabel(repeat) {
  const k = REPEAT[repeat.kind] || repeat.kind;
  if (repeat.kind === "weekly") return k + " · " + (repeat.days || []).map((d) => DAY_LABEL[d]).join(", ");
  if (repeat.kind === "once") {
    const [, m, d] = ymd(repeat.date);
    return k + " · " + d + " " + MONTHS[m - 1];
  }
  return k;
}

// ---------- timezones ----------
export const browserZone = () => {
  try {
    return modernZone(Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC");
  } catch {
    return "UTC";
  }
};
// Now in the given zone (browser zone when unset or unknown): { date: "YYYY-MM-DD", min: minutes since midnight }.
export function clockIn(zone, at = new Date()) {
  const opts = { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23" };
  const fmt = (z) => new Intl.DateTimeFormat("en-US", { ...opts, timeZone: z }).formatToParts(at);
  let p;
  try {
    p = fmt(zone || undefined);
  } catch {
    p = fmt(undefined);
  }
  const g = (t) => p.find((x) => x.type === t).value;
  return { date: g("year") + "-" + g("month") + "-" + g("day"), min: +g("hour") * 60 + +g("minute") };
}
// Today's date ("YYYY-MM-DD") in the given zone (browser zone when unset or unknown).
export const todayIn = (zone) => clockIn(zone).date;
export const weekdayOf = (date) => DAYS[weekday(date)];

// The browser's zone, offered first by the timezone prompts (form step and Schedules page).
export function zoneChoice() {
  const zone = browserZone();
  return { zone, offset: zoneNow(zone).offset };
}

// ---------- the shared form popup and machine sheet ----------
// One <ScheduleForm> and one <MachineSchedules> are mounted in App.vue and driven by this state.
export const formState = reactive({ open: false, seq: 0, machineId: null, schedule: null, fixed: false, prefill: null });
let resolveForm = null;

// Opens the form in add mode (optionally for a machine, optionally prefilled, e.g. { kind: "once", date, time })
// or edit mode; resolves true when saved or deleted.
export function openScheduleForm({ machineId, schedule, prefill } = {}) {
  if (resolveForm) resolveForm(false);
  Object.assign(formState, {
    open: true,
    seq: formState.seq + 1,
    machineId: machineId || (schedule && schedule.machine_id) || null,
    schedule: schedule || null,
    prefill: (!schedule && prefill) || null,
    // From a card the machine is given; an existing rule can't move to another machine (PATCH keeps machine_id).
    fixed: !!(machineId || schedule),
  });
  return new Promise((r) => (resolveForm = r));
}
// `seq` is the open this popup was showing: a late "closed" event from an earlier open is ignored.
export function closeScheduleForm(seq, result) {
  if (seq !== formState.seq) return;
  formState.open = false;
  if (result) schedState.rev++;
  const r = resolveForm;
  resolveForm = null;
  if (r) r(result);
}

export const sheetState = reactive({ open: false, machineId: null });
export function openMachineSchedules(machine) {
  sheetState.machineId = machine.id;
  sheetState.open = true;
}
