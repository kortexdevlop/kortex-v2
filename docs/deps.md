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
