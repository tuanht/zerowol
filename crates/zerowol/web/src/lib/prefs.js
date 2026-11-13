// Theme (iOS / Material) and colour-scheme overrides, kept in localStorage.
export const THEME_KEY = "zw-theme"; // "auto" | "ios" | "md"
export const SCHEME_KEY = "zw-scheme"; // "auto" | "light" | "dark"

function read(key, allowed) {
  try {
    const v = localStorage.getItem(key);
    if (allowed.includes(v)) return v;
  } catch {}
  return "auto";
}
export const themePref = () => read(THEME_KEY, ["ios", "md"]);
export const schemePref = () => read(SCHEME_KEY, ["light", "dark"]);

export function setPref(key, v) {
  try {
    if (v === "auto") localStorage.removeItem(key);
    else localStorage.setItem(key, v);
  } catch {}
}

// Framework7's darkMode parameter: true, false or "auto".
export const darkParam = () => ({ light: false, dark: true })[schemePref()] ?? "auto";

export function applyScheme(f7app) {
  f7app.setDarkMode(darkParam());
  // Keep the browser chrome (status bar / address bar) in step with the forced scheme.
  const sp = schemePref();
  document.querySelectorAll('meta[name="theme-color"]').forEach((m) => {
    if (!m.dataset.media) m.dataset.media = m.getAttribute("media") || "";
    if (sp === "auto") m.setAttribute("media", m.dataset.media);
    else m.setAttribute("media", m.dataset.media.includes(sp) ? "all" : "not all");
  });
}
