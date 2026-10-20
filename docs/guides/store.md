# Kortex Store

## Current delivery
The same `kortex_store::market::App` component builds for browser and desktop (Store 0.3.0, lot 36). The catalogue contains exactly the 38 frozen tool contracts; it comes from `registry/index.json`, embedded at compile time through `kortex-shell`, so a registry change requires rebuilding the Store. Search, universe filters, detail sheets, permission descriptions, language and theme are local. Web links open the existing site/Stage integration.

On desktop, the signed release index is loaded off the UI thread and the Store offers Install, Update, Remove and Open for the 38 tools and an Applications section (Hub, Desk, Flow, Mind, Seek), with the same rules as `kortex install`: device account required, password confirmation when the account demands it, offline fallback to the local registry. Installed state comes from the local package registry and checked binary paths; installed tools open through `kortex-platform`. On the web the Store links to `/telecharger`; a browser does not enumerate local installations.

## Installation status
Public desktop installation has been open since 2026-10-19 (explicit user authorization). Packages come from the GitHub Release of `kortex-v2` named in the signed index; see `docs/guides/release.md` for the current release and open limits. Still open: sign-in from the Store (use `kortex login`), per-permission consent and cancellation (10.4), installation acceptance by the agent on Windows/Linux/ARM (10.7/10.9), anti-rollback of the index (10.2).

## Grouped GitHub builds
Dispatch the `release` workflow with `tools=all`, `only=all`, `intel_mac=true` and `publish=false` (or `publish=true` to create the public GitHub Release). It compiles 38 tools, all beta/stable apps discovered from `registry/apps.json`, and the CLI for four desktop targets; a separate Linux job compiles the web bundles. Native builds use thin LTO and eight codegen units to reduce CI time while retaining release optimization and stripped binaries. This is recorded in package proofs.

Each desktop job verifies the complete requested inventory, target, version and binary SHA-256. `build-proof-<target>.json` binds hashes to the workflow commit and run. The signing job checks the Ed25519 key against the compiled public key and produces the signed-release-index artifact, attached to the GitHub Release when `publish=true`. Binary packages remain in their per-OS artifacts rather than being stored twice. Successful compilation is not a full OS installation acceptance test.

Only binaries, signed indexes and release notes may be attached to a GitHub Release. The source repository is public under a proprietary licence (ADR 0024); never commit or attach credentials or private keys.
