//! Tool-local offline cache registration; never touches the account/API origin.
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function reportErrors(api) {
    try {
        // Respect the browser's privacy signals; at most three reports per page load.
        if (navigator.doNotTrack === '1' || navigator.globalPrivacyControl) return;
        const app = location.hostname.split('.')[0];
        let sent = 0;
        const send = (kind) => {
            if (sent >= 3) return;
            sent += 1;
            fetch(api + '/v1/telemetry', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', 'X-Kortex-Request': '1' },
                body: JSON.stringify({ app, kind }),
                keepalive: true,
                credentials: 'omit',
            }).catch(() => {});
        };
        addEventListener('error', (e) => send(/unreachable|RuntimeError|out of memory/i.test(String(e.message)) ? 'panic' : 'unhandled'));
        addEventListener('unhandledrejection', () => send('unhandled'));
    } catch (_) {}
}

export function registerOffline(url) {
    if (globalThis.isSecureContext && 'serviceWorker' in navigator) {
        navigator.serviceWorker.register(url).catch(() => {});
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = registerOffline)]
    pub fn register(url: &str);

    /// Light error monitoring: tells the account API that this app failed (app name and kind
    /// only, see `services/api/src/routes/telemetry.ts`).
    #[wasm_bindgen(js_name = reportErrors)]
    pub fn report_errors(api: &str);
}
