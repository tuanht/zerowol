export async function api(method, path, body) {
  const r = await fetch("/api/v1" + path, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  if (r.status === 204) return null;
  let j = null;
  try {
    j = await r.json();
  } catch {}
  if (!r.ok) throw Object.assign(new Error((j && j.error) || "Request failed (" + r.status + ")"), { status: r.status });
  return j;
}

// Framework7 builds dialogs and toasts from HTML strings without escaping, so
// every user-controlled string (machine names, server errors) goes through esc().
const ENT = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
export const esc = (s) => (s == null ? "" : String(s)).replace(/[&<>"']/g, (c) => ENT[c]);

export const offlineMsg = (e) => (e instanceof TypeError ? "Can't reach zerowol" : e.message);
