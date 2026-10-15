# Progress
_Last update: 2026-10-15 — session 1_

## Current phase
Phase 0 — Cadrage, spikes, nettoyage (≈ 20 %)

## Done this session
- Master prompt copié dans `docs/MASTER_PROMPT.md`; `CLAUDE.md`/`AGENTS.md` créés.
- Outils installés sans Homebrew : gh 2.102, wrangler 4.151, wasm-opt 132 (binaryen), cible rustup wasm32-unknown-unknown. `dx` + `wasm-bindgen-cli` : `cargo install` en cours.
- Anciens dossiers du Bureau (`Kortex OS`, `kortex-main`, `kortex-pub`, `kortex-ad`, `KortexDevice`) : non touchés, à ne pas toucher.

- Cloudflare nettoyé (zone kortex.site) : 2 Workers, 15 projets Pages, D1 `kortex-accounts`, bucket R2 `kortex-dl` (98 objets) et 18 DNS (CNAME/AAAA) supprimés. Gardés : DNS e-mail (3 MX Email Routing, SPF, DKIM `cf2024-1`, Resend `send`/`rsend`/`resend._domainkey`, TXT Google verification) — à décider. Zone `microsoft-update.download` jamais touchée.
- Archive locale (hors dépôt) : `~/kortex-cloudflare-archive/` (SQL D1, DNS, Workers, Pages, clés R2).
- Jeton Cloudflare dans `~/.kortex-cf-token` (hors dépôt) — À RÉVOQUER puis recréer un jeton limité (il a été collé dans le chat).
- GitHub : `gh` connecté (kortexdevlop). Dépôt privé `kortexdevlop/kortex-v2` créé et poussé (l'ancien `kortex` intact). Dépôt public `kortex-releases` : pas encore créé (accord requis).

## Decisions (user, session 1)
- Pas de Homebrew.
- Dossiers du Bureau : NE PAS TOUCHER.
- GitHub : créer un NOUVEAU dépôt privé ; ne pas supprimer l'ancien.
- Cloudflare : tout nettoyer (retour à l'état d'achat du domaine) — après inventaire + confirmation finale de la liste.
- Langues : FR + EN dès le lancement. E-mail : pas de vérification pour l'instant.
- Crédits IA : affichés en % restant, calculé d'après les neurons consommés (pas en nombre de requêtes) ; limite par utilisateur généreuse mais raisonnable.

## Next action (exact)
- Attendre fin de `cargo install dioxus-cli wasm-bindgen-cli`.
- Décider du sort des DNS e-mail ; créer `kortex-releases` (public) après accord.
- Lire les docs à jour (Dioxus, Astro, quotas Cloudflare), spikes (a)–(f), ADR fondateurs, 4 logos + 2 systèmes de nommage.

## Blockers / waiting for user
- Révoquer/recréer le jeton Cloudflare ; décision DNS e-mail ; accord pour créer le dépôt public `kortex-releases`.

## Hypotheses in use
- Passkeys + mot de passe PBKDF2, sans vérification d'e-mail au départ.

## Catalogue stats
stable: 0 · beta: 0 · draft: 0
