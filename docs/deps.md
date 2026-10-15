# Allowed dependencies
| Crate / package | Where | Why | Alternatives | Weight |
|---|---|---|---|---|
| `toml` 0.9 | xtask | parse contracts & tokens | hand parser (fragile) | build-time only |
| `dioxus` 0.7.x | ui crates, tools, apps | web + desktop UI from one source (ADR 0001) | Leptos, Tauri | ~260 KB gzip hello world |
| `serde` 1 | kortex-contract | (dé)sérialisation du contrat | manuelle | build-time + petit |
| `serde_json` 1 | xtask | écriture du registre JSON | — | build-time only |
