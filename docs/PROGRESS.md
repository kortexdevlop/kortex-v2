# Progress
_Last update: 2026-10-15 — session 1_

## Current phase
Phase 0 — Cadrage, spikes, nettoyage (≈ 55 %) — Phase 1 en cours

## Done this session
- Master prompt copié dans `docs/MASTER_PROMPT.md`; `CLAUDE.md`/`AGENTS.md` créés.
- Outils installés sans Homebrew : gh 2.102, wrangler 4.151, wasm-opt 132 (binaryen), cible rustup wasm32-unknown-unknown. `dx` + `wasm-bindgen-cli` : `cargo install` en cours.
- Anciens dossiers du Bureau (`Kortex OS`, `kortex-main`, `kortex-pub`, `kortex-ad`, `KortexDevice`) : non touchés, à ne pas toucher.

- Cloudflare nettoyé (zone kortex.site) : 2 Workers, 15 projets Pages, D1 `kortex-accounts`, bucket R2 `kortex-dl` (98 objets) et 18 DNS (CNAME/AAAA) supprimés. Gardés : DNS e-mail (3 MX Email Routing, SPF, DKIM `cf2024-1`, Resend `send`/`rsend`/`resend._domainkey`, TXT Google verification) — à décider. Zone `microsoft-update.download` jamais touchée.
- Archive locale (hors dépôt) : `~/kortex-cloudflare-archive/` (SQL D1, DNS, Workers, Pages, clés R2).
- Jeton Cloudflare dans `~/.kortex-cf-token` (hors dépôt) — À RÉVOQUER puis recréer un jeton limité (il a été collé dans le chat).
- GitHub : `gh` connecté (kortexdevlop). Dépôt privé `kortexdevlop/kortex-v2` créé et poussé (l'ancien `kortex` intact). Dépôt public `kortex-releases` : pas encore créé (accord requis).

- Dépôt public `kortexdevlop/kortex-releases` créé (vide). DNS e-mail conservés (décision user).
- Docs lues : versions Dioxus 0.7.10 / Astro 7.4.0 / wrangler 4.151 ; quotas gratuits Cloudflare notés dans `docs/spikes.md`.
- Spike (a)+(d) OK (web + desktop, wasm 261 Ko gzip hello world). ADR 0001–0003 écrits.
- Propositions logo (4) + nommage (2) : `brand/proposals/logo-naming.html`.

- Choix user : logo B, nommage option 1 (Kortex Mind/Seek/Desk/Flow/Hub). `brand/names.toml`, `universes.toml` (17), `domains.toml`, logo SVG écrits.
- Phase 1 : workspace Cargo (membre `xtask`), toolchain figée 1.99.0, tokens `design/tokens/*.toml` + `design/components.css` → `cargo xtask tokens` génère `design/dist/kortex.css` (clair/sombre).
- Crate `kortex-contract` (schéma tool.toml, types de ports + conversions, validation FR) : 7 tests OK.
- xtask : `tokens`, `registry`, `check [id]`, `new-tool` ; `tools/_template` ; `docs/guides/add-tool.md` ; CI Linux (`.github/workflows/ci.yml`). Test négatif de `check` OK.

## Decisions (user, session 1)
- Pas de Homebrew.
- Dossiers du Bureau : NE PAS TOUCHER.
- GitHub : créer un NOUVEAU dépôt privé ; ne pas supprimer l'ancien.
- Cloudflare : tout nettoyer (retour à l'état d'achat du domaine) — après inventaire + confirmation finale de la liste.
- Langues : FR + EN dès le lancement. E-mail : pas de vérification pour l'instant.
- Crédits IA : affichés en % restant, calculé d'après les neurons consommés (pas en nombre de requêtes) ; limite par utilisateur généreuse mais raisonnable.

## Next action (exact)
- Phase 1 suite : crates `kortex-macros` (`contract!`), `kortex-runtime`, `kortex-platform`, `kortex-i18n`, `kortex-ui` (composants + AutoTool), xtask `icons`/`dev`/`build`/`stats`, galerie `/_design`, vérifier la CI sur GitHub.
- Spikes restants (b) Stage iframe, (c) Web Worker, (f) WebCodecs, (e) CI 3 OS ; ADR contrat/auth/codecs.
- Puis Phase 1 (monorepo, tokens, kortex-ui, xtask).

## Blockers / waiting for user
- Révoquer/recréer le jeton Cloudflare ; rien d'autre.

## Hypotheses in use
- Passkeys + mot de passe PBKDF2, sans vérification d'e-mail au départ.

## Catalogue stats
stable: 0 · beta: 0 · draft: 0
