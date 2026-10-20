type Turnstile = {render:(el:HTMLElement,options:Record<string,unknown>)=>string;reset:(id:string)=>void;getResponse:(id:string)=>string};
declare global { interface Window {turnstile?:Turnstile} }
export function setupTurnstile(root: HTMLElement) {
 const el=root.querySelector<HTMLElement>('[data-turnstile]')!;
 let id:string|undefined, loading:Promise<void>|undefined;
 function load() {
  if(!root.dataset.sitekey) return Promise.resolve();
  if(loading) return loading;
  loading=new Promise<void>((resolve,reject)=>{
   const script=document.createElement('script');script.src='https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';script.async=true;
   script.onload=()=>resolve();script.onerror=()=>{loading=undefined;script.remove();reject(new Error('turnstile'));};document.head.append(script);
  });return loading;
 }
 return {
  show(visible:boolean) {el.hidden=!visible;if(visible) void load().then(()=>{
    if(id===undefined && window.turnstile) id=window.turnstile.render(el,{sitekey:root.dataset.sitekey,action:'register',theme:'auto'});
   }).catch(()=>{el.textContent=root.dataset.lang==='fr'?'Vérification indisponible. Rechargez la page.':'Security check unavailable. Reload the page.';});},
  token:()=>id!==undefined ? window.turnstile?.getResponse(id) ?? '' : '',
  reset:()=>{if(id!==undefined) window.turnstile?.reset(id);},
 };
}
