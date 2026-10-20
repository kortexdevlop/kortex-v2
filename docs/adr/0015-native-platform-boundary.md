# 0015 — Native host capabilities in kortex-platform

Date: 2026-10-16
Status: implemented for the native migration below; remaining platform work stays open.

## Context

Package installation, account transport, CLI input/output, Store launch and generated desktop saves called the operating system directly. This violated the shared platform boundary (PLAN 3.2).

## Decision

Move filesystem operations, environment reads, executable launch, terminal input, bounded HTTP transport and CLI bootstrap into the native-only `kortex-platform::native` backend. Keep installation policy, signature checks, account derivation and catalog logic in their existing crates. Reuse ureq and rpassword inside the backend; do not introduce another transport or terminal library. Native dependencies are excluded from wasm builds.

Save user outputs through exclusive creation, preserving existing files and allocating numbered names on collisions. Read local downloads through a streaming size cap, including after the metadata check. Refuse remote plaintext HTTP; local HTTP does not follow redirects. Preserve HTTP statuses for account error handling.

`xtask check` scans production Rust under crates/apps/tools for common direct filesystem, transport, terminal and process calls. Exclude the platform backend, compile-time procedural macros and test fixtures. Check generated desktop source before compilation too. This conservative text scan is a regression guard, not a complete Rust capability or dependency analyzer; grouped imports and indirect dependencies require further audit.

## Alternatives

Leaving direct calls in each host duplicates platform integration and violates the requested architecture. A full universal trait redesign in this lot would mix this migration with unfinished browser, keychain and runtime work.

## Limits and follow-up

This native backend does not complete portable file/dialog/storage/notification interfaces (3.3), native thread/IPC execution (3.4/3.6), contract permission enforcement or OS keychains (8.5). Credentials still use the existing local file implementation; owner-only Unix mode is not a keychain. Windows ACL hardening is not implemented here.

The expanded audit also found browser output/clipboard/host-transfer evaluation in `kortex-ui/src/auto.rs`, and clock/random access in `kortex-runtime`. ADR 0016 records the subsequent migration and dependency inversion; its validation is tracked in PROGRESS.md. This native migration alone did not validate all of 3.2.

Native compilation on the current macOS host is distinct from interactive validation or Windows/Linux release validation. Public downloads remain disabled until distribution requirements pass.
