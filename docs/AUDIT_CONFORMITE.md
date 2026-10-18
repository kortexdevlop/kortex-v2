# Audit de conformité et plan restant — 17 octobre 2026

Référence : MASTER_PROMPT.md + RULES.md. Complète PLAN.md (qui reste le suivi opérationnel étape par étape). Ce fichier est une synthèse ; aucune étape n'y est validée sans preuve dans PLAN.md.

## A. Ce qui respecte les règles (constaté)

- `cargo xtask check` : OK (38 outils, 3 apps).
- Catalogue gelé à 38 outils, tous `beta` ; aucun outil ajouté.
- Dépôt GitHub privé, `kortex-releases` public ; `LICENSE` propriétaire ; CLAUDE.md = AGENTS.md (identiques).
- Aucun `unwrap()`/`panic!` dans `tools/*/src` et `apps/*/src`.
- Outils dans `tools/<univers>/<nom>/`, apps dans `apps/<nom>/` (Rust/Dioxus, `app.toml`) ; plus de prototypes Astro/TS Desk/Hub.
- Tokens → `design/dist/kortex.css` ; aucune couleur en dur dans les outils (une seule hors-token à vérifier, voir B).
- Workers TS (`services/api`, `shared`), CI Linux avec `concurrency`, release desktop par tag `release/*` ou manuel.
- ADR 0001–0022, `docs/deps.md`, `brand/*.toml` présents.

## B. Violations et écarts de structure (à traiter)

