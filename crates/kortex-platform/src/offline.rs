//! Tool-local offline cache registration; never touches the account/API origin.
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function registerOffline(url) {
    if (globalThis.isSecureContext && 'serviceWorker' in navigator) {
        navigator.serviceWorker.register(url).catch(() => {});
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = registerOffline)]
    pub fn register(url: &str);
}
