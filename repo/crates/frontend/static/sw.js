// TerraOps service worker — tiered offline cache for static resources and
// images (Audit #7 Issue #3).
//
// Tiers:
//   1. "static"  — cache-first with version pin. App shell assets (the Yew
//      wasm bundle, JS loader, CSS, favicon, logo, index.html). Served from
//      cache on offline; refreshed on deploy via version bump.
//   2. "images"  — stale-while-revalidate. User-uploaded product images and
//      anything else fetched from /api/v1/products/*/image or /static/*.svg
//      beyond the app shell. Cached copies keep the UI usable offline while
//      a background fetch refreshes them when the network is reachable.
//   3. "api"     — network-first. All /api/v1/* calls hit the network and
//      fall back to cache only if we already have a stored copy. This keeps
//      data fresh while preserving a read-only offline experience.
//
// Non-GET requests are never cached and always go straight to the network,
// so mutations (POST/PUT/PATCH/DELETE) behave exactly as before.

const VERSION = 'terraops-v1';
const STATIC_CACHE = `${VERSION}-static`;
const IMAGE_CACHE = `${VERSION}-images`;
const API_CACHE = `${VERSION}-api`;

const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/static/tailwind.css',
  '/static/favicon.svg',
  '/static/logo.svg',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(STATIC_CACHE).then((cache) =>
      // Best-effort preload — if any individual asset 404s on this build
      // we still let the SW install so the rest of the cache can work.
      Promise.all(
        STATIC_ASSETS.map((url) =>
          cache.add(url).catch(() => undefined),
        ),
      ),
    ),
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((k) => !k.startsWith(VERSION))
          .map((k) => caches.delete(k)),
      ),
    ),
  );
  self.clients.claim();
});

function isImageRequest(req, url) {
  if (req.destination === 'image') return true;
  return /\.(png|jpe?g|gif|webp|svg|ico)$/i.test(url.pathname);
}

function isStaticShellRequest(url) {
  if (url.pathname === '/' || url.pathname === '/index.html') return true;
  if (url.pathname.startsWith('/static/')) return true;
  // Trunk emits hashed /index-<hash>.js and /index-<hash>_bg.wasm at the root.
  if (/\/index(-[\w]+)?(_bg)?\.(js|wasm)$/.test(url.pathname)) return true;
  return false;
}

function isApiRequest(url) {
  return url.pathname.startsWith('/api/');
}

async function cacheFirst(cacheName, req) {
  const cache = await caches.open(cacheName);
  const cached = await cache.match(req);
  if (cached) return cached;
  try {
    const res = await fetch(req);
    if (res && res.ok) cache.put(req, res.clone());
    return res;
  } catch (err) {
    if (cached) return cached;
    throw err;
  }
}

async function staleWhileRevalidate(cacheName, req) {
  const cache = await caches.open(cacheName);
  const cached = await cache.match(req);
  const networkPromise = fetch(req)
    .then((res) => {
      if (res && res.ok) cache.put(req, res.clone());
      return res;
    })
    .catch(() => cached);
  return cached || networkPromise;
}

async function networkFirst(cacheName, req) {
  const cache = await caches.open(cacheName);
  try {
    const res = await fetch(req);
    if (res && res.ok) cache.put(req, res.clone());
    return res;
  } catch (err) {
    const cached = await cache.match(req);
    if (cached) return cached;
    throw err;
  }
}

self.addEventListener('fetch', (event) => {
  const req = event.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  // Only handle same-origin traffic; anything external is left alone.
  if (url.origin !== self.location.origin) return;

  if (isStaticShellRequest(url)) {
    event.respondWith(cacheFirst(STATIC_CACHE, req));
    return;
  }
  if (isImageRequest(req, url)) {
    event.respondWith(staleWhileRevalidate(IMAGE_CACHE, req));
    return;
  }
  if (isApiRequest(url)) {
    event.respondWith(networkFirst(API_CACHE, req));
    return;
  }
});