| # | Écart | Règle / section | Gravité |
|---|---|---|---|
| V1 | ~~Polices non auto-hébergées~~ **Inter Variable intégrée (lot 4)** ; reste la monospace (repli système) et le contrôle de la CSP publiée | §4.3, §8.5 | Partiel |
| V2 | **Jetons desktop en clair** (`credentials.json` 0600) en prod ; correctif trousseau non publié | §9.6, RULES 6.1 | Haute |
| V3 | `--engine` JSON-RPC (lot 3) **et** `engine.wasm` headless pour les 38 outils (lot 7, exécutés en Worker réel pour 4 cas) ; reste leur diffusion par l'hôte `run` et la CI | §7.1, §7.3 | Réalisé (non publié) |
| V4 | Exécuteur natif sur thread avec annulation, délai et panique contenue **réalisé** (lot 3) ; recette en fenêtre native non faite | §6.3, RULES 4.4 | Partiel |
| V5 | ~~`ui = "custom"` non branché~~ **Capacité ajoutée (lot 2)** : aucun des 38 outils n'était en `custom` ; mécanisme prouvé web (rendu) et desktop (compilé, lancé) sur un outil jetable. Reste : rendu desktop inspecté, 7.6 (UI QR) | §6.4 | Partiel |
| V6 | Crate `kortex-io` **créée avec un usage réel** (validation par signature, lot 5) ; `kortex-media` et `kortex-ai` restent à créer quand un usage existe | §5.2 | Partiel |
| V7 | xtask : `new-app`, `dev`, `ci-budget` **ajoutés (lot A)** ; restent `og-images` et `deploy` | §13, §11 | Partiel |
| V8 | **Workers prévus absents** : `ai-gateway`, `proxy`, `oauth-broker` (l'IA est dans `api`, ADR 0006 — compatible mais écart de structure à acter) | §8.7, §10 | Basse/Moyenne |
| V9 | ~~`services/{desk,hub,run,site,store}` ne contiennent qu'un `wrangler.jsonc`~~ **Non-violation** : Workers Static Assets de §8.7 ; documenté dans ARCHITECTURE.md | §8.7 | Résolu |
| V10 | e2e : `tests/e2e/smoke.mjs` + `playwright` 1.63.0 ajoutés (lot 2), exécutés en local ; **CI non encore exécutée** | §11 | Partiel |
| V11 | ~~`web/shell/` absent~~ **Gabarit HTML partagé créé (lot 6)** : `dx` l’applique (titre, `lang`, `noindex`, `noscript`) ; `check` impose le même `index.html` et un titre `Dioxus.toml` égal au nom du contrat. Corrige aussi le titre « dioxus \| ⛺ » de Store | §5 | Résolu |
| V12 | ~~ADR 0012 et 0013 manquants~~ **Non-violation** : retrait volontaire, documenté dans docs/adr/README.md | §0.3 | Résolu |
| V13 | ~~`#000000` en dur~~ **Non-violation** : valeur par défaut d'un `<input type=color>` (donnée, RULES 5.3) | RULES 5.3 | Résolu |
| V14 | **Kortex Seek réalisé** (lot 8, niveau 1, web + desktop compilés, recette web) ; **Flow** en cours (lot 9) ; **Mind** non réalisé : son cœur est l'IA, désactivée par RULES 8.4 | §9 | Partiel |
| V15 | Empreintes de reçus **précises** (lot 6, mesurées) ; `xtask changed` et job `web-changed` exécutés en CI (runs 39070786316 et suivants). Gain de minutes réel à mesurer sur une PR ciblée | §11, RULES 7.2 | Réalisé |
| V16 | **Pas de preview / rollback reproductible** ; Turnstile désactivé ; vérif e-mail absente ; mentions légales en brouillon (adresse postale manquante) | §10, §2.15 | Moyenne |
| V17 | `credentials.yml` : déclenché seulement sur chemins précis ; mais plafond de 1830 min (macOS ×10) — à surveiller avec `xtask ci-budget` | §11 | Basse |

Après relecture du code (lot A), V9, V12 et V13 sont reclassés en non-violations.

Aucune violation grave de périmètre (budget, dépôt privé, 38 outils, Rust/Dioxus pour apps) n'a été trouvée. Les écarts sont des fonctions incomplètes, pas des contournements.

## C. Déjà validé à 100 % (« Oui » dans PLAN.md, périmètre précisé)

Dans leur périmètre exact seulement : 1.1, 1.2, 1.4, 1.5, 1.7, 1.8, 1.9 · 2.1, 2.2, 2.3, 2.4 · 3.1, 3.2, 3.5 · 4.1, 4.2, 4.9 · 5.1 · 6.1, 6.2 · 7.1, 7.2, 7.3, 7.8 · 8.1, 8.2, 8.3, 8.9 · 10.1 · 11.1, 11.2 · 13.1 · 16.1, 16.2 · 17.1, 17.3, 17.6, 17.8.

Aucun outil et aucune app n'est « validé complet » (web + desktop + recette).

## D. Plan restant, dans l'ordre

Chaque lot : annoncer N.M/fichiers/contrôles/publication avant modification, gate complet avant commit, mise à jour PLAN.md + PROGRESS.md.

**Lot 1 — Fermer le trousseau (V2, 8.5, 10.4)**
1. 8.5 : valider les preuves des 3 OS (workflow `credentials.yml`), publier le correctif.
2. 10.4 : transaction permissions/installation, gestion des échecs d'enregistrement serveur.

**Lot 2 — Pipeline et garde-fous (V5, V7, V10, V15, V17)**
3. 4.10 : brancher `ui.rs` + `use_tool` quand `ui = "custom"` (web + desktop).
4. 2.5 / 2.6 : `xtask new-app`, `xtask dev` ; 4.7, 4.8 : renforcer `check` (apps, crates, deps, semver).
5. 17.2 / 17.7 : détection ciblée par dépendances en PR ; restreindre `credentials.yml` ; `xtask ci-budget`.
6. Ajouter `tests/` e2e Playwright (3 pages clés), après entrée dans `deps.md`.

**Lot 3 — Socle exécution (V3, V4, V6)**
7. 3.4 / 3.6 : exécuteur natif (thread + annulation) ; 3.10 : `--engine` JSON-RPC/stdio ; 3.9 : `engine.wasm` headless.
8. 3.3 : plateforme restante (dialogues, notifications, préférences UI) ; 3.7 / 3.8 : bus complet et shell Standalone/Window/Node/Headless.
9. 3.11 / 3.12 : `kortex-io` (streaming, OPFS, ZIP, MIME) ; 3.13 : spike WebCodecs seulement si nécessaire.
10. 4.3–4.6 : types de ports complets, adaptateurs réels, JSON Schema pour l'IA.

**Lot 4 — Design et site (V1, V13, V11)**
11. 5.6 : fournir Inter Variable woff2 auto-hébergée ; 5.2 : composants manquants ; 5.3–5.5, 5.7 : modes, a11y, mouvement réduit.
12. 6.4–6.7, 6.10 : liens Installer/Desk réels, nav, SEO complet (HowTo/Breadcrumb/aliases), PWA, perfs mesurées.
13. Documenter : `ARCHITECTURE.md`, guides `add-app/universe/widget/connector` (2.8), ADR manquants (V12).

**Lot 5 — Compte, sync, distribution**
14. 8.4 / 9.1–9.4 : `kortex-auth` complet (formulaires, passkeys, jetons natifs) ; fusion de conflits ; E2E optionnel.
15. 8.6–8.8 : Turnstile, vérification e-mail (fournisseur gratuit), mesure CPU.
16. 10.2, 10.5–10.9 : signatures, découverte des outils installés, paquets, raccourcis Windows réels, release publique `kortex-releases`.
17. 11.3 / 11.5 : lancement et paquets depuis `app.toml`, parcours web + desktop minimal.

**Lot 6 — Outils : recette des 38 (7.4–7.7)**
18. Compiler et recetter les 38 sur web + desktop ; fermer les 6 pilotes (HEIC, QR Wi-Fi/vCard, UI QR, taux, limites lourdes) ; matrice 18.2. Aucun passage `stable` sans preuves.

**Lot 7 — Apps manquantes, dans l'ordre des dépendances**
19. Seek (12.x) → finaliser Desk (13.3–13.10) → `kortex-ai` + quotas réels (9.5–9.7) → Mind (14.x) → Flow + connecteurs + proxy + OAuth (15.x) → finaliser Hub (16.3–16.7).
20. Chaque app : preuves web **et** desktop avant « achevée ».

**Lot 8 — Finition et lancement (18.x)**
21. Sécurité/CSP/dépendances, Safari iOS/Android/Firefox/3 OS, légal (adresse postale), perfs Lighthouse, monitoring agrégé, preview/rollback, déclaration de fin uniquement avec toutes les preuves.

## E. Prochaine action

Le projet est en pause à la demande de l'utilisateur (PLAN.md). Reprise = lot 1, avec annonce N.M exacte avant toute modification.
