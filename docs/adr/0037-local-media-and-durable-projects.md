# 0037 — Local media capabilities and revision-checked project snapshots

Date: 2026-10-22
Status: local implementation; product UI adoption and deployment pending.

## Context

The initial project model carried opaque resource identifiers and metadata only. Its optional 64 KiB `Storage` snapshot helpers required a single writer. Heavy media and simultaneous UI/automation hosts need bounded reads and a storage transaction that cannot overwrite a newer project revision.

## Decision

- Keep the project core pure by default; persistence remains an opt-in feature.
- Add `kortex-platform::media::Broker`. Only the trusted host registers a selected file or browser Blob and retains the association with project resource identifiers. Handles are private, non-serializable capabilities bound to a broker and project; possession of a serialized resource id never grants access.
- Bound each read to at most 4 MiB and retained handles to a configured quota. Native reads retain an open file and check length/mtime before and after reading. Browser reads use built-in Blob getters and `slice().arrayBuffer()`. These are local range reads, not codecs or immutable native content snapshots.
- Add a bounded byte `SnapshotStore` with revision compare-and-swap. Native storage locks the entire read/check/write transaction, syncs a temporary file and renames it. Browser storage uses a single IndexedDB read-write transaction across tabs. Store exact u64 revisions as strings in JavaScript to avoid number precision loss.
- `DurableSession` validates an incoming snapshot and applies commands to a candidate session. It exposes the new state only after storage confirms the write. A conflict, uncertain storage error or dropped pending write future requires reopening, preventing blind retries after an ambiguous commit.
- Extend the shared workflow executor with opt-in persistence. Resolve all operations before changing a project; persist each command separately and stop at the first failure. Earlier committed commands remain committed.

No new external library, media server, application or tool is introduced. The JavaScript IndexedDB adapter handles storage primitives only; project schema, permissions and commands stay in Rust. Legacy 64 KiB snapshot helpers remain compatible and single-writer only.

## Boundaries and alternatives

The new store caps a snapshot at 1 MiB. It contains local project metadata, not embedded media or authorization handles. Undo/redo history and retry receipts remain process-local; reopening starts a new history. There is no automatic migration from legacy storage or automatic cloud synchronization.

Native roots must be host-controlled application data directories. Cooperative file locking is not a sandbox against an attacker who can replace the directory. Native I/O must run on a background thread; the async trait alone does not move blocking work off the UI thread. Crash and power-loss durability depend on the OS/filesystem; recipes here do not establish guarantees on every supported OS.

Browser databases are origin-local and may be denied, evicted or cleared. No cross-origin resource broker is delivered. File/Blob capabilities require host reselection after reload; native length/mtime checks cannot detect all same-size edits with restored timestamps. Full resource relinking, durable history, codecs, editor UI and product Flow/Mind adoption remain separate work.

`localStorage` read-then-write was rejected for multi-writer snapshots because it cannot provide an atomic revision check across tabs. Copying media into project JSON was rejected because memory and storage would grow with source-file size.
