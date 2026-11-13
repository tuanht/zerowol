import { f7 } from "framework7-vue/bundle";
import { esc } from "./api.js";

export function toast(msg, ok) {
  const t = f7.toast.create({
    text: esc(msg),
    position: "top",
    closeTimeout: 4000,
    destroyOnClose: true,
    cssClass: ok ? "" : "toast-bad",
  });
  t.el.setAttribute("role", "status");
  t.el.setAttribute("aria-live", "polite");
  t.open();
}

// Resolves true when the confirm button is pressed, false on Cancel or when the
// dialog is dismissed (including by navigating away).
export function confirmDialog({ title, text, label, red }) {
  return new Promise((resolve) => {
    let done = false;
    const finish = (v) => {
      if (done) return;
      done = true;
      f7.off("routeChange", onRoute);
      resolve(v);
    };
    const d = f7.dialog.create({
      title: esc(title),
      text: esc(text),
      destroyOnClose: true,
      buttons: [
        { text: "Cancel", onClick: () => finish(false) },
        { text: esc(label), strong: true, color: red ? "red" : null, onClick: () => finish(true) },
      ],
      on: { closed: () => finish(false) },
    });
    const onRoute = () => d.close();
    f7.on("routeChange", onRoute);
    d.open();
  });
}

export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const t = document.createElement("textarea");
    t.readOnly = true;
    t.value = text;
    t.style.cssText = "position:fixed;opacity:0";
    document.body.append(t);
    t.select();
    t.setSelectionRange(0, 1e6);
    try {
      document.execCommand("copy");
    } catch {}
    t.remove();
  }
}
