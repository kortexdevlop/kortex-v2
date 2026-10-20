# 0022 — Native credential storage through the platform

Status: implementation in progress; OS acceptance pending.

RULES.md 6.1 requires the system keychain. Owner-only plaintext credentials are insufficient. Use native-only keyring 4.2.0 behind the optional kortex-platform secret-store interface. Its official v1 adapter selects macOS Keychain, Windows Credential Manager and Linux Secret Service; no file fallback or extra command-line executable is introduced. Native pkg consumers opt in; browser cfg excludes this module/dependency.

Missing credentials are distinct from backend failure. Errors intentionally omit provider payloads because providers may attach secret bytes. Tests inject a memory implementation per Home rather than globally replacing the OS keyring. Production selects the native backend regardless of test-like paths.

A legacy record is parsed with a size cap, persisted and read back before removing the plaintext copy. Failed writes/verifications attempt restoration of the prior protected record; failed rollback is explicit. Failed new-token persistence attempts server revocation. The protected service uses a SHA-256 namespace of the canonical app-home path, isolated within the OS user's store. Diagnostics redact device tokens.

The focused native CI compiles the affected consumers and exercises a public fixture in isolated credential stores on Linux/macOS/Windows. Existing release artifacts remain bound to their earlier commits; they do not acquire this implementation retroactively. The CLI now refuses missing-account installation and no longer suggests logging out to bypass verification; this regression is under grouped validation. Complete permission flows, installable app bundles and OS UI/install acceptance remain open. No public desktop release is opened by this ADR.

Sources: [v4.2 adapter](https://github.com/open-source-cooperative/keyring-rs/blob/v4.2.0/src/v1.rs), [features](https://github.com/open-source-cooperative/keyring-rs/blob/v4.2.0/Cargo.toml), [typed errors](https://github.com/open-source-cooperative/keyring-core/blob/v1.0.1/src/error.rs).
