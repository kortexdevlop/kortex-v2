# 0020 — Rust cookie session and explicit metadata backups

Date: 2026-10-17. Status: implementation decision within the existing architecture.

## Context
The next Hub/Desk lot needs a real shared Rust account/sync client. Website authentication and versioned D1 metadata endpoints already exist. Desktop credentials are not yet in a system keychain; reusing the existing plaintext package-client credential file would violate the target requirement.

## Decision
- `kortex-auth` reads the existing API host-only HttpOnly cookie session through a bounded `kortex-platform` transport. Cookies are neither read by application code nor copied into storage. The client uses the configured API origin, credentials include, no cache, no redirects, a timeout and a streaming response cap.
- `kortex-sync` uses typed schema/version/kind-checked snapshots and the existing optimistic-concurrency API. Hub/Desk share one Rust/Dioxus account panel. It never uploads automatically.
- A snapshot contains only the app's typed layout model plus validated language/theme/mode. Hub includes favourites, recent identifiers and widget metadata; local launch counts are excluded and preserved on this device when restoring. Desk includes names of spaces, identifiers, geometry, stacking and minimisation. Files, inputs, outputs, sessions and credentials are not model fields and cannot be serialized from the tool frames.
- Saving over an existing account backup requires confirmation; restoring requires a preview and confirmation. Conflicts reject rather than retry with a newer revision. Each request carries the expected account identifier, checked after resolving the cookie/device identity on the server, closing the cookie-switch race between a client-side check and the actual write.
- Desk restoration is disabled while windows are open. The restore callback checks again after asynchronous account verification, preventing tools opened during that verification from being closed implicitly.
- Native builds compile the same UI/client sources but report account backup unavailable until secure native sign-in/keychain support is delivered. They never read the legacy plaintext credentials as a substitute.

## Consequences
This is an explicit web backup/restore capability, not automatic Sync, field-level conflict merging, end-to-end encryption, native authentication, or completion of the Hub account/security UI. Existing website account management remains linked. No database migration, schema replacement, tool addition or credential distribution is needed. The API change is backward-compatible for existing clients without an expected-account header.

## Validation
Targeted Rust protocol/model/locale tests, platform transport failure/size tests, API typecheck and session/conflict tests, web/macOS compilation, and one compiled-app two-context recipe. Public verification must use only generated QA accounts and remove only those fixture accounts and their snapshots. PLAN.md and the validation record distinguish those proofs from native interactive acceptance.
