import { reactive } from "vue";
import { api, offlineMsg } from "./api.js";
import { toast } from "./ui.js";
import { FAIL, LIMIT } from "./words.js";

export const state = reactive({ machines: [], loaded: false, offline: false });
export const inflight = reactive(new Map());

// Settles finished or expired transitions in place; returns the expired ones.
export function reconcile(inf, machines, now) {
  const failed = [];
  for (const [id, t] of [...inf]) {
    const m = machines.find((x) => x.id === id);
    if (!m) {
      inf.delete(id);
      continue;
    }
    const st = m.status || {};
    const on = !!st.online;
    if (t.kind === "reboot" && (!on || (t.up != null && st.uptime != null && st.uptime < t.up))) t.off = true;
    const done = t.kind === "wake" ? on : t.kind === "poweroff" ? !on : t.off && on;
    if (done) inf.delete(id);
    else if (now - t.start > LIMIT[t.kind]) {
      inf.delete(id);
      failed.push({ id, kind: t.kind });
    }
  }
  return { failed };
}

let timer = 0;
let polling = false;

export async function refresh() {
  clearTimeout(timer);
  try {
    state.machines = await api("GET", "/machines");
    state.loaded = true;
    state.offline = false;
    const { failed } = reconcile(inflight, state.machines, Date.now());
    for (const f of failed) {
      const m = state.machines.find((x) => x.id === f.id);
      toast((m ? m.name + ": " : "") + FAIL[f.kind]);
    }
  } catch (e) {
    if (e instanceof TypeError) state.offline = true;
    else toast(offlineMsg(e));
  }
  schedule();
}

function schedule() {
  clearTimeout(timer);
  if (polling && !document.hidden) timer = setTimeout(refresh, inflight.size ? 2000 : 5000);
}

export function startPolling() {
  if (polling) return;
  polling = true;
  document.addEventListener("visibilitychange", () => (document.hidden ? clearTimeout(timer) : refresh()));
  refresh();
}

export async function wake(m) {
  inflight.set(m.id, { kind: "wake", start: Date.now() });
  schedule();
  try {
    const r = await api("POST", "/machines/" + m.id + "/wake");
    if (r && r.result === "already online") {
      inflight.delete(m.id);
      toast(m.name + " is already online", true);
    }
  } catch (e) {
    inflight.delete(m.id);
    toast(offlineMsg(e));
  }
  refresh();
}

export async function power(m, kind) {
  inflight.set(m.id, { kind, start: Date.now(), up: m.status && m.status.uptime });
  schedule();
  try {
    await api("POST", "/machines/" + m.id + "/" + kind);
  } catch (e) {
    inflight.delete(m.id);
    toast(offlineMsg(e));
  }
  refresh();
}
