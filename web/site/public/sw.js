// Cache public site assets only. API failures must never become cached HTML successes.
const CACHE = 'kortex-site-v4';
self.addEventListener('install', event => {
  event.waitUntil(caches.open(CACHE).then(cache => cache.addAll(['/', '/kortex.css'])).then(() => self.skipWaiting()));
});
self.addEventListener('activate', event => {
  event.waitUntil(caches.keys().then(keys => Promise.all(
    keys.filter(key => key.startsWith('kortex-') && key !== CACHE).map(key => caches.delete(key))
  )).then(() => self.clients.claim()));
});
self.addEventListener('fetch', event => {
  const request = event.request;
  // The API is never intercepted; tools have their own cache on their isolated origin.
  if (request.method !== 'GET' || new URL(request.url).origin !== self.location.origin) return;
  event.respondWith((async () => {
    const cache = await caches.open(CACHE);
    try {
      const response = await fetch(request);
      if (response.ok && !/(?:no-store|private)/i.test(response.headers.get('Cache-Control') || '')) {
        event.waitUntil(cache.put(request, response.clone()));
      }
      return response;
    } catch {
      const cached = await cache.match(request);
      if (cached) return cached;
      if (request.mode === 'navigate') return await cache.match('/') || Response.error();
      return Response.error();
    }
  })());
});
