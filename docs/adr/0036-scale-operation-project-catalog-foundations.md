# 0036 — Additive operation, project and modular catalogue foundations

Date: 2026-10-22
Status: implemented local foundation slice following the user's explicit request to start the first three scale workstreams in parallel. The full architecture remains a migration target; codecs, custom UI and app cutover are not implemented by this slice.

## Context

The user requested support for 2,000–3,000 heterogeneous tools, including photo/video editors with dedicated interfaces, and shared operations usable by Flow and Mind. A single `run` contract with complete in-memory file values cannot describe all these uses. The source audit also found concrete security issues that must be corrected before extending the execution surface.

## Decision

- Add `kortex-contract::v2` alongside v1, with multiple named operations, explicit effects/execution capabilities and resource/project reference ports. Adapt existing contracts to one `run`; do not change the engine ABI or grant capabilities an old engine did not implement.
- Add pure `kortex-project` metadata, revision-checked atomic commands, bounded undo/redo and retry receipts, host-issued grants and cooperative job states. Reuse it from the shared workflow project-plan executor. Individual commands are atomic; an entire workflow is not declared transactional.
- Represent media by opaque identifiers and declared metadata. Actual storage, range reads, codecs and OS authorization remain a subsequent platform implementation. `Source::Ui/Flow/Mind` is provenance, not authorization.
- Keep cancellation requested distinct from backend-stopped acknowledgement. The pure state machine is not a native supervisor.
- Publish a signed `catalog-index-v2.json` alongside legacy `catalog.json`, with hash-addressed immutable fragments. Verify signature/channel/floor at the root and size/hash/identity of a selected fragment. The existing six apps retain their current consumer until an explicit cutover.
- Generate the modular files with the release tooling and carry the archive/fragments through publication. A local resolver provides a real lazy-consumption recipe without making an unverified production feature visible.
- Correct native redirection handling, cryptographic RNG failures, guest-controlled host allocations, image metadata fallback, API credit races/day binding, malformed input and administrator identity while expanding these foundations.

No new external dependency is needed. `kortex-project` uses approved serde/JSON/contract libraries; codecs and custom editor runtime choices require measured prototypes later. The legacy catalogue, tool packages and ABI stay available during migration.

## Compatibility and operation

Administrator access moves from self-claimed email to operator-provisioned `ADMIN_USER_IDS`; configure this secret with the API rollout. The role is denied until provisioned. The native Flow HTTP node refuses cross-authority redirects, protecting arbitrary secret headers. The image compressor always returns a sanitized re-encoding and may increase output size.

Synthetic 3,000-tool catalogue tests measure data discovery/integrity and selected-fragment resolution. They do not validate 3,000 products or media execution. Native and wasm checks, project examples, corruption tests and legacy-contract adaptation must be recorded separately from OS recipes and deployment.

## Follow-up

Implement resource storage through the platform; extended engine dispatch; native task supervision; real custom UI/codec prototypes; adoption by Flow/Mind/editor product interfaces; lazy catalogue in apps; transactional package dependencies; media workloads and OS acceptance. AI accounting still needs an idempotent reservation journal for partial failures. None is marked complete by this ADR.
