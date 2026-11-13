const CACHE = "zerowol-__VERSION__";
const SHELL = [
  "/",
  "/app.js",
  "/app.css",
  "/manifest.webmanifest",
  "/icon.svg",
];
self.addEventListener("install", (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((ks) => Promise.all(ks.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});
self.addEventListener("fetch", (e) => {
  const u = new URL(e.request.url);
  if (e.request.method !== "GET" || u.origin !== location.origin) return;
  if (u.pathname.startsWith("/api/") || u.pathname.startsWith("/agent/") || u.pathname === "/healthz") return;
  e.respondWith(
    fetch(e.request)
      .then((r) => {
        if (r.ok && (SHELL.includes(u.pathname) || u.pathname.startsWith("/icons/"))) {
          const copy = r.clone();
          caches.open(CACHE).then((c) => c.put(e.request, copy));
        }
        return r;
      })
      .catch(() => caches.match(e.request, { ignoreSearch: true }))
  );
});
