# Progress
_Last update: 2026-10-15 — session 1_

## Current phase
Phase 0 — Cadrage, spikes, nettoyage (≈ 98 %) — Phase 1 quasi finie, Phase 2 démarrée, premier outil pilote

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
- Crates `kortex-runtime` (Cx, ToolError, CancelToken, FileData, Table, block_on), `kortex-platform` (traits Storage/Clipboard + MemoryPlatform), `kortex-i18n` (Catalog), `kortex-macros` (`contract!` génère Inputs/Params/Outputs/validate, `#[engine]` contrôle la signature), façade `kortex`. Template d'outil branché sur la macro et validé de bout en bout (outil temporaire, supprimé).
- Crate `kortex-ui` : Button, Card, TextField, Switch, Progress, DropZone, ModeSwitch, `AutoTool` (UI générée du contrat, params filtrés par mode Simple/Medium/Pro), CSS des composants dans `components.css`; 5 tests (dont rendu SSR). `cargo xtask stats`.
- Site Astro 7.4 (`web/site`) : layout (en-tête verre dépoli, thème auto clair/sombre, hreflang, JSON-LD), accueil, catalogue, page d'outil/variantes générées depuis le registre (`sync.mjs`), galerie `/design` (noindex). Preuve : 1 outil = 6 pages sans toucher au site.
- Outil pilote #5 `secure.password-generator` (beta) : moteur + 7 tests, FR/EN, 2 variantes SEO. `Cx::fill_random` ajouté (getrandom).
- Aperçu local : `.claude/launch.json` (serveur `site`).
- Entrée dynamique générée par la macro (`Inputs/Params::from_map`, `Outputs::into_map`, `run_map`) + `Value`/`ValueMap` (base de Flow/Mind/CLI).
- `AutoTool` exécute l'outil (async), affiche résultat + Copier + erreurs traduites (`Runner`).
- `cargo xtask build <id>` : génère un wrapper Dioxus, `dx build`, publie `dist/run/<id>/`. Password generator : wasm 1,2 Mo brut, testé dans le navigateur (aléa WebCrypto OK).
- Stage (fenêtre flottante, View Transition, iframe cross-origin, `?open=1`, préchargement au survol, Échap, plein écran) testé. Config locale : serveurs `site` (4321) et `run` (autoPort) dans `.claude/launch.json`; `PUBLIC_RUN_ORIGIN` pour le site.
- CI : job `site` ajouté.
- Site : sitemap.xml (hreflang, brouillons exclus), robots.txt, manifeste PWA + icône + service worker (cache hors-ligne), pages légales FR/EN (confidentialité et conditions rédigées ; mentions légales en brouillon noindex).
- Taille wasm password-generator : 1184 Ko brut, 412 Ko gzip, 338 Ko brotli (budget 400 Ko brotli OK, marge faible).
- DropZone fonctionnelle (glisser-déposer + sélecteur), fichiers lus en octets, sorties fichier téléchargeables (base64+Blob), `Select` pour les enums, presets de variantes appliqués (`?variant=`), sorties traduisibles via `[outputs]`/`[values]` des locales (valeur vide = masquée).
- Outil pilote #3 `secure.hash-calculator` (beta) : MD5/SHA-1/SHA-256/SHA-512, texte ou fichier, comparaison ; 5 tests (vecteurs de référence), 2 variantes SEO ; testé dans le navigateur. wasm 1239 Ko brut.
- Mentions légales : nom + région renseignés (page toujours brouillon : adresse et e-mail manquants).
- Outil pilote #4 `qr.generator` (beta) : PNG (palette 1 bit, encodeur maison validé par décodage `png`) ou SVG, couleurs, correction d'erreur, marge ; 4 tests ; aperçu image dans AutoTool ; testé dans le navigateur. TODO : champ couleur natif (`input type=color`) pour les params `color`, variantes Wi-Fi/vCard (templates de contenu).
- Outil pilote #2 `pdf.merge` (beta) : fusion dans l'ordre choisi, héritage MediaBox/Resources conservé, option compression, refus PDF chiffré/invalide ; 4 tests ; liste de fichiers avec monter/descendre/retirer ; testé dans le navigateur (2 PDF, ordre inversé, téléchargement OK).
- BUG corrigé : `AutoTool` ne transmettait pas les fichiers choisis au moteur (remplacement automatique raté en silence). Maintenant `collect_inputs` testé unitairement. Leçon : après un `cargo fmt`, toujours vérifier qu'un remplacement scripté a bien été appliqué.
- Tailles brotli des bundles : password 330 Ko, hash 344 Ko, qr 343 Ko, **pdf.merge 536 Ko (> budget 400 Ko)**. Base Dioxus+runtime ≈ 330 Ko. Pistes : moteur lourd chargé à la demande / en Web Worker séparé, `wasm-opt` plus agressif, budget par outil déclaré dans le contrat + vérifié par `xtask check` (§12.6 pas encore implémenté).
- Outil pilote #1 `image.convert` (beta) : PNG/JPG/WebP(lossless)/BMP/GIF en lot, format détecté par signature, transparence aplatie, garde-fou 20 000 px, 6 variantes SEO (png↔jpg, webp→png…) ; 5 tests ; testé navigateur (png→jpg OK). brotli 508 Ko (> budget). HEIC non supporté (à faire).

## Decisions (user, session 1)
- Pas de Homebrew.
- Dossiers du Bureau : NE PAS TOUCHER.
- GitHub : créer un NOUVEAU dépôt privé ; ne pas supprimer l'ancien.
- Cloudflare : tout nettoyer (retour à l'état d'achat du domaine) — après inventaire + confirmation finale de la liste.
- Langues : FR + EN dès le lancement. E-mail : pas de vérification pour l'instant.
- Crédits IA : affichés en % restant, calculé d'après les neurons consommés (pas en nombre de requêtes) ; limite par utilisateur généreuse mais raisonnable.

## Next action (exact)
- Pilote restant : calc salaire brut/net (UI auto pure ; taux officiels à vérifier, résultat étiqueté « estimation »). Puis check de budget wasm, champ couleur natif, vCard/Wi-Fi QR. Dépôt de fichier sur la page → Stage (bus postMessage), thème/mode transmis à l'iframe, taille wasm (budget 400 Ko brotli), moteur en Web Worker (spike c), PWA, sitemap, OG, pages légales, accueil animée, ⌘K, déploiement Cloudflare (Workers Static Assets, wrangler).
- Phase 1 reste : impls plateforme web/desktop, `kortex-io`, `kortex-bus`, xtask `icons`/`dev`/`build`, galerie `/_design` (dans web/site, Phase 2), xtask `icons`/`dev`/`build`/`stats`, galerie `/_design`, vérifier la CI sur GitHub.
- Spikes restants (b) Stage iframe, (c) Web Worker, (f) WebCodecs, (e) CI 3 OS ; ADR contrat/auth/codecs.
- Puis Phase 1 (monorepo, tokens, kortex-ui, xtask).

## Blockers / waiting for user
- Mentions légales : reste l'adresse postale (ou recours à l'hébergeur pour les particuliers) et l'e-mail de contact (ex. `contact@kortex.site` via Email Routing déjà configuré). Nom et région renseignés.
- Déploiement public : NE PAS déployer pour l'instant (décision user).
- Révoquer/recréer le jeton Cloudflare ; rien d'autre.

## Hypotheses in use
- Passkeys + mot de passe PBKDF2, sans vérification d'e-mail au départ.

## Catalogue stats
stable: 0 · beta: 0 · draft: 0
