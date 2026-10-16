# Progress
_Last update: 2026-10-15 — session 1_

## Current phase
Phase 0 — Cadrage, spikes, nettoyage (≈ 100 % des pilotes) — Phase 1 quasi finie, Phase 2 démarrée, premier outil pilote

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

- `scripts/gate.sh` : garde-fou local strict (fmt, clippy -D warnings, tests, tokens, registry, check) à lancer AVANT chaque commit (`./scripts/gate.sh && git commit`). Leçon : ne jamais enchaîner via des pipes (`| tail`) qui masquent les échecs.
- Outil pilote #6 `calc.gross-net-salary` (beta) : brut↔net France secteur privé, taux salariaux 2026, mensuel/annuel, cadre (APEC), PAS ; 7 tests (cas 2500 € calculé à la main, inversion net→brut, marche du CET au plafond, annuel, erreurs) ; testé navigateur (282,75 € cotisations, 238,26 € CSG/CRDS, 1978,99 € net). 4 variantes SEO. 339 Ko brotli.
  - Sources vérifiées : PASS 2026 (4 005 €/mois), Agirc-Arrco T1 3,15 % / T2 8,64 % / CEG 0,86 % et 1,08 % / CET 0,14 %. NON reconfirmés par source officielle : vieillesse de base 6,90 % / 0,40 % et APEC 0,024 % → à revérifier (Urssaf) et chaque janvier. Résultat présenté comme estimation. Les sites secondaires consultés contenaient des erreurs (ex. « chômage salarié 2,40 % ») : ne pas s'y fier.
