# Allowed dependencies
| Crate / package | Where | Why | Alternatives | Weight |
|---|---|---|---|---|
| `toml` 0.9 | xtask | parse contracts & tokens | hand parser (fragile) | build-time only |
| `dioxus` 0.7.x | ui crates, tools, apps | web + desktop UI from one source (ADR 0001) | Leptos, Tauri | ~260 KB gzip hello world |
| `serde` 1 | kortex-contract | (dé)sérialisation du contrat | manuelle | build-time + petit |
| `serde_json` 1 | xtask | écriture du registre JSON | — | build-time only |
| (aucune) `syn`/`quote` | kortex-macros | évitées volontairement : génération de code par texte depuis le contrat validé | syn+quote (compile plus lent) | — |
| `getrandom` 0.3 | kortex-runtime | CSPRNG de l'OS / WebCrypto (mots de passe, clés) | rand (plus lourd) | minime |
| `astro` 7.4.0 | web/site | site statique, 0 JS par défaut | Eleventy, Next | build-time |
| `dioxus-ssr` 0.7 | kortex-ui (dev) | tests de rendu HTML | navigateur headless (lent) | dev only |
| `md-5`, `sha1`, `sha2` 0.10 | tool secure.hash-calculator | RustCrypto audité, impensable à réécrire | blake3 seul (moins courant) | ~30 Ko wasm |
| `qrcodegen` 1.8 | tool qr.generator | encodeur QR de référence (Nayuki), sans dépendance | réécriture (Reed-Solomon, masques) | minime |
| `png` 0.17 | tool qr.generator (dev) | décodeur indépendant pour valider notre encodeur PNG | — | dev only |
| `lopdf` 0.45 (sans features par défaut) | tool pdf.merge | lecture/écriture PDF, irréaliste à réécrire | pdf-writer (écriture seule) | ~200 Ko brotli |
| `image` 0.25 (png, jpeg, webp, gif, bmp) | tool image.convert | décodeurs/encodeurs de référence | codecs individuels (plus de glue) | ~170 Ko brotli |

