# 0017 — Persistent preference storage behind platform

Date: 2026-10-17
Status: Accepted implementation scope; plan 3.3 remains partial.

## Context

Storage previously exposed only a synchronous trait and a volatile MemoryPlatform. Applications need real local preference persistence on the web and desktop before shared shells and synchronization can use it. Credentials require the separate keychain work in step 8.5.

## Decision

Implement the existing Storage trait with WebStorage (origin-local localStorage accessed by Rust bindings) and NativeStorage (an explicitly selected native application-data root). Namespace/key strings are injectively encoded as UTF-8 hexadecimal, limited to 120 bytes before encoding to fit ordinary filesystem filename limits. Values are UTF-8 strings, at most 64 KiB per key. Missing and empty values are distinct; removal is idempotent. Browser access/quota failures are returned, without silently falling back to memory.

Native writes use exclusive random temporary files in the same namespace, sync their data, close and rename them over the old entry. Failed replacements clean up their temporary file. Reads are capped even if an entry grows. New Unix directories/files request 0700/0600. The caller owns the root choice; this is not a defense against a hostile local account racing directory replacement. Final-component symlinks are refused for reads; native filesystem access is always within platform. No new dependency is added.

## Consequences

This backend is for small non-secret preferences, not credentials, files, IndexedDB event journals or OPFS streaming. localStorage is origin-scoped, may be cleared by the browser and remains synchronous; the web backend does not provide cross-origin synchronization. Namespace separation is a convention, not a permission boundary. Native writes are last-writer-wins per key, with no multi-key transaction, global size quota or conflict resolution. Orphan temporary files after a process crash are not read as preferences. Hardware power-loss durability across platforms is not claimed.

NativeStorage is compiled and tested on the current macOS host; Windows/Linux execution is separate acceptance work. The UI does not yet consume these backends, and no new user-facing persistence or completed application is claimed. Notifications, dialogs, keychain and the remaining unified interfaces stay open in PLAN.md.

## Alternatives

A new storage dependency or database is unnecessary for bounded preferences. A shared JSON file risks concurrent writers losing unrelated keys. WebView localStorage would tie native persistence to renderer details; native callers instead choose a stable application-data directory. Async IndexedDB is appropriate for future event journals, not required for this small synchronous interface.
