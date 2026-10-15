# Progress
_Last update: 2026-10-15 — session 1_

## Current phase
Phase 0 — Cadrage, spikes, nettoyage (≈ 10 %)

## Done this session
- Master prompt copié dans `docs/MASTER_PROMPT.md`; `CLAUDE.md`/`AGENTS.md` créés.
- Outils installés sans Homebrew : gh 2.102, wrangler 4.151, wasm-opt 132 (binaryen), cible rustup wasm32-unknown-unknown. `dx` + `wasm-bindgen-cli` : `cargo install` en cours.
- Anciens dossiers du Bureau (`Kortex OS`, `kortex-main`, `kortex-pub`, `kortex-ad`, `KortexDevice`) : non touchés, à ne pas toucher.

## Decisions (user, session 1)
- Pas de Homebrew.
- Dossiers du Bureau : NE PAS TOUCHER.
- GitHub : créer un NOUVEAU dépôt privé ; ne pas supprimer l'ancien.
- Cloudflare : tout nettoyer (retour à l'état d'achat du domaine) — après inventaire + confirmation finale de la liste.
- Langues : FR + EN dès le lancement. E-mail : pas de vérification pour l'instant.
- Crédits IA : affichés en % restant, calculé d'après les neurons consommés (pas en nombre de requêtes) ; limite par utilisateur généreuse mais raisonnable.

## Next action (exact)
- `gh auth login` et jeton API Cloudflare (par l'utilisateur) → inventaire Cloudflare, puis confirmation de la liste de suppression.
- Créer le nouveau dépôt GitHub privé + dépôt public `kortex-releases`.
- Lire les docs à jour (Dioxus, Astro, quotas Cloudflare), spikes (a)–(f), ADR fondateurs, 4 logos + 2 systèmes de nommage.

## Blockers / waiting for user
- `gh auth login` (interactif, par l'utilisateur) et jeton API Cloudflare.

## Hypotheses in use
- Passkeys + mot de passe PBKDF2, sans vérification d'e-mail au départ.

## Catalogue stats
stable: 0 · beta: 0 · draft: 0
