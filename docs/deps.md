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
