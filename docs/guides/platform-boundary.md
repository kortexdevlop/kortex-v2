# Platform boundary

The dependency direction is `engine → runtime → platform`. UI and native hosts may also depend on platform. Platform never imports runtime or a tool. Shared values stay in runtime; platform operations take primitive arguments or serde-compatible messages. See ADR 0015 and 0016.

Engines obtain time and entropy from `Cx::now_ms` and `Cx::fill_random`. Tests inject a fixed clock or deterministic entropy with the existing Cx builders. Never replace a missing CSPRNG with a pseudo-random fallback. File, network, environment, terminal and process effects belong to platform backends, not engines.

Dioxus hosts enable the optional platform `document` feature for selected-file reads, downloads, host messages and launch query. Call named operations in `kortex_platform::document`; clipboard operations use `kortex_platform::clipboard` with explicit results; do not build JavaScript in UI or generated entry points. Dioxus event handlers and rendering remain in UI. Platform scripts are minimal adapters, never a second implementation of an application.

`xtask check` scans production source and generated entries for common direct system calls. Platform implementations, compile-time procedural macros and test fixtures have legitimate effects. The current scan is conservative text matching, not complete Rust name resolution or dependency capability analysis. Review aliases and new dependencies as well; passing the scan does not prove all permission, storage or isolation requirements.

For common runtime/UI/platform changes, group checks: workspace tests, Clippy, fmt, `cargo xtask check`, and a wasm check of runtime/UI. Rebuild affected web bundles with `cargo xtask build all`: content receipts skip identical sources and outputs. Compile a native dependent when the changed code affects desktop; one successful macOS sample does not validate Windows/Linux or every native binary. For a single engine change, build its dependents only. Site-only edits do not require rebuilding unchanged Rust engines.

Before publishing, run one relevant end-to-end recipe on the finished artifacts. Keep sources, receipts, artifact hashes and deployed version IDs together in validation evidence. A source file, a test, a compilation and a published artifact are different proofs. Native downloads remain disabled until the separate distribution requirements pass. Full storage/dialog/notification backends, keychains, contract permissions and native isolation remain tracked in PLAN.md.

## Non-secret preferences (plan 3.3)

The existing `Storage` interface has persistent backends under `kortex_platform::storage`:

- `WebStorage::open(namespace)` on wasm32 uses browser origin-local localStorage.
- `NativeStorage::open(application_data_root, namespace)` on native targets uses a caller-selected directory. Choosing that root belongs to the host, using platform environment/path helpers.

Both accept UTF-8 keys/namespaces up to 120 bytes, encode them without path separators, store UTF-8 values up to 64 KiB, distinguish missing from empty values and return explicit failures. They implement `get`, `set` and idempotent `remove`. Browser denial/quota errors never silently switch to volatile storage. Native writes replace one key through an exclusive temporary file in its directory, and reads are bounded. See ADR 0017 for concurrency, durability and local-account limitations.

These backends are not for session tokens, passwords, large event histories or user files. No UI is wired to them yet; this is a tested platform capability, not a new visible preference screen. Keychains, async event storage and cross-origin synchronization remain distinct work. The browser recipe exercises the actual Rust wasm backend through a test-only fixture, never a JavaScript replacement:

```sh
cargo test -p kortex-platform --features document
node scripts/tests/storage-backends.mjs
```

The browser recipe uses the existing Playwright/Chrome runtime and offline Cargo cache. Its separate wasm fixture has no product UI and is not published. No new dependency or product tool is installed.


## Clipboard results (plan 3.3)

Use `kortex_platform::clipboard::write_text(text)` and await its result before confirming a copy. The browser operation starts when that function is called, before the returned future is polled. Invoke it directly from the user interaction, then await it in the UI task. Use `read_text().await` only for a deliberate paste/read action, never on component initialization.

The browser backend uses Rust bindings to the Clipboard API. macOS uses Rust AppKit bindings and implements the existing native Clipboard trait; the automated roundtrip uses a unique private pasteboard. Other Dioxus desktop WebViews use a capability/result adapter; Windows/Linux and Safari still need execution acceptance. Headless unsupported native targets return unavailable. See ADR 0018 for the scope and errors.

The shared CopyButton shows confirmed success or an accessible failure in FR/EN, retains the result for manual selection, and never reports success for a failed or stale copy. Grouped recipes: `scripts/tests/clipboard-backends.mjs` for the actual Rust wasm backend, `scripts/tests/platform-ui-smoke.mjs` on rebuilt local bundles, and `scripts/tests/clipboard-public.mjs` for the cross-origin public Stage. The public clipboard recipe creates no account. Native downloads remain closed.
