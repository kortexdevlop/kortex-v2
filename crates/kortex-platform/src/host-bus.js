
(() => {
  if (window.__kortexBus) return;
  window.__kortexBus = true;
  const host = (location.ancestorOrigins && location.ancestorOrigins[0]) || (document.referrer ? new URL(document.referrer).origin : '');
  if (!host || window.parent === window) return;
  const send = (msg) => window.parent.postMessage(msg, host);
  let pending = null;
  const deliver = () => {
    const input = document.querySelector('input[type=file]');
    if (!pending || !input) return false;
    const dt = new DataTransfer();
    for (const f of pending) { if (f instanceof File) dt.items.add(f); }
    pending = null;
    input.files = dt.files;
    input.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  };
  window.addEventListener('message', (e) => {
    if (e.source !== window.parent || e.origin !== host || !e.data || typeof e.data.type !== 'string') return;
    if (e.data.type === 'kortex:init') {
      if (e.data.theme === 'light' || e.data.theme === 'dark') document.documentElement.dataset.theme = e.data.theme;
      else delete document.documentElement.dataset.theme;
      if (['fr', 'en'].includes(e.data.lang) && typeof dioxus !== 'undefined') { document.documentElement.lang=e.data.lang; dioxus.send('lang:'+e.data.lang); }
      if (['simple', 'medium', 'pro'].includes(e.data.mode) && typeof dioxus !== 'undefined') dioxus.send(e.data.mode);
      if (e.data.size === 'large') document.documentElement.dataset.size = 'large';
      else if (e.data.size === '' || e.data.size === 'normal') delete document.documentElement.dataset.size;
    } else if (e.data.type === 'kortex:files' && Array.isArray(e.data.files)) {
      pending = e.data.files;
      let tries = 0;
      const t = setInterval(() => { if (deliver() || ++tries > 60) clearInterval(t); }, 100);
    }
  });
  send({ type: 'kortex:ready' });
})();
