# Desktop credential migration — step 8.5

Status: implementation specification; **not implemented or validated**. Authority: RULES.md §§4/6/7 and MASTER_PROMPT.md account/distribution requirements. No new product tool, service, dependency or paid signature is authorized by this document.

## Existing behavior and affected callers

`crates/kortex-pkg/src/account.rs` stores the full `Credentials` value, including the device token, in `Home::credentials_path()` (`credentials.json`). Unix mode 0600 does not meet the system keychain requirement. `load_credentials()` currently returns `Option`, conflating missing, unreadable and invalid data. `clear_credentials()` silently ignores deletion errors. The derived Debug implementation exposes the token if the value is formatted.

Consumers include CLI login/logout/whoami and package commands, plus the native Store/package library. Shared Rust session clients for browser cookies remain distinct; browser cookies must never be copied to this native store. The 38 tool engines do not need credential access.

## Implementation sequence

- [ ] **8.5.1 — Platform contract.** Add an explicit secret-store interface behind `kortex-platform`, with read/write/delete operations returning typed missing/locked/unavailable/access-denied/corrupt/backend errors. Tests inject an in-memory implementation explicitly; production never chooses it because of a temporary path or environment heuristic.
- [ ] **8.5.2 — OS backends.** Implement macOS Keychain, Windows Credential Manager and Linux Secret Service using reviewed SDK/library bindings. Select and justify any essential dependency in `docs/deps.md` before installation. No shell argument may contain a token. No plaintext, localStorage or unencrypted-file fallback is allowed when a backend is unavailable. A locked/missing Linux secret service remains an actionable error.
- [ ] **8.5.3 — Credential operations.** Store the credential record as a single protected item. Bind the item to the Kortex service and an explicit account/home namespace so isolated test homes never access production credentials. Redact tokens from Debug/errors/logs. Replace Option-only reads and infallible clears with Result-based calls; update every caller.
- [ ] **8.5.4 — Migration.** When the protected item is missing and a legacy record exists, strictly parse/validate the legacy record, write it to the protected backend, read it back and compare, then remove the legacy file. Backend/verification failure preserves the legacy record for recovery but does not use it as a runtime fallback or report login success. Failure to remove the plaintext file remains a visible incomplete migration. Never upload/export/sync the token.
- [ ] **8.5.5 — Login/logout recovery.** If device login creates a remote token but protected persistence fails, revoke that new token on a best-effort basis and return an error; preserve any previous valid credential. Offline logout must clear the protected local record and report remote revocation separately. A local deletion failure must never be described as a completed logout.
- [ ] **8.5.6 — Installation gate and release.** An absent/locked/invalid account must not silently authorize installation. Preserve offline catalogue/headless calculations and safe removal separately. Enforce the configured installation confirmation, including expired-session and wrong-password errors. This also addresses the anonymous-install gap tracked in 10.4; it does not close full bundle/OS acceptance requirements.

## Grouped acceptance evidence

| Check | Required result | Current validation |
|---|---|---|
| Fresh login | Protected record present; no plaintext token file | Not performed |
| Existing credential migration | Verified protected item; legacy file removed only after successful persistence | Not performed |
| Locked/unavailable backend | Actionable failure; no plaintext fallback, install or false login success | Not performed |
| Failed write/read-back/delete | No false success; previous account remains recoverable | Not performed |
| Failed new-token persistence | New remote token revocation attempted; error returned | Not performed |
| Offline logout / failed local clear | Local and remote outcomes distinguished; failed clear remains an error | Not performed |
| Isolated test store | No access to the user's real keychain entries | Not performed |
| Privacy | Debug/errors/exports contain no token/auth key/password | Not performed |
| macOS / Windows / Linux | Each changed native consumer compiled and the OS backend exercised | Not performed |
| Browser boundary | Browser build excludes native secret backend; HttpOnly session handling retained | Not performed |

Run the targeted failure/migration cases as one group, then the repository gate before commit. Compile changed native consumers for the affected OS targets and the shared app web target to prove feature isolation. Rebuild/reuse the 38 tool artifacts only according to actual compiled inputs and valid receipts; never relabel an old binary as a new-source build. Keep desktop public distribution closed until 8.5, 10.4, 10.6, 10.7 and 10.9 have their own required evidence.
