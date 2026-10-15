# 0001 — Dioxus 0.7 pour web et desktop depuis une seule source
**Statut** : accepté (2026-10-15)
**Contexte** : un moteur Rust + une UI pour site (wasm) et appli bureau. Spike (a) validé.
**Décision** : Dioxus 0.7.x (stable), features `web` / `desktop`, une CSS générée (`kortex.css`) partagée avec Astro. Profil release : opt-level z, lto, codegen-units 1, panic abort, **sans** `strip` (dx s'en charge). Rust edition 2024.
**Alternatives** : Leptos/Yew (pas de desktop natif unifié), Tauri (webview + JS, deux mondes).
**Conséquences** : builds lents à froid (cache obligatoire en CI) ; Dioxus 0.8 attendu en stable plus tard, migration par ADR.
