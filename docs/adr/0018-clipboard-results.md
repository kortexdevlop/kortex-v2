# 0018 — Confirm clipboard operations before reporting success

Date: 2026-10-17
Status: Accepted scoped implementation; platform step 3.3 remains partial.

## Decision

`kortex_platform::clipboard` provides explicit asynchronous `write_text` and `read_text` results. On wasm the Rust backend invokes the browser API immediately, before awaiting its Promise, preserving the initiating interaction. Rejected permission requests and missing APIs return stable errors; exception details and clipboard contents are never logged. Reads are explicit calls, never performed during UI initialization.

macOS uses the existing objc2 AppKit/Foundation bindings directly in Rust through NSPasteboard. The native Clipboard trait is implemented by NativeClipboard. Tests use a unique private pasteboard and clear only that board, without accessing the general user clipboard. Existing cached dependencies become direct target-specific dependencies; none is installed as a new executable or service.

Other native Dioxus WebViews use a small renderer capability/result adapter when the document feature is enabled; headless unsupported native targets return unavailable. Windows/Linux and Safari execution remain unvalidated. A native compilation is distinct from an interactive desktop recipe.

The shared Dioxus CopyButton reports pending, confirmed success or failure in French/English. Errors remain visible with an alert and a manual-selection instruction; success uses a status region. Each output owns its feedback, and a response for old text is not displayed as success for changed output. No automatic clipboard read, retry, hidden permission workaround or legacy execCommand fallback is added.

## Validation scope

Targeted platform/UI tests, real Rust wasm browser clipboard recipe (including denial/unavailable), native private-pasteboard roundtrip, native sample compilation, and rebuilt shared-UI recipe before publication. Clipboard read is an API for later explicitly requested paste actions, not an automatically reading button in every tool.

References: [objc2 NSPasteboard API](https://docs.rs/objc2-app-kit/0.3.2/objc2_app_kit/struct.NSPasteboard.html), [Apple setString](https://developer.apple.com/documentation/appkit/nspasteboard/setstring(_:fortype:)).
