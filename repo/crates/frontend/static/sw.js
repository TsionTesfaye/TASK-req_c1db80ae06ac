// TerraOps service worker — tiered offline cache for static resources and
// images (Audit #7 Issue #3, hardened by Audit #11 Issue #2).
//
// Tiers:
//   1. "static"  — cache-first with version pin. App shell assets (the Yew
//      wasm bundle, JS loader, CSS, favicon, logo, index.html). Served from
//      cache on offline; refreshed on deploy via version bump.
//   2. "images"  — stale-while-revalidate. User-uploaded product images and
//      anything else fetched from /api/v1/products/*/image or /static/*.svg
//      beyond the app shell. Cached copies keep the UI usable offline while
//      a background fetch refreshes them when the network is reachable.
//
// Non-GET requests are never cached and always go straight to the network,
// so mutations (POST/PUT/PATCH/DELETE) behave exactly as before.
//
// Audit #11 Issue #2 — authenticated-response isolation:
//   * Any request that carries an `Authorization` header (i.e. per-user
//     bearer-authenticated traffic) is passed through directly and never
//     cached. This is true for `/api/*` and for authenticated image
//     fetches alike. The previous "api" network-first tier cached
//     responses keyed only by URL, which would allow one logged-in user
//     to read another user's cached response on a shared device. That
//     tier has been removed.
//   * The image cache only stores public / unauthenticated GETs. Signed
//     `/api/v1/images/{id}?exp=..&sig=..` requests carry the caller's
//     bearer token and are therefore never cached here either.
//   * On logout the app posts `{type:'logout'}` to the active SW, which
//     purges the image cache so no previously fetched user data remains
//     on the device for the next account that signs in.

// Bumped to v2 (Audit #11 Issue #2). The old `v1-api` cache is dropped
// on activate by the existing prefix-based purge below.
const VERSION = 'terraops-v2';
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

// A request is treated as authenticated if it carries any Authorization
// header. Such responses are user-scoped and must never be written to a
// shared cache keyed only by URL.
function isAuthenticatedRequest(req) {
  try {
    return req.headers && req.headers.has('Authorization');
  } catch (_e) {
    return false;
  }
}

self.addEventListener('fetch', (event) => {
  const req = event.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  // Only handle same-origin traffic; anything external is left alone.
  if (url.origin !== self.location.origin) return;

  // Never cache or short-circuit authenticated traffic — pass it straight
  // through so every read is re-authorized by the server per caller.
  if (isAuthenticatedRequest(req)) return;

  // API traffic without an Authorization header is either a public
  // endpoint (e.g. /api/v1/healthz) or a pre-auth probe; we still do not
  // cache /api/* responses to avoid any accidental cross-session reuse
  // if an app path ever omits the header.
  if (isApiRequest(url)) return;

  if (isStaticShellRequest(url)) {
    event.respondWith(cacheFirst(STATIC_CACHE, req));
    return;
  }
  if (isImageRequest(req, url)) {
    event.respondWith(staleWhileRevalidate(IMAGE_CACHE, req));
    return;
  }
});

// Audit #11 Issue #2: the app posts `{type:'logout'}` on sign-out so we
// can drop any user-scoped caches before the next user signs in on the
// same device. Static shell cache is kept because it is not user-scoped.
self.addEventListener('message', (event) => {
  const data = event.data;
  if (!data || typeof data !== 'object') return;
  if (data.type === 'logout') {
    event.waitUntil(
      Promise.all([
        caches.delete(IMAGE_CACHE),
        caches.delete(API_CACHE),
      ]),
    );
  }
});
