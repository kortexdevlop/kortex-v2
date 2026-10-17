# 0011 — Passkeys, security settings and registration protection

## Context
Password accounts are public. The next substantial account milestone adds WebAuthn, session management, password rotation and bot protection without a new tool or a paid service.

## Decision
Use the @simplewebauthn/server 14.0.3 and @simplewebauthn/browser 14.0.0 packages, explicitly listed in docs/deps.md before installation. This bounded WebAuthn dependency replaces unsafe custom CBOR/COSE/signature verification; it does not introduce a hosted authentication provider. The current patched release is used; registration is limited to ES256 and attestation none. Review the dependency tree and bundle before deployment.

Use resident credentials and required user verification, strict RP/origin checks, short-lived single-use D1 challenges and counters. Enrolling or removing passkeys requires a cookie session and password confirmation. Discoverable passkeys can sign in without entering an email. Password fallback remains available; this milestone does not remove passwords or add passwordless signup.

Sensitive password/session changes require fresh password proof. Password rotation atomically revokes old browser/device tokens. Registration can use Cloudflare Turnstile with server-side action/hostname validation and a Worker secret; site keys are public, secrets never enter Git. Enable only after a real widget and Worker secret are provisioned. The current token returns 403 for Turnstile API access, so registration protection remains disabled in this milestone. Additive migrations preserve all existing accounts. Deploy after grouped API checks, then exercise the public flow with a virtual authenticator on an isolated QA account.

## Consequences
No model calls, paid provider or custom crypto. Browser support and authenticators still determine whether a user can create a passkey. Cryptographic verification and negative/replay cases require focused tests despite the preference for minimal testing.
