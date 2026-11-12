"use strict";
const VERSION = "__VERSION__";
const REPO = "https://github.com/tuanht/zerowol";
const LIMIT = { wake: 120e3, shutdown: 90e3, reboot: 180e3 };
const BUSY = { wake: "Waking…", shutdown: "Shutting down…", reboot: "Restarting…" };
const HINT = " Check journalctl -u zerowol-agent on the PC.";
const FAIL = { wake: "Didn't come online", shutdown: "Didn't shut down." + HINT, reboot: "Didn't restart." + HINT };
const THEME_KEY = "zw-theme"; // "auto" | "ios" | "md"
const SCHEME_KEY = "zw-scheme"; // "auto" | "light" | "dark"
const $ = (s, root) => (root || document).querySelector(s);
const T = new Map(); // machine id -> {kind, start, off}
let machines = [], loaded = false, timer = 0;

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

/* ---------- theme + app ---------- */
function themePref() {
  try { const v = localStorage.getItem(THEME_KEY); if (v === "ios" || v === "md") return v; } catch {}
  return "auto";
}
function schemePref() {
  try { const v = localStorage.getItem(SCHEME_KEY); if (v === "light" || v === "dark") return v; } catch {}
  return "auto";
}
function store(key, v) {
  try { if (v === "auto") localStorage.removeItem(key); else localStorage.setItem(key, v); } catch {}
}
const darkParam = () => ({ light: false, dark: true })[schemePref()] ?? "auto";
function applyScheme() {
  app.setDarkMode(darkParam());
  // Keep the browser chrome (status bar / address bar) in step with the forced scheme.
  const sp = schemePref();
  document.querySelectorAll('meta[name="theme-color"]').forEach((m) => {
    if (!m.dataset.media) m.dataset.media = m.getAttribute("media") || "";
    if (sp === "auto") m.setAttribute("media", m.dataset.media);
    else m.setAttribute("media", m.dataset.media.includes(sp) ? "all" : "not all");
  });
}
const app = new Framework7({
  el: "#app",
  name: "zerowol",
  theme: themePref(),
  darkMode: darkParam(),
  colors: { primary: "#007aff" },
  touch: { tapHold: false },
  routes: [
    { path: "/machine/new/", async: ({ resolve }) => resolve({ el: machinePage(null) }) },
    { path: "/machine/:id/pair/", async: ({ to, resolve, reject }) => routeMachine(to, resolve, reject, pairingPage) },
    { path: "/machine/:id/", async: ({ to, resolve, reject }) => routeMachine(to, resolve, reject, machinePage) },
    { path: "/settings/", async: ({ resolve }) => resolve({ el: settingsPage() }) },
  ],
});
const IOS = app.theme === "ios";
document.documentElement.classList.add(IOS ? "zw-ios" : "zw-md");
if (navigator.standalone || matchMedia("(display-mode: standalone)").matches) document.documentElement.classList.add("zw-standalone");
applyScheme();
// Hash-based history gives Android a working system back button. iOS keeps it
// off: Safari's own edge swipe would fight Framework7's swipe-back.
if (location.hash) history.replaceState(null, "", location.pathname + location.search);
const view = app.views.create(".view-main", {
  url: "/",
  browserHistory: !IOS,
  browserHistorySeparator: "#!",
  browserHistoryAnimateOnLoad: false,
  iosSwipeBack: true,
});

function routeMachine(to, resolve, reject, build) {
  const m = machines.find((x) => x.id === to.params.id);
  if (m) return resolve({ el: build(m) });
  api("GET", "/machines").then((list) => {
    machines = list;
    const found = list.find((x) => x.id === to.params.id);
    if (found) resolve({ el: build(found) });
    else { toast("Machine not found"); reject(); }
  }).catch((e) => { fail(e); reject(); });
}