- AutoTool : sorties dans l'ordre du contrat, nombres affichés à 2 décimales.
- **6 pilotes terminés** (password, hash, qr, pdf.merge, image.convert, salary) : catalogue = 6 outils beta, ~24 pages SEO FR/EN.
- Accueil animée (logo qui se dessine, halo suivant le curseur, révélations `animation-timeline: view()`, grille des 17 univers avec compteurs, section vie privée) FR/EN.
- Kortex Seek niveau 1 (`web/site/src/lib/seek.ts`) : recherche locale hors-ligne (accents, fautes, préfixes, synonymes FR/EN, mots vides, variantes) ; palette ⌘K / Ctrl+K / « / » dans l'en-tête ; index `public/seek-index.json` généré par sync ; 9 tests `node --test` (+ CI). Testé navigateur.
- Décision user : on ne s'occupe plus du budget de taille wasm sauf cas inadmissible.
- `cargo xtask icons` : icônes composées par outil (squircle + dégradé OKLCH de l'univers + glyphe + ombre) en SVG, icônes de marque PNG 192/512, images Open Graph 1200×630 FR/EN par outil + accueil (resvg, polices système ; si `design/fonts/*.ttf` existe elles sont chargées). Site : balises OG/Twitter, apple-touch-icon, manifeste PWA PNG, icônes sur cartes et pages d'outil. `design/dist` est ignoré par git (régénéré par xtask/CI).
- TODO police : Inter auto-hébergée (téléchargement à valider avec l'utilisateur) → déposer dans `design/fonts/` pour des images OG et un site fidèles à la charte.
- Bus hôte↔outil (ADR 0004) : iframe cross-origin, `kortex:ready` / `kortex:init {theme}` / `kortex:files {files}`, origine et fenêtre source vérifiées des deux côtés. Glisser un fichier n'importe où sur la page d'un outil ouvre le Stage et lui transmet le fichier (testé : abc.txt reçu). Sélecteur de thème auto/clair/sombre dans l'en-tête, propagé à l'iframe (testé). `cargo xtask build all` reconstruit tous les outils.
- Mode Simple/Medium/Pro global : sélecteur dans l'en-tête (localStorage `k-mode`), passé à l'iframe (`?mode=` au chargement puis `kortex:init {mode}` en direct) ; `AutoTool` prop `host_mode`, réglable ensuite par outil. Testé (Pro → Simple en direct).
- **Phase 4 (backend) démarrée, local uniquement** : `services/api` (Worker TypeScript, D1) — inscription/connexion, sessions (cookie `HttpOnly` host-only, id stocké haché SHA-256, 30 j), déconnexion, `/v1/me`, suppression de compte (RGPD, cascade) et export JSON, réglages (≤16 Ko), sync versionnée (≤64 Ko, conflit 409 avec copie courante, 200 clés max), compteurs d'usage journaliers (90 j), limitation de débit D1, CSRF (Origin liste blanche ou en-tête `X-Kortex-Request`), CORS liste blanche, en-têtes de sécurité. 22 tests (SQLite `node:sqlite` en simulateur D1) + test de fumée réel sur `wrangler dev` (workerd + D1 local). Schéma `migrations/0001_init.sql`.
- **ADR 0005** : PBKDF2 100 000 côté Worker ≈ 60 ms (limite gratuite 10 ms CPU) → le mot de passe ne quitte pas l'appareil ; le client dérive `auth_key` (PBKDF2 600 000 it., ~310 ms navigateur, `services/shared/src/auth.ts`), le Worker n'en fait qu'un PBKDF2 léger (5 000). Politique de mot de passe côté client. `kdf: 1` pour migrations futures.
- `services/shared` (auth client, 3 tests). Gate et CI couvrent site, shared, api. `pnpm-workspace.yaml` : scripts d'installation tiers ignorés (esbuild, workerd).
- Reste backend : passkeys (WebAuthn), jetons d'appareil (CLI/desktop), Turnstile (nécessite des clés), pages connexion/inscription dans le site (utiliser `deriveAuthKey`), `ai-gateway` (crédits en %, budget global), mesure CPU réelle en production (impossible en local).
- **Passerelle IA** (`/v1/ai/credits`, `/v1/ai/chat`, ADR 0006) : crédits en % d'après les neurons réellement consommés ; seau 1 200 neurons (+400/3 h), budget global 9 000/jour (marge sous les 10 000 gratuits) ; réservation puis règlement sur usage réel, remboursement si échec, pause globale « IA en pause » jusqu'à minuit UTC ; consentement obligatoire ; aucun prompt/réponse stocké. 11 tests (faux modèle). Bug trouvé par les tests et corrigé : la 1re requête du jour contournait le plafond global (INSERT non gardé) ; vérifié sur D1 local réel. Non testable en local : l'appel réel au binding `AI` (nécessite le compte Cloudflare) → à valider au premier déploiement.
- 33 tests API au total.
- **Page Compte** (`/compte`, `/en/account`, noindex) : inscription/connexion (mot de passe → `deriveAuthKey` dans le navigateur, politique côté client), jauge de crédits IA en %, export JSON, suppression de compte (mot de passe re-demandé, cascade vérifiée sur D1 local), déconnexion. Testé en vrai navigateur contre `wrangler dev` (site 4321 ↔ API 8787, cookie inter-origines OK) : session conservée au rechargement, mauvais mot de passe = message générique, export, suppression refusée/acceptée. Variables de build : `PUBLIC_API_ORIGIN` (défaut `https://api.kortex.site`), `PUBLIC_RUN_ORIGIN`. Dérivation 600 000 it. : ~0,3 s en Node, ~1-2 s dans le navigateur de test.
- Lien « Compte » dans l'en-tête.
- **CLI `kortex`** (`crates/kortex-cli`, binaire `kortex`) : `list`, `search`, `info`, `run` (entrées `nom=valeur` ou `nom=@fichier`, sorties fichier écrites sans écraser ni sortir du dossier, `--json`), mêmes moteurs que le web (`run_map`). Liste des outils générée par `cargo xtask cli-gen` (vérifiée fraîche par `xtask check` : un outil ajouté sans `cli-gen` fait échouer le contrôle). 9 tests (dont fusion de PDF en ligne de commande). Base du mode headless pour Flow/Mind.
- Reste Phase 5 : `kortex install/update/remove/doctor/login` (nécessite le registre signé Ed25519 + dépôt `kortex-releases` + compte), Kortex Store (Dioxus desktop), scripts `install.sh/ps1`, paquets desktop par outil, CI de release (matrice 3 OS).
- **CI GitHub verte** (jobs rust, site, services) après correction du lockfile pnpm (importer `services/shared`) ; `allowBuilds` (pnpm 12) déclaré ; `pnpm install --frozen-lockfile` ajouté au gate pour détecter ce cas en local. Leçon : les 3 commits précédents avaient échoué en CI sans que le gate local le voie → le gate reproduit désormais l'installation gelée de la CI.
- **Phase 5 — installation et distribution (2026-10-16)** :
  - `crates/kortex-pkg` : index signé Ed25519, téléchargement, vérification (taille, SHA-256, signature), extraction par liste blanche, installation atomique, mise à jour, suppression, `doctor`, auto-mise à jour de la CLI, lanceurs (.desktop / .app / .cmd) ; 20 tests dont archives piégées, paquets altérés, serveur HTTP local, rollback. ADR 0007.
  - CLI : `available`, `install <id>…|--all`, `update [id|--all|--self]`, `remove`, `installed`, `doctor`, `open` ; 7 tests de commandes.
  - **Kortex Store** (`apps/store`, Dioxus desktop, id `apps.store`) : catalogue vérifié, recherche, filtres par univers, installer/mettre à jour/ouvrir/supprimer, « tout mettre à jour » ; 4 tests (logique + rendu).
  - xtask : `keygen`, `build-desktop`, `build-cli`, `build-app`, `package`, `release-index` (+ `cli-latest.txt`), `list-tools`. Clé de signature générée (privée hors dépôt `~/.kortex-signing-key`, publique `crates/kortex-pkg/release.pub`).
  - `installers/install.sh` et `install.ps1` ; le site les sert et sert l'index signé (`sync.mjs`).
  - **Test de bout en bout réel (macOS x86_64)** : binaires de release compilés → paquets → index signé → serveur local → `install.sh` installe la CLI → `kortex install` installe un outil → lanceur .app créé → l'appli de bureau et le Store démarrent ; paquet modifié refusé (taille), index modifié refusé (signature).
  - `.github/workflows/release.yml` (Linux, macOS arm64 [+ Intel en option], Windows ; ad hoc codesign ; job `publish` signe l'index ; publication vers `kortex-releases` si `RELEASES_TOKEN`). Guide `docs/guides/release.md`.
- **Compte lié aux installations (2026-10-16)** :
  - API : `POST /v1/auth/device` (jeton d'appareil `kdt_…`, stocké haché, révocable, plafond 20, même limite de tentatives que le login), `GET/DELETE /v1/devices`, `POST /v1/auth/confirm` (re-vérifie le mot de passe, limité), `POST/GET/DELETE /v1/installs` ; `Authorization: Bearer` accepté partout sauf suppression du compte (cookie de session obligatoire) ; export RGPD complété ; migration `0003_devices.sql` ; 43 tests API.
  - `kortex-pkg::account` : même dérivation de clé que le site (**vérifiée identique octet pour octet, 600 000 itérations incluses**), identifiants `~/.kortex/credentials.json` en 0600 (épinglés à l'API émettrice).
  - CLI : `login` (mot de passe saisi masqué ou `--password-stdin`), `logout` (révoque le jeton), `whoami`, confirmation du mot de passe avant `install` si le compte le demande (réglage `confirm_install`, défaut oui ; **échec fermé** si le serveur est injoignable ; désactivable depuis le compte), installation enregistrée sur le compte (échec non bloquant), `doctor` vérifie le compte ; 8 tests avec faux serveur d'API qui prouve que le mot de passe n'est jamais envoyé.
  - **Vérifié de bout en bout** : vraie CLI ↔ vrai Worker local (workerd + D1) : login, mauvais mot de passe bloque l'installation, bon mot de passe installe et enregistre, jeton ≠ suppression de compte (401), confirmation désactivée via réglage, logout → jeton 401.
  - Page Compte : liste des appareils (révocation), outils installés par appareil, interrupteur « demander mon mot de passe à chaque installation » (lecture-modification-écriture du blob de réglages). Testé navigateur ↔ API ↔ CLI : la révocation depuis le site rend le jeton de la CLI invalide (401).
  - Reste : trousseau système (le jeton est dans un fichier 0600 pour l'instant), passkeys.
- **`release.yml` validé sur les 3 systèmes (2026-10-16)** : run complet n°1 = Linux ✓ macOS arm64 ✓ (signature ad hoc) Windows ✗ → cause : chemins Windows avec `\` dans le Cargo.toml/Rust générés ; corrigé (`fwd()`, aussi dans `cli-gen`) ; run n°2 Windows seul ✓ + job `publish` ✓ (index signé dans la CI avec le secret `KORTEX_SIGNING_KEY`). L'index signé par la CI est accepté par la CLI locale (clé publique embarquée) et rejeté dès qu'un octet change. Entrées du workflow : `tools`, `only` (linux|macos|windows|all), `intel_mac`, `publish`. Coût mesuré : run complet ≈ 25 min Linux + ≈ 35 min macOS (×10) + ≈ 28 min Windows (×2) pour 1 outil + CLI + Store.
- Page `/telecharger` (FR) et `/en/download` : commandes par système (celle du visiteur mise en avant), aide SmartScreen / Gatekeeper, vérification (`kortex doctor`), désinstallation ; en-tête adaptable sur petit écran.
- Reste : `RELEASES_TOKEN` pour publier, `kortex login`/jetons d'appareil/installs liées au compte, confirmation par mot de passe à l'installation, numéro de séquence anti-rollback dans l'index, releases groupées (fusion d'index), raccourci Windows `.lnk`, Linux ARM.

## Decisions (user, session 1)
- Pas de Homebrew.
- Dossiers du Bureau : NE PAS TOUCHER.
- GitHub : créer un NOUVEAU dépôt privé ; ne pas supprimer l'ancien.
- Cloudflare : tout nettoyer (retour à l'état d'achat du domaine) — après inventaire + confirmation finale de la liste.
- Langues : FR + EN dès le lancement. E-mail : pas de vérification pour l'instant.
- Crédits IA : affichés en % restant, calculé d'après les neurons consommés (pas en nombre de requêtes) ; limite par utilisateur généreuse mais raisonnable.

## Next action (exact)
- Phase 2 (site) restant : crate `kortex-bus` côté Rust (protocole typé, pour Desk/Flow), spike Web Worker (c), police Inter auto-hébergée (téléchargement à valider), pages univers `/[univers]`.
- Phase 4 suite (local, SANS déployer) : passkeys, pages connexion/inscription du site (`deriveAuthKey`, jauge de crédits IA), jetons d'appareil (CLI/desktop), install liée au compte.
- Plus tard : Phase 5 desktop/CLI/Store, 6 Seek niveau 2, 7 Desk, 8 Mind, 9 Flow, 10 Hub, 11 catalogue (≥150 outils), 12 finition.
- À revérifier : vieillesse de base 6,90 %/0,40 % et APEC 0,024 % (calc.gross-net-salary) ; champ couleur natif ; QR Wi-Fi/vCard ; HEIC.
- Hors périmètre (décision user) : budget de taille wasm.

## Blockers / waiting for user
- Mentions légales : reste l'adresse postale (ou recours à l'hébergeur pour les particuliers) et l'e-mail de contact (ex. `contact@kortex.site` via Email Routing déjà configuré). Nom et région renseignés.
- Déploiement public : NE PAS déployer pour l'instant (décision user).
- (rien d'autre pour l'instant)

## Hypotheses in use
- Passkeys + mot de passe PBKDF2, sans vérification d'e-mail au départ.

## Catalogue stats
stable: 0 · beta: 6 · draft: 0 — password-generator, hash-calculator (secure) · generator (qr) · merge (pdf) · convert (image) · gross-net-salary (calc)
