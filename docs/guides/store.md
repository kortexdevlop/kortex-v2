# Kortex Store

## Current delivery
The same `kortex_store::market::App` component builds for browser and desktop. The browser catalogue contains exactly the 38 frozen tool contracts. Search, universe filters, detail sheets, permission descriptions, language and theme are local. Web links open the existing site/Stage integration.

On desktop, installed state comes from the local package registry and checked binary paths. Refresh re-reads this device. Installed tools open through `kortex-platform`. A browser does not enumerate local installations.

## Installation status
Public desktop installation is closed. The package library already verifies Ed25519 indexes and package hashes, but secure credential storage, install confirmation, complete bundles and three-system installation acceptance remain open in PLAN 8.5/10.4/10.6/10.7/10.9. Do not expose an install button or bootstrap command as ready until these gates pass.

## Grouped GitHub builds
Dispatch the private repository's `release` workflow with `tools=all`, `only=all`, `intel_mac=true`, `publish=false`. It compiles 38 tools, all beta/stable apps discovered from `registry/apps.json`, and the CLI for four desktop targets; a separate Linux job compiles the web bundles. Native builds use thin LTO and eight codegen units to reduce private CI minutes while retaining release optimization and stripped binaries. This is recorded in package proofs.

Each desktop job verifies the complete requested inventory, target, version and binary SHA-256. `build-proof-<target>.json` binds hashes to the workflow commit and run. The signing job checks the Ed25519 key against the compiled public key and produces a private signed-release-index artifact. Binary packages remain in their per-OS artifacts rather than being stored twice. Successful compilation is not a full OS installation acceptance test.

Only binaries, signed indexes and release notes may enter the public binaries repository. Never publish source code, test fixtures, credentials or private keys there.
