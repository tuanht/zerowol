"use strict";
const VERSION = "__VERSION__";
const REPO = "https://github.com/tuanht/zerowol";
const LIMIT = { wake: 120e3, shutdown: 90e3, reboot: 180e3 };
const BUSY = { wake: "Waking…", shutdown: "Shutting down…", reboot: "Restarting…" };
const HINT = " Check journalctl -u zerowol-agent on the PC.";
const FAIL = { wake: "Didn't come online", shutdown: "Didn't shut down." + HINT, reboot: "Didn't restart." + HINT };
const $ = (s) => document.querySelector(s);
const T = new Map(); // machine id -> {kind, start, off}
let machines = [], loaded = false, timer = 0, sheetEls = null, onClose = null;

function h(tag, props, ...kids) {
  const e = document.createElement(tag);
  for (const k in props || {}) {
    const v = props[k];
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (v === true) e.setAttribute(k, "");
    else if (v !== false && v != null) e.setAttribute(k, v);
  }
  for (const c of kids.flat(Infinity)) if (c != null && c !== false) e.append(c);
  return e;
}
function ico(id) {
  const NS = "http://www.w3.org/2000/svg";
  const s = document.createElementNS(NS, "svg"), u = document.createElementNS(NS, "use");
  s.setAttribute("class", "ico");
  u.setAttribute("href", "#i-" + id);
  s.append(u);
  return s;
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function api(method, path, body) {
  const r = await fetch("/api/v1" + path, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  if (r.status === 204) return null;
  let j = null;
  try { j = await r.json(); } catch {}
  if (!r.ok) throw Object.assign(new Error((j && j.error) || "Request failed (" + r.status + ")"), { status: r.status });
  return j;
}
function toast(msg, ok) {
  const t = h("div", { class: "toast" + (ok ? "" : " bad") }, msg);
  $("#toasts").append(t);
  setTimeout(() => t.remove(), 4000);
}
const fail = (e) => toast(e.message === "Failed to fetch" || e instanceof TypeError ? "Can't reach zerowol" : e.message);
function uptime(s) {
  const d = Math.floor(s / 86400), hr = Math.floor(s / 3600) % 24, m = Math.floor(s / 60) % 60;
  return d ? d + "d " + hr + "h" : hr ? hr + "h " + m + "m" : m ? m + "m" : "<1m";
}

/* ---------- polling and transitions ---------- */
function reconcile() {
  const now = Date.now();
  for (const [id, t] of T) {
    const m = machines.find((x) => x.id === id);
    if (!m) { T.delete(id); continue; }
    const st = m.status || {}, on = !!st.online;
    if (t.kind === "reboot" && (!on || (t.up != null && st.uptime != null && st.uptime < t.up))) t.off = true;
    const done = t.kind === "wake" ? on : t.kind === "shutdown" ? !on : t.off && on;
    if (done) T.delete(id);
    else if (now - t.start > LIMIT[t.kind]) { T.delete(id); toast(m.name + ": " + FAIL[t.kind]); }
  }
}
async function refresh() {
  clearTimeout(timer);
  try {
    machines = await api("GET", "/machines");
    loaded = true;
    $("#offline").hidden = true;
    reconcile();
  } catch (e) {
    if (e instanceof TypeError) $("#offline").hidden = false;
    else fail(e);
  }
  render();
  schedule();
}
function schedule() {
  clearTimeout(timer);
  if (!document.hidden) timer = setTimeout(refresh, T.size ? 2000 : 5000);
}
document.addEventListener("visibilitychange", () => (document.hidden ? clearTimeout(timer) : refresh()));

/* ---------- list ---------- */
function card(m) {
  const t = T.get(m.id), st = m.status || {}, on = !!st.online;
  const label = t ? BUSY[t.kind] : on ? "Online" : "Offline";
  const meta = m.ip + " · " + (on ? (st.uptime != null ? "Up " + uptime(st.uptime) : "Online") : "Offline") + (st.agent_error ? " · Agent: " + st.agent_error : "");
  const btn = (cls, icon, text, fn) => h("button", { class: cls, disabled: !!t, onclick: fn }, ico(icon), text);
  let acts;
  if (on && st.agent === "paired")
    acts = [btn("", "redo", "Restart", () => confirmPower(m, "reboot")), btn("red", "power", "Shut Down", () => confirmPower(m, "shutdown"))];
  else if (m.agent && st.agent_error) acts = [btn("", "redo", "Agent: " + st.agent_error + " · Pair", () => pairingSheet(m))];
  else if (on && m.agent) acts = [btn("", "redo", "Agent unreachable · Pair", () => pairingSheet(m))];
  else acts = [btn("", "power", "Wake", () => wake(m))];
  return h("div", { class: "group" },
    h("button", { class: "item", onclick: () => machineSheet(m) },
      h("div", { class: "tile" }, ico("desk")),
      h("div", { class: "txt" }, h("div", { class: "name" }, m.name), h("div", { class: "meta" }, meta)),
      h("div", { class: "status-r" + (t ? " busy-t" : "") }, h("span", { class: "dot" + (t ? " busy" : on ? " on" : "") }), label)),
    h("div", { class: "actions" }, acts));
}
function render() {
  const list = $("#list");
  if (!loaded) return list.replaceChildren();
  $("#foot").hidden = !machines.length;
  list.replaceChildren(...(machines.length
    ? machines.map(card)
    : [h("div", { class: "group" }, h("button", { class: "item empty", onclick: () => machineSheet(null) }, h("div", { class: "txt" }, h("b", null, "Add your first machine"), "Tap to set up Wake-on-LAN")))]));
}
async function wake(m) {
  T.set(m.id, { kind: "wake", start: Date.now() });
  render(); schedule();
  try {
    const r = await api("POST", "/machines/" + m.id + "/wake");
    if (r.result === "already online") { T.delete(m.id); toast(m.name + " is already online", true); }
  } catch (e) { T.delete(m.id); fail(e); }
  refresh();
}
function confirmPower(m, kind) {
  const down = kind === "shutdown";
  confirmSheet((down ? "Shut Down " : "Restart ") + m.name + "?",
    down ? "Unsaved work on " + m.name + " will be lost." : m.name + " will restart. Unsaved work will be lost.",
    down ? "Shut Down" : "Restart", down, async () => {
      closeSheet();
      T.set(m.id, { kind, start: Date.now(), up: m.status && m.status.uptime });
      render(); schedule();
      try { await api("POST", "/machines/" + m.id + "/" + kind); } catch (e) { T.delete(m.id); fail(e); }
      refresh();
    });
}

/* ---------- sheets ---------- */
function openSheet(...kids) {
  closeSheet();
  const dim = h("div", { class: "dim", onclick: closeSheet });
  const s = h("div", { class: "sheet glass", role: "dialog", "aria-modal": "true" }, h("div", { class: "grab" }), kids);
  $("#sheet-root").append(dim, s);
  sheetEls = [dim, s];
  return s;
}
function closeSheet() {
  if (sheetEls) sheetEls.forEach((e) => e.remove());
  sheetEls = null;
  if (onClose) { const f = onClose; onClose = null; f(); }
}
document.addEventListener("keydown", (e) => e.key === "Escape" && closeSheet());
const big = (text, fn, cls) => h("button", { class: "big " + (cls || ""), type: "button", onclick: fn }, text);
const cancel = (fn) => big("Cancel", fn || closeSheet, "cancel");
function confirmSheet(title, text, label, red, fn, back) {
  openSheet(h("h3", null, title), h("p", null, text), big(label, fn, red ? "red" : ""), cancel(back));
}
async function copy(text, b) {
  try { await navigator.clipboard.writeText(text); } catch {
    const t = h("textarea", { readonly: true }, text);
    t.style.cssText = "position:fixed;opacity:0";
    document.body.append(t);
    t.select(); t.setSelectionRange(0, 1e6);
    try { document.execCommand("copy"); } catch {}
    t.remove();
  }
  const o = b.textContent;
  b.textContent = "Copied";
  setTimeout(() => (b.textContent = o), 1500);
}

function machineSheet(m) {
  const f = {}, rows = {};
  const inp = (k, label, ph, val, type, mode) => {
    f[k] = h("input", { id: "f-" + k, type: type || "text", placeholder: ph, value: val == null ? "" : val, inputmode: mode, autocapitalize: "off", autocomplete: "off", autocorrect: "off", spellcheck: "false" });
    return (rows[k] = h("div", { class: "row" }, h("label", { for: "f-" + k }, label), f[k]));
  };
  const ag = h("input", { type: "checkbox", class: "sw", id: "f-agent" });
  ag.checked = !!(m && m.agent);
  const err = h("div", { class: "errmsg" });
  const rIp = inp("ip", "IP", "192.168.1.20", m && m.ip, "text", "url");
  // iOS numeric pads show the locale decimal separator (often ","), so use the
  // url keyboard (always has ".") and normalise any typed "," to ".".
  const dots = (el) => el.addEventListener("input", () => {
    if (el.value.includes(",")) el.value = el.value.replace(/,/g, ".");
  });
  dots(f.ip);
  f.ip.addEventListener("input", () => {
    const o = f.ip.value.trim().split(".");
    f.broadcast.placeholder = o.length === 4 ? o[0] + "." + o[1] + "." + o[2] + ".255" : "Default from IP";
  });
  const save = big("Save", async () => {
    Object.values(rows).forEach((r) => r.classList.remove("err"));
    err.textContent = "";
    const body = { name: f.name.value.trim(), mac: f.mac.value.trim(), ip: f.ip.value.trim(), agent: ag.checked };
    if (f.broadcast.value.trim()) body.broadcast = f.broadcast.value.trim();
    if (f.wol_port.value) body.wol_port = +f.wol_port.value;
    if (f.probe_port.value) body.probe_port = +f.probe_port.value;
    save.disabled = true;
    try {
      const r = m ? await api("PATCH", "/machines/" + m.id, body) : await api("POST", "/machines", body);
      closeSheet();
      refresh();
      if (ag.checked && !(m && m.agent)) pairingSheet(r);
    } catch (e) {
      save.disabled = false;
      const k = (/^invalid (name|mac|ip|broadcast|wol_port|probe_port)/.exec(e.message) || [])[1];
      if (k && /port$/.test(k)) f[k].closest("details").open = true;
      if (k) rows[k].classList.add("err");
      err.textContent = e instanceof TypeError ? "Can't reach zerowol" : e.message.replace(/^./, (c) => c.toUpperCase());
    }
  });
  const grp = [
    h("div", { class: "group" },
      inp("name", "Name", "Desktop", m && m.name),
      inp("mac", "MAC", "AA:BB:CC:DD:EE:FF", m && m.mac),
      rIp,
      inp("broadcast", "Broadcast", "Default from IP", m && m.broadcast, "text", "url"),
      h("div", { class: "row" }, h("div", { class: "grow" }, h("label", { for: "f-agent" }, "Has zerowol agent"), h("div", { class: "sub" }, "Needed for Restart and Shut Down")), ag)),
    h("div", { class: "group" }, h("details", null, h("summary", null, "Advanced"),
      inp("wol_port", "WoL port", "9", m && m.wol_port, "number", "numeric"),
      inp("probe_port", "Probe port", "22", m && m.probe_port, "number", "numeric"))),
  ];
  dots(f.broadcast);
  if (m) {
    const extra = [];
    if (m.agent) extra.push(h("button", { class: "row link", type: "button", onclick: () => pairingSheet(m) }, "Agent install command"));
    extra.push(h("button", { class: "row link red", type: "button", onclick: () => confirmSheet("Delete " + m.name + "?", "This removes the machine from zerowol. The PC itself is not touched.", "Delete", true, async () => {
      try { await api("DELETE", "/machines/" + m.id); T.delete(m.id); closeSheet(); refresh(); } catch (e) { fail(e); }
    }, () => machineSheet(m)) }, "Delete Machine"));
    grp.push(h("div", { class: "group" }, extra));
  }
  f.ip.dispatchEvent(new Event("input"));
  openSheet(h("h3", null, m ? "Edit Machine" : "Add Machine"), grp, err, save, cancel());
}

async function pairingSheet(m, rotate) {
  let alive = true;
  const cmd = h("div", { class: "code" }, "…");
  const wait = h("span", null, "Waiting for agent…");
  const st = h("div", { class: "pair" }, h("span", { class: "dot busy" }), wait);
  const cp = big("Copy", () => copy(cmd.textContent, cp));
  openSheet(h("h3", null, "Pair " + m.name), h("p", null, "Run this command on " + m.name + " (Linux)."), cmd, cp, st,
    big("Regenerate secret", () => confirmSheet("Regenerate secret?", "The installed agent will stop working until you run the new command.", "Regenerate", true, () => pairingSheet(m, true), () => pairingSheet(m)), "cancel"),
    big("Done", closeSheet, "cancel"));
  onClose = () => { alive = false; };
  try {
    cmd.textContent = (await api(rotate ? "POST" : "GET", "/machines/" + m.id + "/agent/" + (rotate ? "rotate" : "install-command"))).install_command;
  } catch (e) { fail(e); }
  while (alive) {
    await sleep(2000);
    try {
      const s = await api("GET", "/machines/" + m.id + "/status");
      if (alive && s.agent !== "paired") wait.textContent = s.agent_error ? "Agent: " + s.agent_error : "Waiting for agent…";
      if (alive && s.agent === "paired") {
        st.className = "pair ok";
        st.replaceChildren("Paired ✓");
        refresh();
        return;
      }
    } catch {}
  }
}

async function settingsSheet(shown) {
  const box = h("div");
  const nameIn = h("input", { type: "text", style: "text-align:left", placeholder: "Token name", autocapitalize: "off", autocomplete: "off", maxlength: 64 });
  const date = (s) => new Date(s * 1000).toLocaleDateString();
  async function load(newTok) {
    let toks = [];
    try { toks = await api("GET", "/tokens"); } catch (e) { fail(e); }
    box.replaceChildren(...[
      newTok && h("div", { class: "group", style: "padding:12px 16px" },
        h("div", { class: "small", style: "margin:0" }, "Copy " + newTok.name + " now. It won't be shown again."),
        h("div", { class: "code" }, newTok.token),
        big("Copy", function () { copy(newTok.token, this); })),
      h("div", { class: "group" }, toks.map((t) => h("div", { class: "row" },
        h("div", { class: "grow" }, h("div", { style: "overflow:hidden;text-overflow:ellipsis" }, t.name), h("div", { class: "sub" }, "Created " + date(t.created) + " · " + (t.last_used ? "Used " + date(t.last_used) : "Never used"))),
        h("button", { class: "row link red", style: "width:auto;padding:0", type: "button", onclick: () => confirmSheet("Revoke " + t.name + "?", "Anything using this token will stop working.", "Revoke", true, async () => {
          try { await api("DELETE", "/tokens/" + t.id); } catch (e) { fail(e); }
          settingsSheet();
        }, () => settingsSheet()) }, "Revoke"))),
        h("div", { class: "row" }, nameIn, h("button", { class: "row link", style: "width:auto;padding:0;justify-content:flex-end", type: "button", onclick: async () => {
          try { const r = await api("POST", "/tokens", { name: nameIn.value.trim() }); nameIn.value = ""; load(r); } catch (e) { fail(e); }
        } }, "Create")))].filter(Boolean));
  }
  openSheet(h("h3", null, "Settings"),
    h("div", { class: "h" }, "API Tokens"), box,
    h("p", { class: "small" }, "Tokens let scripts call the API with Authorization: Bearer."),
    h("div", { class: "h" }, "About"),
    h("div", { class: "group" },
      h("div", { class: "row" }, h("div", { class: "grow" }, "Version"), h("span", { class: "sub" }, VERSION)),
      h("a", { class: "row link", href: REPO, target: "_blank", rel: "noopener", style: "text-decoration:none" }, "zerowol on GitHub")),
    big("Done", closeSheet, "cancel"));
  await load(shown);
}

$("#add").addEventListener("click", () => machineSheet(null));
$("#gear").addEventListener("click", () => settingsSheet());
if ("serviceWorker" in navigator) navigator.serviceWorker.register("/sw.js").catch(() => {});
refresh();
