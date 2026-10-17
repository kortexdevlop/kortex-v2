# 0016 — Runtime depends on platform capabilities

Date: 2026-10-16
Status: implemented; validation evidence in PROGRESS.md.

## Context

The previous platform crate depended on runtime values for native saves and Worker messages. This prevented Cx from calling platform entropy and clocks without a dependency cycle. The UI and generated web entry also evaluated platform scripts directly.

## Decision

Make kortex-platform independent of kortex-runtime. Native saves accept a name and bytes. Worker execution is generic over serde-compatible messages; its owned failure type belongs to the transport layer, reexported by runtime to preserve callers and the serialized protocol. ValueMap, FileData, ToolError and engine conversions remain in runtime.

Cx delegates default clocks and CSPRNG to kortex-platform; injected sources retain precedence. The synchronous headless executor also yields via the platform. Package installation reads the same platform clock.

An optional `document` feature provides Dioxus browser/WebView adapters: selected-file reads, clipboard writes, downloads, embedding bus and launch query. UI invokes named operations, never raw eval. Generated web entries use the same operations and are scanned before compilation, as desktop entries already are. Dioxus event handling and rendering remain in UI; system effects belong in platform. The headless platform dependency does not enable Dioxus or document helpers.

Scripts use JSON serialization for untrusted strings. Downloads release their temporary object URL and remove their anchor, including after a click failure. The existing bus still checks exact parent window and origin; moving its adapter does not constitute the planned full typed bus crate.

## Alternatives

Injecting defaults only from every generated host would leave direct engine calls without a functional default clock or entropy. Creating a new low-level crate merely to break the cycle is unnecessary: generic transport boundaries remove the reverse dependency.

## Limits

This establishes the dependency boundary, not a complete platform feature set. Browser storage, dialogs, native keychains, contract permission enforcement, streaming limits, native execution isolation and cross-OS release validation retain their separate unvalidated plan steps. Clipboard/read-file errors retain existing UI behavior; a full error/permission UX belongs to those follow-up steps. No new tool or product app is introduced.