| `serde_json` 1 (preserve_order, arbitrary_precision) | tools/code/json-formatter | formatage JSON en gardant l'ordre et les nombres | manuelle | petit |
| `wasm-bindgen` 0.2.129 | docs/spikes/web-worker-demo | exposer le moteur Rust à un Web Worker pour le spike de Phase 0 | protocole wasm manuel (fragile) | spike uniquement, hors produit |
| `serde` 1 | kortex-runtime, kortex-platform, moteur généré | protocole typé des valeurs et erreurs entre UI et Worker | conversion manuelle (fragile) | déjà présent dans le dépôt |
| `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `serde-wasm-bindgen` | kortex-platform (web), moteur généré | Worker isolé, promesse annulable, buffers binaires transférables | JSON/base64 (copies et mémoire supplémentaires) | déjà transitifs via Dioxus ; aucun nouvel écosystème |

| `playwright` | scripts/tests (test environment only) | real Chromium Worker, transferable buffers and offline cache smoke test | manual browser recipe (not reproducible) | test-only; supplied by the local runtime, no production dependency |

| `kortex-date` (internal) | date/age/timestamp tools | bounded Gregorian calendar and UTC arithmetic shared by four engines | duplicating calendar logic or adding chrono/time | no external dependencies |

| `@simplewebauthn/server` 14.0.3 | services/api | mature WebAuthn verification: CBOR/COSE parsing, signed challenges, RP/origin checks, user verification and counters; essential cryptographic protocol code should not be reimplemented | hand-written FIDO verification (rejected), full auth platform (too heavy) | API-only, bundled with Wrangler, WebCrypto runtime |
| `@simplewebauthn/browser` 14.0.0 | web/site account | matching JSON/binary conversion and native credential API handling for registration/authentication | manual conversions (fragile browser compatibility) | account bundle only; no third-party hosted script |

| `ureq` 3 (rustls, gzip) | kortex-platform native backend | existing native HTTP transport moved out of package/auth clients; bounded responses, TLS and timeout handling | OS-specific HTTP implementations | native only; absent from browser dependency graph |
| `rpassword` 7 | kortex-platform native terminal backend | existing hidden terminal password input moved behind platform boundary | manual terminal-mode handling (unsafe portability) | native only |

| `getrandom` 0.3, `js-sys` 0.3 | kortex-platform | move runtime entropy/clock OS bindings behind platform, preserving injected test sources | callbacks required at every engine call (fragile) | existing dependencies moved, no new ecosystem |
| `dioxus` 0.7.10 (document + file event types), `serde_json` 1 | kortex-platform optional document feature | shared browser/WebView clipboard, download and host integration; safe JSON argument serialization | raw JavaScript evaluation in UI/generators (violates boundary) | existing workspace dependencies, feature disabled for headless engines |
| `objc2-app-kit` 0.3.2 (`NSPasteboard`), `objc2-foundation` 0.3.2 (`NSString`) | kortex-platform macOS clipboard | direct Rust access to AppKit pasteboards, including an isolated named test board; existing Dioxus desktop dependencies reused | subprocess clipboard utilities or handwritten Objective-C FFI | macOS only, existing cached versions, no new service or executable |
| `serde` / `serde_json`, `toml`, Dioxus 0.7.10 and existing kortex-* libraries | Hub, Desk, shell and typed handshake | existing serializers, contract catalogue, shared UI and renderer features reused for both app targets | duplicate application logic in TypeScript (rejected) | no new external direct library; web/desktop renderer features remain explicit |
| Existing serde/serde_json/toml, Dioxus document bridge and kortex-platform | kortex-auth, kortex-sync and shared account panel | typed cookie-session client and explicit metadata backup through the existing API; bounded platform fetch, no credential storage or new network library | duplicating auth/sync business logic in TypeScript (rejected) | existing external dependencies only; two internal shared libraries |

### Store common application (2026-10-17)
Store reuses the existing internal `kortex-shell` and existing workspace Serde. Native package access is target-gated; no package manager, system HTTP backend or native filesystem is linked into its browser build. No new external dependency is introduced.

### Static deployment CI (2026-10-17)
The deployment workflow reuses the existing pinned Wrangler 4.151.0 through npm exec on its temporary GitHub runner only. The receipt verifier uses Node.js built-ins and the existing Rust compiler/std library; no product dependency or additional catalogue tool is introduced. Cloudflare credentials remain GitHub Actions secrets, never artifacts.

### Native credential store (step 8.5, 2026-10-17)
`keyring =4.2.0`, native-only optional `kortex-platform/secret-store` feature: required OS-protected storage for desktop device credentials. Its v1 backend selects macOS Keychain, Windows Credential Manager and Linux Secret Service (zbus/Rust crypto), without a plaintext fallback or extra CLI executable. Existing pkg/CLI opt in; browser builds exclude it. Reimplementing secure OS protocols/FFI is rejected. Sources: https://github.com/open-source-cooperative/keyring-rs/blob/v4.2.0/Cargo.toml and src/v1.rs. License MIT/Apache-2.0 is compatible with the proprietary product; no paid service.
Linux Secret Service acceptance uses the free OS packages `gnome-keyring` and `dbus-x11` on an ephemeral GitHub runner only; these are test prerequisites, not bundled executables or a new tool in the catalogue. The test daemon stores only a public fixture in an isolated temporary data directory. No production credential is used.

### End-to-end smoke test (step 18.2, 2026-10-17)
`playwright` 1.63.0 (published 2026-09-04), root `devDependency`, test-only: MASTER_PROMPT §11 requires a Playwright smoke on three key pages in CI, and the existing `scripts/tests/*.mjs` recipes expect it to be resolvable although it was only "supplied by the local runtime". It is pinned so CI and local runs use the same driver; nothing is bundled into the site, Workers or apps. `tests/e2e/smoke.mjs` serves the built `web/site/dist` from a local Node `http` server (no extra package), so it needs no network, account or production host. CI installs Chromium with `pnpm exec playwright install --with-deps chromium` on the ephemeral Linux runner; locally `CHROME_PATH` may point at an installed Chrome instead of downloading a browser. Alternatives rejected: manual browser recipes (not reproducible), Cypress/WebDriver (heavier, a second tool). Licence Apache-2.0, compatible with the proprietary product; no paid service. Install scripts stay disabled by the workspace `allowBuilds` policy.

### Contract JSON Schema (step 4.5, 2026-10-17)
`serde_json` 1 is now also a dependency of `kortex-contract` (already used by xtask, runtime and platform): it builds the JSON Schema tool definitions from contracts for future Mind/Seek hosts. Alternative rejected: hand-written schemas per tool (drift from contracts). Small, already in the lockfile, no new ecosystem.

### Native execution and `--engine` (steps 3.4/3.6/3.10, 2026-10-17)
`serde_json` 1 is also a native-only dependency of `kortex-runtime` (`cfg(not(wasm32))`, absent from browser bundles): it parses and writes the JSON-RPC lines of the desktop `--engine` mode. Threads, condition variables and standard streams use only `std` behind `kortex-platform::native` (`task`, `stdio`); no async runtime, no new crate. Alternative rejected: an async runtime such as tokio for one blocking worker thread (large, a second execution model).

### Self-hosted Inter Variable (step 5.6, 2026-10-18)
Font file only, no npm dependency kept: `inter-latin-wght-normal.woff2` from `@fontsource-variable/inter` 5.3.0 (published 2026-07-19, OFL-1.1), tarball sha512 checked against the registry, stored in `design/fonts/` with its licence. MASTER_PROMPT §4.3 requires self-hosted Inter (no Google Fonts). Embedded as a `data:` URI by `cargo xtask tokens` (about 63 KB of CSS per bundle) so every host and webview gets it without serving `/fonts`; CSP gains `font-src 'self' data:`. Alternatives rejected: per-host `/fonts` files (plumbing on five hosts and desktop), the Google Fonts CDN (forbidden), the full multi-script family (about 300 KB). No paid service.

### Kortex Flow (steps 15.1-15.4, 2026-10-18)
`apps/flow` reuses `serde`/`serde_json` (workflow files, JSON-RPC to tool engines), `toml` (contract defaults, already allowed) and the internal shared crates; no new external dependency. Child processes go through the new `kortex-platform::native::process` (std only). Alternatives rejected: an embedded scripting language or a graph library (large, a second execution model, unnecessary for typed ports).

### Shared crates for Mind (steps 5.2, 9.5, 14.x, 2026-10-18)
`kortex-search` (Seek's engine moved out of the app, now used by Mind to pick candidate tools), `kortex-workflow` (Flow's graph, validation, step-by-step execution and installed-binary runner, now used by Flow and Mind) and `kortex-ai` (gateway wire format, SSE parser, tool definitions from contracts, reference protocol, plan validation, per-attachment consent, credits). Internal crates, each with two real users; no new external dependency (`serde`, `serde_json`, `toml` are already allowed).

### Mind (steps 14.x, 2026-10-18)
`apps/mind` uses only crates already allowed (`serde`, `serde_json`, Dioxus) and the internal shared crates `kortex-ai`, `kortex-search`, `kortex-workflow`, `kortex-auth`; desktop credentials come from `kortex-pkg` (native-only). The AI provider is Cloudflare Workers AI through the existing `AI` binding (free plan, 10 000 neurons/day shared, capped by the gateway), model `@cf/google/gemma-4-26b-a4b-it` (ADR 0023). No new external package, no paid service.
| `serde_json` 1 (`preserve_order`) | kortex-ai | garder l'ordre des clés du JSON du modèle pour les colonnes d'un tableau | `BTreeMap` (ordre alphabétique, colonnes mélangées) | `indexmap` déjà présent dans le lockfile |
| `aes-gcm` 0.10, `pbkdf2` 0.12, `sha2` 0.10 | kortex-vault | chiffrement de bout en bout des données synchronisées (AES-256-GCM, clé dérivée d'une phrase de passe) | WebCrypto depuis du JavaScript (viole la frontière plateforme), chiffrement maison (à éviter) | crates RustCrypto auditées ; `pbkdf2` et `sha2` déjà utilisées par kortex-pkg, seule `aes-gcm` (et ses dépendances `aes`, `ghash`, `ctr`) est nouvelle |
