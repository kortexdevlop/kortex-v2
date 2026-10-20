const [frame, origin] = __KORTEX_ARGS__;
const receive = event => {
  if (event.origin !== origin || event.data?.type !== 'kortex:ready') return;
  const element = Array.from(document.querySelectorAll('iframe[data-kortex-window]')).find(e => e.dataset.kortexWindow === String(frame));
  if (!element || event.source !== element.contentWindow) return;
  element.contentWindow.postMessage({type:'kortex:init', mode:element.dataset.kortexMode || 'medium',lang:element.dataset.kortexLang || 'fr', theme:document.documentElement.dataset.theme || '', size:document.documentElement.dataset.size || 'normal'},origin);
  dioxus.send({version:1,frame});
};
window.addEventListener('message',receive);
try { await dioxus.recv(); }
finally { window.removeEventListener('message',receive); }
