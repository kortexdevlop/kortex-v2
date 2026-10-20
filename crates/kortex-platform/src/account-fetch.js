const [url, method, body, limit, user] = __KORTEX_ARGS__;
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 20000);
try {
  const response = await fetch(url, {
    method, credentials: 'include', cache: 'no-store', redirect: 'error', signal: controller.signal,
    headers: {'X-Kortex-Request':'1', ...(user === null ? {} : {'X-Kortex-User':user}), ...(body === null ? {} : {'Content-Type':'application/json'})},
    ...(body === null ? {} : {body})
  });
  if (Number(response.headers.get('content-length') || 0) > limit) throw new Error('response_too_large');
  const reader = response.body?.getReader();
  const decoder = new TextDecoder('utf-8', {fatal:true});
  let size=0, text='';
  if(reader) {
    try {
      while(true) {
        const {done,value}=await reader.read();
        if(done)break;
        size+=value.byteLength;
        if(size>limit){await reader.cancel();throw new Error('response_too_large');}
        text+=decoder.decode(value,{stream:true});
      }
      text+=decoder.decode();
    } finally {reader.releaseLock();}
  }
  return {status:response.status,body:text};
} catch {
  controller.abort();
  return {status:0,body:''};
} finally {clearTimeout(timer);}
