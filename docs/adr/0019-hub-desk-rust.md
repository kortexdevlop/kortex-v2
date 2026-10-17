# 0019 — Shared Rust Hub and Desk, staged delivery

Date: 2026-10-17. Status: accepted implementation decision within the existing requirements.

## Context
The user explicitly prioritised rebuilding Hub and delivering the Workspace. The earlier Astro prototypes were removed and are not reused. This request changes the work order in PLAN.md, not the language, platform, privacy or complete functional requirements.

## Decision
- `apps/hub` and `apps/desk` contain pure Rust state models and the same Dioxus UI compiled for wasm and macOS. They have validated `app.toml` contracts, FR/EN locales/content and model tests.
- `kortex-shell` shares catalogue lookup, search, preferences, appearance and application navigation. `kortex-bus` currently types only ready/init handshake messages. Browser listeners and DOM/system operations remain in `kortex-platform`.
- Web Desk hosts the actual tool bundles on the run origin. Both origin and source window are checked. Window focus updates stacking metadata without moving/remounting iframes. Minimised windows and inactive spaces remain mounted. Language, theme and mode update over the bridge.
- Desktop discovers validated, existing installed binaries from the local package registry. Tools launch as separate native windows. An embedded native host and typed tool IPC remain pending; the UI explains the separate window lifecycle.
- Hub saves favourites, recent launches, launch counts and widget metadata locally. Desk saves window geometry and named spaces only. No file contents, results, credentials or cross-device sync are added to this storage.
- Static publishing derives application targets from app contracts and brand domains. Build receipts bind source fingerprints and artifact hashes; source changes during compilation reject the receipt. Only public compiled assets are deployed.

## Consequences and remaining scope
This is a usable first web delivery, not completion of prompt §9.1 or §9.5. Typed transfers/connections, native embedding, Seek integration, account/auth/sync Rust clients, broader widgets, desktop packaging/distribution and Windows/Linux acceptance remain separate unvalidated plan items. No fake metrics, account session, remote sync or unavailable release is displayed. App contracts explicitly declare offline=false.

Alternative rejected: implementing another Hub/Desk in Astro/TypeScript, or using a catalogue entry as evidence of a native installation.
