/* LOOM — service worker (PWA)
 * Stratégie : coquille d'app préchargée à l'installation ; navigation réseau
 * d'abord (les mises à jour arrivent), repli sur la coquille en cache une fois
 * hors-ligne ; ressources statiques (samples/…, police) en cache runtime
 * stale-while-revalidate pour jouer hors-ligne après un premier passage.
 * Rien n'est gardé pour les autres origines ; tout est éphémère sauf la
 * coquille (mise à jour par le navigateur quand sw.js change). */
const CORE = "loom-shell-v1";
const RUNTIME = "loom-runtime-v1";
const SHELL = [
  "./",
  "index.html",
  "manifest.webmanifest",
  "icon-192.png",
  "icon-512.png",
  "icon-512-maskable.png",
  "pkg/loom_wasm.js",
  "pkg/loom_wasm_bg.wasm",
];

self.addEventListener("install", (e) => {
  e.waitUntil(
    caches.open(CORE).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CORE && k !== RUNTIME).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET") return;

  // Navigations : réseau d'abord, repli coquille en cache (hors-ligne).
  if (req.mode === "navigate") {
    e.respondWith(
      fetch(req).then((res) => {
        const copy = res.clone();
        caches.open(CORE).then((c) => c.put("index.html", copy));
        return res;
      }).catch(() =>
        caches.match("index.html").then((r) => r || caches.match("./"))
      )
    );
    return;
  }

  const url = new URL(req.url);
  // Même origine seulement (les polices Google restent best-effort, sans cache).
  if (url.origin !== self.location.origin) return;

  // Statique (samples/*.wav, pkg/*, icônes) : cache d'abord, puis réseau,
  // et on enrichit le cache au passage — la première visite rend jouable en
  // hors-ligne les samples déjà entendus.
  e.respondWith(
    caches.match(req).then((hit) => {
      const net = fetch(req).then((res) => {
        if (res.ok) {
          const copy = res.clone();
          caches.open(RUNTIME).then((c) => c.put(req, copy));
        }
        return res;
      });
      return hit || net;
    })
  );
});
