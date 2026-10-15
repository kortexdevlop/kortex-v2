# Spikes Phase 0 (résultats)

Versions vérifiées le 2026-10-15 : Dioxus 0.7.10 (stable ; 0.8 en alpha), dx 0.7.10, Astro 7.4.0, wrangler 4.151.0, Rust 1.99.0.

## (a) Composant Dioxus web + desktop avec une CSS partagée — OK
- Même `main.rs` : `dx build --release --platform web` et `--platform desktop --features desktop`.
- Feuille `kortex.css` chargée via `asset!()` + `document::Stylesheet` sur les deux cibles.
- Piège : `strip = true` dans `[profile.release]` casse `wasm-bindgen` (section `__wasm_bindgen_unstable` supprimée). Ne pas l'utiliser, dx strip lui-même. Installer `rustup component add llvm-tools`.
- Build desktop release à froid : ~4 min 20 ; web : ~5 min à froid, 5 s en incrémental.

## (d) Taille wasm — OK
- Hello world Dioxus web (opt-level z, lto, wasm-opt -Oz via dx) : 737 Ko brut, 261 Ko gzip. Budget UI ≤ 400 Ko brotli tenable pour une UI simple ; à re-mesurer avec `kortex-ui` complet.
- .app macOS (x86_64) : 4,9 Mo.

## Quotas gratuits Cloudflare (doc officielle)
- Workers AI : 10 000 neurons/jour, reset 00:00 UTC, erreur au-delà (pas de dépassement sur plan gratuit). Llama 3.2 3B : 4 625 neurons/M tokens en entrée, 30 475/M en sortie ; Llama 3.1 8B fp8-fast : 4 119 / 34 868 ; embeddings bge-m3 : 1 075/M.
  → ordre de grandeur : ~300 réponses de 1 000 tokens/jour pour tout le compte en 3B. Très serré : crédits affichés en % par utilisateur.
- Workers Free : 100 000 req/jour, 10 ms CPU/req, 50 sous-requêtes.
- Static Assets : 20 000 fichiers/version, 25 Mo/fichier.

## À faire
(b) Stage iframe + postMessage + View Transition · (c) moteur en Web Worker · (e) build desktop 3 OS en CI · (f) WebCodecs depuis Rust — nécessitent navigateur / CI ; planifiés après le socle Phase 1.