/* ---------- helpers ---------- */
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
// Framework7 builds dialogs and toasts from HTML strings without escaping, so
// every user-controlled string (machine/token names, server errors) goes through esc().
const esc = (s) => { const d = document.createElement("div"); d.textContent = s == null ? "" : String(s); return d.innerHTML; };
function toast(msg, ok) {
  const t = app.toast.create({ text: esc(msg), position: "top", closeTimeout: 4000, destroyOnClose: true, cssClass: ok ? "" : "toast-bad" });
  t.el.setAttribute("role", "status");
  t.el.setAttribute("aria-live", "polite");
  t.open();
}
const offlineMsg = (e) => (e instanceof TypeError ? "Can't reach zerowol" : e.message);
const fail = (e) => toast(offlineMsg(e));
function uptime(s) {
  const d = Math.floor(s / 86400), hr = Math.floor(s / 3600) % 24, m = Math.floor(s / 60) % 60;
  return d ? d + "d " + hr + "h" : hr ? hr + "h " + m + "m" : m ? m + "m" : "<1m";
}
function confirmDialog(title, text, label, red, fn) {
  app.dialog.create({
    title: esc(title), text: esc(text),
    buttons: [{ text: "Cancel" }, { text: label, strong: true, color: red ? "red" : null, onClick: fn }],
  }).open();
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

/* ---------- page scaffolding ---------- */
function page(name, title, right, ...content) {
  const back = h("a", { class: "link back", href: "#", "aria-label": "Back" }, h("i", { class: "icon icon-back" }), IOS ? h("span", null, "Back") : null);
  return h("div", { class: "page", "data-name": name },
    h("div", { class: "navbar" },
      h("div", { class: "navbar-bg" }),
      h("div", { class: "navbar-inner sliding" },
        h("div", { class: "left" }, back),
        h("div", { class: "title" }, title),
        right ? h("div", { class: "right" }, right) : null)),
    h("div", { class: "page-content" }, content));
}
const list = (cls, ...items) => h("div", { class: "list " + (cls || "") }, h("ul", null, items));
const LIST = "list-strong inset list-dividers-ios";
const blockTitle = (t) => h("div", { class: "block-title" }, t);
const footer = (t) => h("div", { class: "block-footer" }, t);
const bigBtn = (text, fn, cls) => h("button", { class: "button button-large button-fill " + (cls || ""), type: "button", onclick: fn }, text);
const linkItem = (text, fn, cls) => h("li", null, h("a", { class: "list-button " + (cls || ""), href: "#", onclick: (e) => { e.preventDefault(); fn(); } }, text));
function onRemove(pageEl, fn) {
  pageEl.addEventListener("page:beforeremove", fn);
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

/* ---------- home list ---------- */
const go = (url) => view.router.navigate(url);
function card(m) {
  const t = T.get(m.id), st = m.status || {}, on = !!st.online;
  const label = t ? BUSY[t.kind] : on ? "Online" : "Offline";
  const meta = m.ip + " · " + (on ? (st.uptime != null ? "Up " + uptime(st.uptime) : "Online") : "Offline") + (st.agent_error ? " · Agent: " + st.agent_error : "");
  const btn = (cls, icon, text, fn) => h("button", { class: "button " + cls, type: "button", disabled: !!t, onclick: fn }, ico(icon), h("span", null, text));
  let acts;
  if (on && st.agent === "paired")
    acts = [btn("", "redo", "Restart", () => confirmPower(m, "reboot")), btn("color-red", "power", "Shut Down", () => confirmPower(m, "shutdown"))];
  else if (m.agent && st.agent_error) acts = [btn("", "redo", "Agent: " + st.agent_error + " · Pair", () => go("/machine/" + m.id + "/pair/"))];
  else if (on && m.agent) acts = [btn("", "redo", "Agent unreachable · Pair", () => go("/machine/" + m.id + "/pair/"))];
  else acts = [btn("", "power", "Wake", () => wake(m))];
  return h("div", { class: "card machine" + (IOS ? "" : " card-outline") },
    h("div", { class: "list media-list no-margin no-hairlines" }, h("ul", null, h("li", null,
      h("a", { class: "item-link item-content", href: "/machine/" + m.id + "/" },
        h("div", { class: "item-media" }, h("div", { class: "tile" }, ico("desk"))),
        h("div", { class: "item-inner" },
          h("div", { class: "item-title-row" },
            h("div", { class: "item-title name" }, m.name),
            h("div", { class: "item-after status-r" + (t ? " busy-t" : "") }, h("span", { class: "dot" + (t ? " busy" : on ? " on" : "") }), label)),
          h("div", { class: "item-subtitle meta" }, meta)))))),
    h("div", { class: "card-footer actions" }, acts));
}
function render() {
  const el = $("#list");
  if (!loaded) return el.replaceChildren();
  $("#foot").hidden = !machines.length;
  el.replaceChildren(...(machines.length
    ? machines.map(card)
    : [list(LIST, h("li", null, h("a", { class: "item-link item-content empty", href: "/machine/new/" },
        h("div", { class: "item-inner" }, h("div", null, h("b", null, "Add your first machine"), h("div", { class: "sub" }, "Tap to set up Wake-on-LAN"))))))]));
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
  confirmDialog((down ? "Shut Down " : "Restart ") + m.name + "?",
    down ? "Unsaved work on " + m.name + " will be lost." : m.name + " will restart. Unsaved work will be lost.",
    down ? "Shut Down" : "Restart", down, async () => {
      T.set(m.id, { kind, start: Date.now(), up: m.status && m.status.uptime });
      render(); schedule();
      try { await api("POST", "/machines/" + m.id + "/" + kind); } catch (e) { T.delete(m.id); fail(e); }
      refresh();
    });
}

/* ---------- machine page ---------- */
function machinePage(m) {
  const f = {}, rows = {};
  const inp = (k, label, ph, val, type, mode) => {
    f[k] = h("input", { id: "f-" + k, name: k, type: type || "text", placeholder: ph, value: val == null ? "" : val, inputmode: mode, autocapitalize: "off", autocomplete: "off", autocorrect: "off", spellcheck: "false" });
    return (rows[k] = h("li", { class: "item-content item-input" + (IOS ? "" : " item-input-outline") },
      h("div", { class: "item-inner" },
        h("div", { class: "item-title item-label" }, h("label", { for: "f-" + k }, label)),
        h("div", { class: "item-input-wrap" }, f[k]))));
  };
  const ag = h("input", { type: "checkbox", id: "f-agent" });
  ag.checked = !!(m && m.agent);
  const err = h("div", { class: "block errmsg" });
  // iOS numeric pads show the locale decimal separator (often ","), so use the
  // url keyboard (always has ".") and normalise any typed "," to ".".
  const dots = (el) => el.addEventListener("input", () => {
    if (el.value.includes(",")) el.value = el.value.replace(/,/g, ".");
  });
  const main = list(LIST + (IOS ? " zw-inline" : ""),
    inp("name", "Name", "Desktop", m && m.name),
    inp("mac", "MAC", "AA:BB:CC:DD:EE:FF", m && m.mac),
    inp("ip", "IP", "192.168.1.20", m && m.ip, "text", "url"),
    inp("broadcast", "Broadcast", "Default from IP", m && m.broadcast, "text", "url"));
  dots(f.ip); dots(f.broadcast);
  f.ip.addEventListener("input", () => {
    const o = f.ip.value.trim().split(".");
    f.broadcast.placeholder = o.length === 4 ? o[0] + "." + o[1] + "." + o[2] + ".255" : "Default from IP";
  });
  const agentList = list(LIST, h("li", { class: "item-content" }, h("div", { class: "item-inner" },
    h("div", { class: "item-title" }, h("label", { for: "f-agent" }, "Has zerowol agent"), h("div", { class: "item-footer" }, "Needed for Restart and Shut Down")),
    h("div", { class: "item-after" }, h("label", { class: "toggle toggle-init" }, ag, h("span", { class: "toggle-icon" }))))));
  const adv = h("li", { class: "accordion-item" },
    h("a", { class: "item-link item-content", href: "#" }, h("div", { class: "item-inner" }, h("div", { class: "item-title" }, "Advanced"))),
    h("div", { class: "accordion-item-content" }, list(IOS ? "zw-inline" : "",
      inp("wol_port", "WoL port", "9", m && m.wol_port, "number", "numeric"),
      inp("probe_port", "Probe port", "22", m && m.probe_port, "number", "numeric"))));
  const advList = list(LIST + " accordion-list", adv);

  let busy = false;
  async function save() {
    if (busy) return;
    Object.values(rows).forEach((r) => r.classList.remove("item-input-invalid"));
    err.textContent = "";
    const body = { name: f.name.value.trim(), mac: f.mac.value.trim(), ip: f.ip.value.trim(), agent: ag.checked };
    if (f.broadcast.value.trim()) body.broadcast = f.broadcast.value.trim();
    if (f.wol_port.value) body.wol_port = +f.wol_port.value;
    if (f.probe_port.value) body.probe_port = +f.probe_port.value;
    busy = true; saveBtn.classList.add("disabled");
    try {
      const r = m ? await api("PATCH", "/machines/" + m.id, body) : await api("POST", "/machines", body);
      await refresh();
      // The user may have gone back while the request was in flight.
      if (view.router.currentPageEl !== pageEl) return;
      if (ag.checked && !(m && m.agent)) view.router.navigate("/machine/" + r.id + "/pair/", { reloadCurrent: true });
      else view.router.back();
    } catch (e) {
      busy = false; saveBtn.classList.remove("disabled");
      const k = (/^invalid (name|mac|ip|broadcast|wol_port|probe_port)/.exec(e.message) || [])[1];
      if (k && /port$/.test(k)) app.accordion.open(adv);
      if (k) rows[k].classList.add("item-input-invalid");
      err.textContent = offlineMsg(e).replace(/^./, (c) => c.toUpperCase());
    }
  }
  const saveBtn = bigBtn("Save", save);
  const extra = [];
  if (m) {
    const items = [];
    if (m.agent) items.push(linkItem("Agent install command", () => go("/machine/" + m.id + "/pair/")));
    items.push(linkItem("Delete Machine", () => confirmDialog("Delete " + m.name + "?", "This removes the machine from zerowol. The PC itself is not touched.", "Delete", true, async () => {
      try {
        await api("DELETE", "/machines/" + m.id);
        T.delete(m.id);
        await refresh();
        if (view.router.currentPageEl === pageEl) view.router.back();
      } catch (e) { fail(e); }
    }), "color-red"));
    extra.push(list(LIST, items));
  }
  f.ip.dispatchEvent(new Event("input"));
  const form = h("form", { onsubmit: (e) => { e.preventDefault(); save(); } }, main, agentList, advList, h("button", { type: "submit", hidden: true }));
  const pageEl = page("machine", m ? "Edit Machine" : "Add Machine", null,
    form, err, h("div", { class: "block zw-actions" }, saveBtn), extra);
  return pageEl;
}

/* ---------- pairing page ---------- */
function pairingPage(m) {
  let alive = true, gen = 0;
  const cmd = h("div", { class: "code" }, "…");
  const wait = h("span", null, "Waiting for agent…");
  const st = h("div", { class: "pair" }, h("span", { class: "dot busy" }), wait);
  const cp = bigBtn("Copy", () => copy(cmd.textContent, cp));
  const done = h("a", { class: "link back", href: "#" }, "Done");
  const el = page("pair", "Pair " + m.name, done,
    blockTitle("Install command"),
    h("div", { class: "block block-strong inset" },
      h("p", null, "Run this command on " + m.name + " (Linux)."), cmd, h("div", { class: "pair-btn" }, cp), st),
    list(LIST, linkItem("Regenerate secret", () => confirmDialog("Regenerate secret?", "The installed agent will stop working until you run the new command.", "Regenerate", true,
      () => load(true)), "color-red")),
    footer("Regenerating invalidates the secret the agent currently uses."));
  onRemove(el, () => { alive = false; });
  // Rotation happens in place (never via a URL), so history back/forward can't
  // silently regenerate the secret.
  async function load(rotate) {
    const my = ++gen;
    cmd.textContent = "…";
    st.className = "pair";
    wait.textContent = "Waiting for agent…";
    st.replaceChildren(h("span", { class: "dot busy" }), wait);
    try {
      const r = await api(rotate ? "POST" : "GET", "/machines/" + m.id + "/agent/" + (rotate ? "rotate" : "install-command"));
      if (my === gen) cmd.textContent = r.install_command;
    } catch (e) { fail(e); }
    while (alive && my === gen) {
      await sleep(2000);
      try {
        const s = await api("GET", "/machines/" + m.id + "/status");
        if (!alive || my !== gen) return;
        if (s.agent !== "paired") wait.textContent = s.agent_error ? "Agent: " + s.agent_error : "Waiting for agent…";
        else {
          st.className = "pair ok";
          st.replaceChildren("Paired ✓");
          refresh();
          return;
        }
      } catch {}
    }
  }
  load(false);
  return el;
}

/* ---------- settings page ---------- */
function settingsPage() {
  const box = h("div");
  const nameIn = h("input", { type: "text", placeholder: "Token name", autocapitalize: "off", autocomplete: "off", maxlength: 64 });
  const date = (s) => new Date(s * 1000).toLocaleDateString();
  async function load(newTok) {
    let toks = [];
    try { toks = await api("GET", "/tokens"); } catch (e) { fail(e); }
    const create = async () => {
      try { const r = await api("POST", "/tokens", { name: nameIn.value.trim() }); nameIn.value = ""; load(r); } catch (e) { fail(e); }
    };
    box.replaceChildren(...[
      newTok && h("div", { class: "block block-strong inset new-token" },
        h("p", null, "Copy " + newTok.name + " now. It won't be shown again."),
        h("div", { class: "code" }, newTok.token),
        h("div", { class: "pair-btn" }, bigBtn("Copy", function () { copy(newTok.token, this); }))),
      list(LIST,
        toks.map((t) => h("li", { class: "item-content" }, h("div", { class: "item-inner" },
          h("div", { class: "item-title" }, t.name, h("div", { class: "item-footer" }, "Created " + date(t.created) + " · " + (t.last_used ? "Used " + date(t.last_used) : "Never used"))),
          h("div", { class: "item-after" }, h("a", { class: "link color-red", href: "#", onclick: (e) => {
            e.preventDefault();
            confirmDialog("Revoke " + t.name + "?", "Anything using this token will stop working.", "Revoke", true, async () => {
              try { await api("DELETE", "/tokens/" + t.id); } catch (e) { fail(e); }
              load();
            });
          } }, "Revoke"))))),
        h("li", { class: "item-content item-input token-row" }, h("div", { class: "item-inner" },
          h("div", { class: "item-input-wrap" }, nameIn),
          h("div", { class: "item-after" }, h("a", { class: "link", href: "#", onclick: (e) => { e.preventDefault(); create(); } }, "Create")))))].filter(Boolean));
  }
  nameIn.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); e.target.closest("li").querySelector(".link").click(); } });

  const radio = (group, current, value, title, sub, apply) => h("li", null, h("label", { class: "item-radio item-radio-icon-start item-content" },
    h("input", { type: "radio", name: group, value, checked: current === value, onchange: () => apply(value) }),
    h("i", { class: "icon icon-radio" }),
    h("div", { class: "item-inner" }, h("div", { class: "item-title" }, title, sub ? h("div", { class: "item-footer" }, sub) : null))));
  const setTheme = (v) => { store(THEME_KEY, v); location.reload(); };
  const setScheme = (v) => { store(SCHEME_KEY, v); applyScheme(); };
  const tp = themePref(), sp = schemePref();
  const el = page("settings", "Settings", null,
    blockTitle("Theme"),
    list(LIST,
      radio("zw-theme", tp, "auto", "Automatic", "Match this device", setTheme),
      radio("zw-theme", tp, "ios", "iOS", null, setTheme),
      radio("zw-theme", tp, "md", "Android", null, setTheme)),
    blockTitle("Appearance"),
    list(LIST,
      radio("zw-scheme", sp, "auto", "Automatic", "Follow system light/dark setting", setScheme),
      radio("zw-scheme", sp, "light", "Light", null, setScheme),
      radio("zw-scheme", sp, "dark", "Dark", null, setScheme)),
    blockTitle("API Tokens"), box,
    footer("Tokens let scripts call the API with Authorization: Bearer."),
    blockTitle("About"),
    list(LIST,
      h("li", { class: "item-content" }, h("div", { class: "item-inner" }, h("div", { class: "item-title" }, "Version"), h("div", { class: "item-after" }, VERSION))),
      h("li", null, h("a", { class: "item-link item-content external", href: REPO, target: "_blank", rel: "noopener" }, h("div", { class: "item-inner" }, h("div", { class: "item-title" }, "zerowol on GitHub"))))));
  load();
  return el;
}

/* ---------- boot ---------- */
$(".page[data-name=home]").addEventListener("page:reinit", refresh);
// A confirm dialog must not outlive the page that opened it (Android system back).
app.on("routeChange", () => app.dialog.close());
if ("serviceWorker" in navigator) navigator.serviceWorker.register("/sw.js").catch(() => {});
refresh();
