# Scale foundations: migration slice, 2026-10-22

These slices implement additive foundations for the proposed 3,000-tool architecture. Local project persistence and bounded media reads are now available through opt-in shared APIs. They do not replace the v1 engine ABI, load a custom editor UI, implement codecs or supervise a native media process. Existing tools retain their current contracts and execution path.

## Compatibility

The v2 operation contract adapts a legacy tool into one `run` operation without granting stronger cancellation, retry or undo guarantees. Project resources are opaque local identifiers, never file paths or embedded media bytes. The project workflow executor accepts the host's explicit grants and validates project revisions. Cancellation requested and cancellation acknowledged are separate states.

The signed modular catalogue is emitted alongside the legacy catalogue. Existing app clients continue reading their legacy catalogue until a later migration explicitly switches them to lazy resolution. A successful 3,000-entry synthetic test validates discovery and integrity mechanics, not 3,000 delivered tool implementations.

An old index can still verify correctly while its old fragment is absent from a newer static deployment. This slice does not guarantee historical-fragment retention on the live host. Before app cutover, implement retention/cache or a controlled refresh on a missing fragment; do not silently mix new fragments with the old signed snapshot.

## Local executable recipes

Run `cargo run -p kortex-project --example shared_project` for the shared metadata-command example. It declares a 40 GB media reference without reading or allocating that media; it proves revisions/history/reference transport, not video processing.

The modular release writer emits `catalog-index-v2.json`, its detached signature and `catalog-contracts-v2.tar.gz` next to the legacy release files. Extract the archive under the same directory to obtain `contracts/<sha256>.json`. The consumer is `cargo xtask catalog-resolve --root DIR --id TOOL_ID --state FLOOR_FILE`; it defaults to the embedded public key and stable channel. `--public-key FILE` is an explicit developer/operator trust override, never an automatic recovery fallback. The floor is locked and written transactionally; unreadable/corrupt state is an error.

## Administrator configuration migration

The API no longer grants dashboard access based on `ADMIN_EMAILS`: registration does not prove email ownership. Provision `ADMIN_USER_IDS` as a Worker secret containing the exact account ids approved by the operator. Verify the existing account through a trusted operator procedure; copying an unverified email into the secret is not a valid replacement. A signed-in user can see their own id through `/v1/me`, but that response alone does not prove they should be an administrator.

Until the operator provisions the new secret, the dashboard intentionally returns 404. No secret or account has been changed remotely by this implementation. Roll out the configuration with the API release; the other account features do not depend on this secret.

## Local resources and durable sessions (ADR 0037)

`kortex-platform::media::Broker` accepts host-selected native files or browser File/Blob objects and returns non-serializable handles. The host retains the association between a project's resource identifiers and these handles. Engines, model arguments and saved snapshots cannot mint access from a resource identifier. Check the project on every read; revoke handles on project closure. Native reads belong on a background thread.

Reads are bounded to at most 4 MiB; a broker retains at most its configured handle quota (64 by default). File paths and browser filenames are not exposed by metadata. Native length/mtime checks invalidate changed files but are not content hashes or immutable snapshots. Browser File/Blob handles and native capabilities must be reacquired after reopening. There is no cross-origin media transport or durable relinking yet.

Enable `kortex-project/persistence` to use `persistence::DurableSession`. Create/load with `projects::NativeStore` on desktop or `projects::WebStore` in a browser. Snapshot metadata is capped at 1 MiB. Native operations lock each project's read/check/write transaction; IndexedDB performs the browser transaction across tabs. A stale revision is an explicit conflict. Store native data in a trusted application directory, not an untrusted shared folder. Browser storage is origin-local and may be unavailable, cleared or evicted.

`apply`, `undo` and `redo` commit a candidate snapshot before exposing the new in-memory project. If `needs_reload()` becomes true after a conflict, uncertain storage error or dropped pending write, reopen before continuing. Undo/redo history, retry receipts and resource capabilities are not persisted. Do not automatically retry a command after reopening: persistent command deduplication is not implemented.

The earlier `save`/`load` helpers using `Storage` retain their 64 KiB limit and single-writer requirement. They use a separate storage path; there is no automatic migration to `SnapshotStore`.

Enable `kortex-workflow/project-persistence` for `Plan::execute_persisted`. It resolves the operations first and persists every successful command separately. It stops at the first error; previously committed steps remain committed.

Native executable recipe: `cargo run -p kortex-workflow --features project-persistence --example durable_project -- PATH_TO_EXPLICIT_LOCAL_DIRECTORY`. This writes metadata for the example project only; it exercises shared Ui/Flow/Mind provenance, conflict detection and reopen without providing the product UI. Browser adapter recipe: set `CHROME_PATH` if required, then run `pnpm e2e:projects`. This tests the actual IndexedDB adapter in two browser tabs; it does not test a complete editor application.

## Remaining work

- Full transaction/idempotency journal for AI reservations and settlement under partial D1 failure.
- Durable history/retry journal, resource relinking and cross-origin capability transport; progressive streaming/backpressure beyond one bounded range read.
- Actual editors and Flow/Mind UI integration, headless operation dispatch and extended engine protocol.
- Native process cancellation, memory/disk budgets, codecs and custom UI loading prototypes.
- Lazy catalogue consumption in the six apps, release installation transaction and full OS recipes.

Use the current session's validation record to distinguish source changes, local tests, native/wasm compilation and publication. Do not mark these remaining capabilities complete on the strength of the pure-model tests.
