// The timezone list behind the timezone pickers (Settings → Timezone and the schedule form):
// one entry per IANA zone, and a forgiving search over city, zone id, region, generic name and offset.

// Chromium lists (and resolves) a few zones under their old CLDR names; show the current IANA name,
// which is the one people search for ("Ho Chi Minh", "Kyiv"). The server's tzdb knows both.
const MODERN = {
  "Africa/Asmera": "Africa/Asmara",
  "America/Buenos_Aires": "America/Argentina/Buenos_Aires",
  "America/Catamarca": "America/Argentina/Catamarca",
  "America/Coral_Harbour": "America/Atikokan",
  "America/Cordoba": "America/Argentina/Cordoba",
  "America/Godthab": "America/Nuuk",
  "America/Indianapolis": "America/Indiana/Indianapolis",
  "America/Jujuy": "America/Argentina/Jujuy",
  "America/Louisville": "America/Kentucky/Louisville",
  "America/Mendoza": "America/Argentina/Mendoza",
  "Asia/Calcutta": "Asia/Kolkata",
  "Asia/Katmandu": "Asia/Kathmandu",
  "Asia/Rangoon": "Asia/Yangon",
  "Asia/Saigon": "Asia/Ho_Chi_Minh",
  "Atlantic/Faeroe": "Atlantic/Faroe",
  "Europe/Kiev": "Europe/Kyiv",
  "Pacific/Enderbury": "Pacific/Kanton",
  "Pacific/Ponape": "Pacific/Pohnpei",
  "Pacific/Truk": "Pacific/Chuuk",
};
export const modernZone = (id) => MODERN[id] || id;

// "Asia/Ho_Chi_Minh" → city "Ho Chi Minh", region "Asia"; a zone without a "/" (UTC) is in "Other".
function describe(id) {
  const parts = id.split("/");
  let generic = "";
  try {
    const p = new Intl.DateTimeFormat("en-US", { timeZone: id, timeZoneName: "longGeneric" }).formatToParts(0);
    generic = (p.find((x) => x.type === "timeZoneName") || {}).value || "";
  } catch {}
  return {
    id,
    city: parts[parts.length - 1].replace(/_/g, " "),
    region: parts.length > 1 ? parts[0] : "Other",
    // Without a name ICU falls back to "GMT+07:00", which the offset already shows.
    generic: /^GMT|^UTC/.test(generic) ? "" : generic,
  };
}

let base = null;
function baseList() {
  if (!base) {
    let all = [];
    try {
      all = Intl.supportedValuesOf("timeZone");
    } catch {}
    const ids = new Set(all.map(modernZone));
    ids.add("UTC");
    base = [...ids].map(describe);
  }
  return base;
}

// The offset ("GMT+7", "GMT-3:30", "GMT") and the wall clock ("14:05") in a zone right now.
export function zoneNow(id, at = new Date()) {
  try {
    const p = new Intl.DateTimeFormat("en-US", {
      timeZone: id, hour: "2-digit", minute: "2-digit", hourCycle: "h23", timeZoneName: "shortOffset",
    }).formatToParts(at);
    const g = (t) => (p.find((x) => x.type === t) || {}).value || "";
    return { offset: g("timeZoneName").replace(/^GMT\+0$/, "GMT"), time: g("hour") + ":" + g("minute") };
  } catch {
    return { offset: "", time: "" };
  }
}

// One zone as the list shows it (city, region, generic name, offset, time).
export const zoneInfo = (id) => ({ ...describe(modernZone(id)), ...zoneNow(id) });

// Every zone, sorted by region then city, with its current offset and time; `extra` (the stored zone)
// is added when the browser doesn't know it.
export function zoneEntries(extra) {
  let list = baseList();
  const x = extra && modernZone(extra);
  if (x && !list.some((e) => e.id === x)) list = [...list, describe(x)];
  const at = new Date();
  return list
    .map((e) => ({ ...e, ...zoneNow(e.id, at) }))
    .sort((a, b) => a.region.localeCompare(b.region) || a.city.localeCompare(b.city) || a.id.localeCompare(b.id));
}

// Lower case, no accents, letters and digits only: "Ho_Chi Minh" and "ho chi-minh" both become "hochiminh".
const norm = (s) => s.normalize("NFD").toLowerCase().replace(/[^a-z0-9]/g, "");
// Every tail of the text that starts at a word: "Pacific Time" → ["pacifictime", "time"].
const tails = (s) => {
  const w = s.normalize("NFD").toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
  return w.map((_, i) => w.slice(i).join(""));
};
// "+7", "-5", "+5:30", "gmt+7", "UTC-3:30", "gmt" (and entries' "GMT+7") → minutes east of UTC; null when not an offset.
function queryOffset(q) {
  const m = /^(?:gmt|utc)?\s*(?:([+-])\s*(\d{1,2})(?::?(\d{2}))?)?$/i.exec(q.trim());
  if (!m || (!m[1] && !/^(gmt|utc)$/i.test(q.trim()))) return null;
  if (!m[1]) return 0;
  const v = +m[2] * 60 + +(m[3] || 0);
  return m[1] === "-" ? -v : v;
}

// Entries matching `query`, best first: city prefix, then a word prefix in the city or generic name,
// then a match anywhere (zone id, region, generic name); ties by city. An offset query ("+7") lists
// the zones at that offset. An empty query returns `entries` unchanged.
export function searchZones(entries, query) {
  const raw = (query || "").trim();
  if (!raw) return entries;
  const byCity = (a, b) => a.e.city.localeCompare(b.e.city) || a.e.id.localeCompare(b.e.id);
  const off = queryOffset(raw);
  // A signed offset ("+7") is not also searched as text ("7").
  const q = off !== null && /[+-]/.test(raw) ? "" : norm(raw);
  const hits = [];
  for (const e of entries) {
    let rank = -1;
    if (q) {
      const city = norm(e.city);
      if (city.startsWith(q)) rank = 0;
      else if ([...tails(e.city), ...tails(e.generic)].some((t) => t.startsWith(q))) rank = 1;
      else if ([city, norm(e.id), norm(e.region), norm(e.generic)].some((t) => t.includes(q))) rank = 2;
    }
    if (off !== null && queryOffset(e.offset) === off) rank = rank < 0 ? 3 : Math.min(rank, 3);
    if (rank >= 0) hits.push({ e, rank });
  }
  return hits.sort((a, b) => a.rank - b.rank || byCity(a, b)).map((h) => h.e);
}
