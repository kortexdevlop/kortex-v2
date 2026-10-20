const [url, body, user] = __KORTEX_ARGS__;
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 120000);
// The Rust side sends one message to cancel.
dioxus.recv().then(() => controller.abort()).catch(() => {});
try {
  const response = await fetch(url, {
    method: 'POST', credentials: 'include', cache: 'no-store', redirect: 'error', signal: controller.signal,
    headers: {'X-Kortex-Request':'1', 'Content-Type':'application/json', ...(user === null ? {} : {'X-Kortex-User':user})},
    body,
  });
  dioxus.send({status: response.status});
  const reader = response.body?.getReader();
  const decoder = new TextDecoder('utf-8');
  let size = 0;
  if (reader) {
    while (true) {
      const {done, value} = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > 4000000) { controller.abort(); throw new Error('response_too_large'); }
      dioxus.send({chunk: decoder.decode(value, {stream: true})});
    }
  }
  dioxus.send({end: true});
} catch {
  dioxus.send({failed: true});
} finally {
  clearTimeout(timer);
}
