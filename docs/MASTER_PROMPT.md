# KORTEX — Prompt maître du projet

Le cahier des charges fonctionnel ci-dessous est appliqué avec [RULES.md](RULES.md), version consolidée des règles et des instructions utilisateur. Les consignes utilisateur les plus récentes priment ; les ADR et journaux ne peuvent pas y déroger.

Périmètre actuel : **38 outils gelés, plus 4 ajouts autorisés par l’utilisateur (`image.compress`, `code.jwt-decoder`, `text.diff`, `calc.bmi`) = 42**, correction de l’existant, applications Rust/Dioxus dans `apps/`, publication des lots terminés après contrôles groupés. Les anciennes cibles de croissance et chiffres indicatifs ne déclenchent aucun ajout d’outil. `docs/PLAN.md` contient les étapes et leurs validations.

## 0. Ton rôle et ta façon de travailler

Tu es l'architecte principal et le développeur de **Kortex**. Tu travailles en autonomie sur un chantier long (plusieurs semaines, plusieurs sessions). Tu dois :

1. **Communiquer en français** avec moi. **Code, noms de fichiers, identifiants, commits et commentaires techniques en anglais.** Contenu du site : français par défaut + anglais (voir §8.6).
2. **Aller vite sans sacrifier la qualité** : écris de gros blocs cohérents d'un coup, factorise, génère du code répétitif via des outils (xtask, macros) plutôt qu'à la main. Ne lance pas de tests manuels lents (navigateur, captures) à chaque micro-étape : appuie-toi sur des **tests automatiques rapides** (cargo test, check du contrat) et fais une vérification visuelle **en fin de lot**.
3. **Tenir la mémoire du projet** (tu n'as pas de mémoire entre sessions) :
   - `docs/PROGRESS.md` : phase en cours, fait / à faire / bloquants / prochaine action précise. **Mis à jour à la fin de chaque session, obligatoirement.**
   - `docs/adr/NNNN-titre.md` : une décision d'architecture = un ADR court (contexte, décision, alternatives, conséquences).
   - `CLAUDE.md` et `AGENTS.md` (contenu identique, l'un pour Claude Code, l'autre pour Codex) : règles condensées du projet, commandes utiles, structure, interdits. Ils pointent vers ce prompt.
4. **Ne jamais faire d'action destructive ou irréversible sans mon accord explicite dans le chat** (suppression de repo, DNS, Workers, bases de données, force-push, dépenses). Toujours présenter d'abord un **inventaire** de ce qui sera supprimé/modifié.
5. **Ne jamais committer de secret.** Secrets via `wrangler secret`, GitHub Secrets, `.dev.vars` (ignoré par git).
6. **Vérifier les versions et limites actuelles** (Dioxus, Astro, Rust, quotas Cloudflare) dans la documentation officielle au moment de la Phase 0 — les chiffres donnés ici sont indicatifs.
7. À la fin de chaque session : résumé en 5 lignes max + **ce que je dois tester moi-même** (URL, commande) + prochaine étape.

---

## 1. Vision

**Kortex** est un écosystème de logiciels **gratuits, modernes, multiplateformes et respectueux de la vie privée** :

- **38 outils existants** (convertisseurs, PDF, image, vidéo, audio, dev, calcul, sécurité, QR…) qui s'exécutent **100 % côté client** : *« vos fichiers ne quittent jamais votre appareil »* est l'argument central (marketing + architecture + coût serveur quasi nul).
- **Un seul code par outil** : moteur en **Rust**, interface en **Dioxus**, compilé à l'identique pour **WebAssembly (site)** et **application bureau** (Windows, macOS, Linux). Plus tard : mobile natif et **Kortex OS**.
- **Des applications d'écosystème** construites exactement comme les outils (même structure, même contrat, installables) :
  - **Kortex Mind** — agent IA universel (`agent.kortex.site`)
  - **Kortex Seek** — recherche intelligente d'outils (`search.kortex.site`)
  - **Kortex Desk** — bureau multi-fenêtres où les outils coopèrent (`workspace.kortex.site`)
  - **Kortex Flow** — automatisation type n8n/Make (`flow.kortex.site`)
  - **Kortex Hub** — tableau de bord personnalisable + compte (`dashboard.kortex.site`)
  - **Kortex Store** — appstore bureau + CLI `kortex` pour installer les outils
  - **Kortex Sync** — continuité entre navigateur et applications installées
  - **Kortex ID** — le compte utilisateur
- Le serveur (Cloudflare) **ne fait presque rien** : authentification, petits blobs de préférences/sync, quotas IA, proxy IA. Aucun traitement de fichier côté serveur.

---

## 2. Clarifications et optimisations du cahier des charges

Ces points corrigent ou précisent la demande initiale. Ils sont **décidés** sauf mention « à valider ».

1. **« Serveur pas puissant »** : il n'y a pas de serveur à gérer. Tout est sur Cloudflare (Workers, D1, Durable Objects, R2, Workers AI). Les **requêtes vers des assets statiques sont gratuites et illimitées** sur Workers Static Assets : chaque sous-domaine statique est donc servi **sans invoquer de script**. Seule l'API (`api.kortex.site`) exécute du code.
2. **IA Cloudflare** : le quota gratuit de Workers AI est d'environ **10 000 « neurons » par jour pour tout le compte**, pas par utilisateur. C'est **peu** (de quelques centaines à quelques milliers de petites requêtes LLM/jour au total). Conséquences :
   - un **moteur IA commun** (`crates/kortex-ai` côté client + Worker `ai-gateway`) gère quotas, cache, choix du modèle, streaming ;
   - crédits par utilisateur **rechargés toutes les 3 h** (seau à jetons), **plafonnés par un budget global journalier** ; quand le budget global est épuisé → message élégant « IA en pause, revient à HH:MM » ;
   - modèles petits par défaut (ex. Llama 3.x 8B/3B instruct, modèles d'embedding légers), gros modèles seulement si nécessaire ;
   - **l'orchestration de l'agent tourne dans le navigateur** ; le serveur ne fait que proxifier les appels modèle (CPU serveur minimal) ;
   - prévoir dans la config la possibilité future de passer en payant ou d'ajouter un autre fournisseur, sans changer le code des apps.
3. **Limites du plan gratuit Workers** (à revérifier) : ~100 k requêtes/jour, ~10 ms CPU par requête. → **Pas d'Argon2/bcrypt côté Worker** (trop coûteux en CPU) : privilégier **passkeys (WebAuthn)** + mot de passe hashé en **PBKDF2 via WebCrypto** (natif). KV a très peu d'écritures gratuites/jour → **ne pas utiliser KV pour des écritures utilisateur** ; utiliser **D1** (et Durable Objects pour les compteurs de quotas atomiques).
4. **Flow (automatisation) côté client** : un workflow web ne tourne que **pendant que l'onglet est ouvert**. Les déclencheurs planifiés / webhooks entrants nécessitent un serveur → disponibles **dans l'app bureau** (tourne en arrière-plan) ; sur le web, déclenchement manuel ou « tant que l'onglet est ouvert ». Expliquer cela clairement dans l'UI.
5. **Connecteurs (Gmail, Dropbox, Telegram, requêtes HTTP)** : 
   - OAuth avec **PKCE** quand le fournisseur le permet (pas de secret) ; sinon un tout petit Worker `oauth-broker` échange le code. **Les tokens sont stockés chez l'utilisateur** (chiffrés localement, sync chiffrée de bout en bout optionnelle), jamais en clair sur le serveur.
   - Les requêtes web depuis le navigateur sont bloquées par **CORS** pour beaucoup d'API → Worker `proxy` minimal, **réservé aux utilisateurs connectés**, rate-limité, liste blanche de domaines connecteurs + mode requête libre limité. Sur desktop : requêtes natives, pas de proxy.
6. **Noms d'outils et SEO** : un nom de marque par outil (façon « iMovie ») tuerait le référencement (« convertir png en jpg » se cherche, pas « Pixora »). Décision (voir §3) : **marques fortes pour les apps et les univers**, **noms fonctionnels clairs pour les outils**, préfixés visuellement par l'univers.
7. **Un outil = plusieurs pages SEO** : un même moteur (ex. convertisseur d'images) expose des **variantes** (png→jpg, jpg→png, webp→png, heic→jpg…) déclarées dans son contrat. Chaque variante = une page générée, préréglée, sans code supplémentaire. **Objectif actuel : terminer les 38 outils existants et leurs pages ; extension du catalogue suspendue.**
8. **UI auto-générée** : pour aller vite et garantir la cohérence, l'interface d'un outil simple est **générée automatiquement depuis son contrat** (zone de dépôt, paramètres filtrés par mode Simple/Medium/Pro, bouton d'action, aperçu du résultat). L'outil ne code une UI Dioxus sur mesure que s'il en a besoin (éditeur vidéo, éditeur photo, QR stylé…). → Structure d’un outil simple = **contrat + moteur** (+ icône + textes).
9. **Garde-fous à la compilation** : le contrat est lu par une **macro procédurale** ; si le moteur ne correspond pas au contrat, **ça ne compile pas**. `cargo xtask check` complète (structure, icône, textes, taille, dépendances). La CI bloque.
10. **Mobile** : site responsive irréprochable + **PWA installable et utilisable hors ligne** (service worker qui met en cache les outils utilisés). Applications mobiles natives Dioxus : plus tard, l'architecture doit le permettre.
11. **Budget zéro (décidé)** : aucun service payant, aucune signature de code payante, Cloudflare **plan gratuit uniquement**, IA limitée aux **~10 000 neurons/jour partagés par tous les utilisateurs**. Toute solution proposée doit rester gratuite ; si une fonctionnalité exige un service payant, elle est reportée et notée dans PROGRESS.md.
12. **Distribution desktop sans signature payante** : Windows SmartScreen et macOS Gatekeeper afficheront des avertissements. Mesures gratuites :
    - **la voie principale d'installation est la CLI `kortex` / le Store** : les fichiers téléchargés par `curl`/la CLI ne reçoivent pas l'attribut de quarantaine macOS, donc pas de blocage Gatekeeper pour les outils installés ainsi ;
    - signature **ad hoc** macOS (`codesign -s -`, gratuite, obligatoire pour Apple Silicon) ;
    - intégrité garantie par signature **Ed25519/minisign** vérifiée par la CLI et le Store ;
    - page d'aide claire pour les téléchargements directs (Windows : « Informations complémentaires → Exécuter quand même » ; macOS : clic droit → Ouvrir) ;
    - l'architecture doit permettre d'ajouter une vraie signature plus tard sans rien refaire (étape isolée dans la CI, désactivée).
12 bis. **Dépôt GitHub PRIVÉ (décidé)** — le code source ne doit jamais être public. Conséquences :
    - les GitHub Releases d'un dépôt privé ne sont **pas téléchargeables publiquement** → les binaires sont publiés dans un **second dépôt public `kortex-releases` qui ne contient QUE des binaires, l'index signé et les notes de version** (aucun code source), alimenté par la CI du dépôt privé via un jeton à portée limitée ;
    - les minutes GitHub Actions d'un dépôt privé sont **limitées (~2 000 min/mois en gratuit ; Windows ×2, macOS ×10)** → stratégie économe obligatoire (voir §11) ;
    - rappel honnête : tout code exécuté côté client (wasm, JS) reste inspectable par un utilisateur motivé. Mesures gratuites : builds release optimisés et *strippés* (pas de symboles, `panic = "abort"`, `wasm-opt -Oz`), pas de source maps publiées, logique sensible (quotas, crédits) **uniquement côté serveur**. Aucune licence open source : `LICENSE` propriétaire « tous droits réservés ».
13. **Éditer DOCX/ODT/convertir vers PDF** fidèlement est **très difficile** sans LibreOffice. Prévoir une approche progressive (texte, styles de base, tableaux, images) et étiqueter honnêtement les limites. Ces outils viennent tard.
14. **Vidéo/audio dans le navigateur** : utiliser l'**API WebCodecs** (native, zéro dépendance) pour encoder/décoder sur le web, et un *muxer* en Rust. Sur desktop : backend natif (crates Rust ou FFmpeg système détecté — à décider par ADR). Le moteur expose une **abstraction de codec** avec backends par plateforme.
15. **Légal (France / RGPD)** : mentions légales, politique de confidentialité, CGU, export et suppression du compte, analytics **sans cookie** (Cloudflare Web Analytics), **polices auto-hébergées** (pas de Google Fonts), consentement explicite avant d'envoyer un contenu à l'IA (un fichier n'est jamais envoyé à l'IA sans accord).
16. **Opérations Cloudflare** : privilégier **Wrangler CLI + fichiers de config versionnés** (reproductible) plutôt que le pilotage du navigateur. Le navigateur sert seulement pour ce que l'API ne permet pas (et avec mon accord).
17. **Internationalisation dès le départ** : FR à la racine (`kortex.site/convertisseur/png-jpg`), EN sous `/en/` (`kortex.site/en/converter/png-to-jpg`). Les slugs sont dans le contrat. Ajouter une langue = ajouter des fichiers de traduction.

---

## 3. Identité de marque et nommage

### 3.1 Système de noms (style « Apple Music / Apple TV »)
- Pattern : **Kortex + un mot anglais court**. Toujours deux mots, jamais de préfixe collé.
- Applications : **Kortex Mind** (agent), **Kortex Seek** (recherche), **Kortex Desk** (workspace), **Kortex Flow** (automatisation), **Kortex Hub** (dashboard), **Kortex Store**, **Kortex Sync**, **Kortex ID**, et la CLI `kortex`. Futur : **Kortex OS**.
- Univers (familles d'outils) : **Kortex Convert, PDF, Image, Video, Audio, Text, Code, Data, Calc, Secure, QR, Web, Color, Life, Play, AI**.
- Outils : nom fonctionnel localisé (« PNG en JPG », « Fusionner des PDF ») affiché avec le **glyphe de l'univers**. Exemple d'affichage : `Kortex PDF · Fusionner`.
- **À valider avec moi en Phase 0** : propose 2 alternatives de système de nommage avant de figer. Tous les noms vivent dans **un seul fichier** (`brand/names.toml`) pour pouvoir tout renommer d'un coup.

### 3.2 Logo
- Kortex : **monogramme ultra-fin, une seule ligne continue** (esprit pomme Apple : reconnaissable en 16 px comme en 512 px). Piste : un « K » formé par un pli/repli évoquant un cortex, ou une forme organique minimale dans un squircle.
- Livre en Phase 0 **4 propositions SVG** (clair/sombre, 16/32/512 px) pour que je choisisse.
- **Icônes d'outils générées** : chaque outil fournit uniquement un **glyphe SVG** (grille 24, trait 1.5, coins arrondis). Le build compose automatiquement l'icône finale : squircle + dégradé subtil de la couleur de l'univers + glyphe + ombre douce. → cohérence absolue, ajout trivial.
- Exports automatiques : favicon, icônes PWA, icônes desktop (`.ico`, `.icns`, PNG Linux), images Open Graph (rendues par `resvg` dans xtask, pas de dépendance npm).

---

## 4. Design system

### 4.1 Principes
Premium, futuriste, intemporel, **extrêmement minimaliste** (références : Apple, Linear, Vercel). Fond blanc lumineux, gris très clairs, **accents bleu azur et cyan**. Grandes cartes très arrondies, surfaces douces, bordures quasi invisibles, espacements généreux et mathématiques, **profondeur 3D raffinée** (superpositions, ombres diffuses multicouches, élévations, légère perspective, verre dépoli discret). Chaque élément semble **flotter**. Zéro surcharge, zéro couleur agressive, zéro bordure épaisse, zéro décoration inutile. **Mode sombre aussi soigné que le clair.**

### 4.2 Source unique de vérité
- `design/tokens/*.toml` (couleurs, typographie, espacements, rayons, ombres, flous, durées, courbes, z-index, points de rupture) → `cargo xtask tokens` génère :
  - `design/dist/kortex.css` : variables CSS + classes composants (`k-btn`, `k-card`, `k-input`…), **utilisé à la fois par Astro et par Dioxus** (Dioxus web et desktop rendent du HTML/CSS : une seule feuille de style pour tout l'écosystème) ;
  - `crates/kortex-ui/src/tokens.rs` : constantes Rust si besoin.
- **Changer le thème de tout l'écosystème = modifier les tokens.** Interdit : couleurs/rayons/ombres en dur dans un outil (vérifié par `xtask check`).
- Pas de Tailwind ni de framework CSS : CSS moderne natif (custom properties, `color-mix`, `oklch`, container queries, `:has`, nesting, `@layer`).

### 4.3 Valeurs de départ (à affiner)
- Couleurs en **OKLCH**. Clair : fond `#FFFFFF`, surfaces `#F7F8FA` / `#F1F3F6`, texte `#0B0F19`, secondaire `#5B6472`, bordure ≈ 6 % d'opacité. Accent azur ≈ `#0A84FF`, cyan ≈ `#22D3EE`, dégradé signature azur→cyan utilisé avec parcimonie. Sombre : fond `#0A0B0D`, surfaces élevées plus claires (l'élévation s'exprime par la luminosité, pas par l'ombre).
- Échelle d'espacement base 4 : 4, 8, 12, 16, 24, 32, 48, 64, 96, 128.
- Rayons : 10 (contrôles), 16 (champs), 24 (cartes), 32 (panneaux/fenêtres), squircle pour les icônes.
- Ombres : 3 couches diffuses très faibles par niveau d'élévation (0 à 5).
- Typographie : **Inter Variable** (ou Geist) auto-hébergée, `font-feature-settings` soignés, échelle modulaire 1.25, titres serrés (letter-spacing négatif), texte secondaire discret ; mono : JetBrains Mono / Geist Mono.
- Mouvement : courbes type ressort (`cubic-bezier(.2,.8,.2,1)` + `linear()` pour springs), durées 120/200/320/500 ms, **`prefers-reduced-motion` toujours respecté**.

### 4.4 Animations
- Animations **natives** d'abord : CSS transitions, **View Transitions API** (transitions entre pages Astro et ouverture d'outil), **scroll-driven animations** (`animation-timeline: view()`) pour la page d'accueil. Pas de GSAP.
- Micro-interactions précises : survol (élévation + lueur azur très douce qui suit le curseur), pression (léger scale 0.98), focus visibles élégants, dépôt de fichier (la zone « respire »), succès (coche dessinée au trait).
- Signature visuelle : un **halo/maillage lumineux azur-cyan** très subtil, réactif au curseur, sur les éléments héros. Jamais criard.

### 4.5 Accessibilité et lisibilité
WCAG 2.2 AA minimum : contrastes, navigation clavier complète, ARIA correct, tailles tactiles ≥ 44 px, `lang` correct, textes alternatifs. Le mode **Simple** sert aussi d'accessibilité (gros textes, une seule action).

---

## 5. Architecture du dépôt (monorepo)

```
kortex/
├─ CLAUDE.md · AGENTS.md            # règles condensées pour les agents IA
├─ Cargo.toml                       # workspace Rust (membres générés/glob)
├─ rust-toolchain.toml              # version Rust figée, edition 2024
├─ package.json · pnpm-workspace.yaml
├─ brand/                           # names.toml, logos sources, couleurs d'univers
├─ design/
│  ├─ tokens/                       # SOURCE de vérité du thème
│  ├─ icons/                        # icônes système (UI) en SVG
│  └─ dist/                         # généré : kortex.css, icônes composées
├─ tools/                           # ★ UNIQUEMENT les outils, rien d'autre
│  ├─ _template/                    # modèle copié par `xtask new-tool`
│  └─ <universe>/<tool-name>/       # ex. tools/image/convert/
├─ apps/                            # apps d'écosystème, MÊME structure que les outils
│  ├─ mind/ seek/ desk/ flow/ hub/ store/
├─ crates/                          # bibliothèques partagées (voir §5.2)
├─ web/
│  ├─ site/                         # Astro : kortex.site (vitrine, catalogue, pages outils)
│  └─ shell/                        # gabarit HTML minimal partagé par les apps Dioxus web
├─ services/                        # Cloudflare Workers (TypeScript, zéro framework lourd)
│  ├─ api/                          # auth, compte, sync, stats, installs
│  ├─ ai-gateway/                   # quotas + proxy Workers AI
│  ├─ proxy/                        # proxy CORS pour Flow (connectés uniquement)
│  ├─ oauth-broker/                 # échange OAuth quand PKCE impossible
│  └─ shared/                       # types, validation, helpers
├─ registry/                        # généré : index des outils (JSON) pour site, apps, Store
├─ xtask/                           # outillage du projet en Rust (cargo xtask …)
├─ installers/                      # install.sh, install.ps1, packaging desktop
├─ tests/                           # tests transverses (smoke e2e minimal)
├─ docs/
│  ├─ MASTER_PROMPT.md · PROGRESS.md · ARCHITECTURE.md
│  ├─ adr/ · guides/ (ajouter un outil, ajouter un univers, déployer)
│  └─ deps.md                       # dépendances autorisées + justification
└─ .github/workflows/
```

### 5.1 Structure d'un outil (identique pour une app)

```
tools/image/convert/
├─ tool.toml          # LE CONTRAT (unique source de vérité de l'outil)
├─ Cargo.toml         # généré par xtask new-tool
├─ glyph.svg          # glyphe 24×24 (l'icône finale est composée au build)
├─ src/
│  ├─ lib.rs          # kortex::contract!("tool.toml"); pub mod engine; pub mod ui;
│  ├─ engine.rs       # moteur pur Rust : AUCUNE dépendance UI, AUCUN accès plateforme direct
│  └─ ui.rs           # optionnel : UI Dioxus sur mesure (sinon UI auto-générée)
├─ locales/
│  ├─ fr.toml · en.toml   # libellés UI, messages d'erreur
├─ content/
│  ├─ fr.md · en.md       # texte SEO de la page : intro, guide, FAQ (structurés)
├─ tests/
│  ├─ fixtures/           # fichiers d'entrée de test (petits)
│  └─ engine.rs           # tests du moteur (cas nominal, limites, erreurs)
└─ CHANGELOG.md
```

Règles : un outil **ne dépend que de `crates/kortex-*` et de dépendances autorisées**. Il **n'importe jamais** un autre outil (la coopération passe par le contrat et le bus). Il ne contient **aucun** élément de navigation du site.

### 5.2 Crates partagées
| Crate | Rôle |
|---|---|
| `kortex-contract` | Schéma du contrat, système de types de ports, validation, conversions/adaptateurs entre types, génération JSON Schema (pour l'IA) |
| `kortex-macros` | `contract!`, `#[engine]` : génère les structs Inputs/Params/Outputs depuis `tool.toml`, vérifie la signature à la compilation |
| `kortex-runtime` | Exécution des moteurs : `Executor` avec backends **InProcess**, **WebWorker** (web), **NativeIpc** (desktop) ; progression, annulation, journal |
| `kortex-ui` | Composants Dioxus du design system + **UI auto-générée** (`AutoTool`), `DropZone`, `ParamPanel`, `RunButton`, `OutputView`, `BatchList`, `ModeSwitch`… |
| `kortex-shell` | Enveloppes d'exécution d'un outil : **Standalone** (desktop), **Stage** (site), **Window** (Desk), **Node** (Flow), **Headless** (Agent/CLI) |
| `kortex-bus` | Protocole de messages entre hôte et outils (postMessage / IPC) : envoyer/recevoir des valeurs typées, état, événements |
| `kortex-platform` | Trait d'abstraction plateforme : fichiers, dialogues, presse-papiers, notifications, stockage, réseau. Impl. `web`, `desktop`, futur `os` |
| `kortex-io` | Fichiers volumineux en streaming, OPFS, ZIP, détection MIME par signature, téléchargement/enregistrement |
| `kortex-media` | Abstraction codecs (WebCodecs sur web / natif sur desktop), image (decode/encode), audio |
| `kortex-ai` | Client IA commun (streaming, appels d'outils, quotas, cache, consentement) |
| `kortex-sync` | Client Kortex Sync (préférences, historique, layouts, workflows), chiffrement optionnel |
| `kortex-auth` | Session, passkeys, jetons d'appareil (desktop/CLI) |
| `kortex-i18n` | Chargement des `locales/*.toml`, clés vérifiées au build |
| `kortex-cli` | La CLI `kortex` (installation, mise à jour, exécution headless d'un outil) |

---

## 6. Le contrat d'outil (`tool.toml`)

C'est la pièce maîtresse : le site, Desk, Flow, Mind, Seek, le Store et la CLI **lisent uniquement les contrats** (via `registry/`). Un outil bien décrit est **automatiquement disponible partout**.

### 6.1 Exemple complet

```toml
schema = 1                                   # version du format de contrat

[tool]
id = "qr.generate"                           # identifiant stable, JAMAIS modifié
version = "1.0.0"                            # semver ; majeure = rupture de contrat
universe = "qr"
status = "stable"                            # draft | beta | stable | deprecated
ui = "custom"                                # auto | custom
platforms = ["web", "desktop"]
offline = true
ai = false                                   # consomme des crédits IA ?
viral_priority = 1                           # 1 = haute (ordonne le catalogue)

[tool.name]
fr = "Générateur de QR code"
en = "QR Code Generator"
[tool.tagline]
fr = "Des QR codes élégants, personnalisés, en un instant."
en = "Beautiful custom QR codes in seconds."
[tool.slug]
fr = "generateur-qr-code"
en = "qr-code-generator"

[search]                                     # alimente Kortex Seek et le SEO
keywords.fr = ["qr code", "qrcode", "code qr", "wifi qr"]
keywords.en = ["qr code", "qr generator"]
intents.fr = ["créer un qr code pour mon wifi", "mettre un lien dans un qr code"]
intents.en = ["make a qr code for my wifi"]
related = ["qr.scan", "image.convert"]

[limits]
max_input_bytes = "10MB"
max_batch = 500
timeout_s = 30

[[inputs]]
id = "content"
type = "text"
required = true
max_len = 4096
label = { fr = "Contenu", en = "Content" }

[[params]]
id = "size"
type = "integer"
range = [64, 4096]
default = 1024
level = "medium"                             # simple | medium | pro
label = { fr = "Taille (px)", en = "Size (px)" }

[[params]]
id = "foreground"
type = "color"
default = "#0B0F19"
level = "medium"

[[params]]
id = "error_correction"
type = "enum"
options = ["L", "M", "Q", "H"]
default = "M"
level = "pro"

[[outputs]]
id = "image"
type = "file"
formats = ["image/png", "image/svg+xml"]
default_format = "image/png"

[permissions]                                # requis pour Desk/Flow/Mind et le desktop
network = false
filesystem = "none"                          # none | read | write
clipboard = "write"
sensitive = false                            # true = confirmation obligatoire (Mind/Flow)
privacy = "local"                            # local | ai-consent (envoie du contenu à l'IA)

[[variants]]                                 # pages SEO supplémentaires = préréglages
id = "wifi"
slug = { fr = "qr-code-wifi", en = "wifi-qr-code" }
name = { fr = "QR code Wi-Fi", en = "Wi-Fi QR Code" }
preset = { template = "wifi" }
```

### 6.2 Système de types des ports
`text`, `rich_text`, `number`, `integer`, `bool`, `enum`, `color`, `date`, `datetime`, `duration`, `range`, `json`, `table` (lignes/colonnes), `file<mime-pattern>` (ex. `file<image/*>`), `files<…>` (lot), `secret` (jamais journalisé ni synchronisé), `url`.
- `kortex-contract` définit les **conversions automatiques** sûres (ex. `table` → `file<text/csv>`, `text` → `file<text/plain>`, `file<image/*>` → `files<image/*>`) : Desk et Flow les proposent quand on relie deux outils incompatibles directement.
- Les types de fichiers sont validés par **signature binaire** (pas seulement l'extension).

### 6.3 Moteur

```rust
kortex::contract!("tool.toml"); // génère Inputs, Params, Outputs, ToolMeta

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    cx.progress(0.1);
    cx.check_cancelled()?;
    // … logique pure …
    Ok(Outputs { image })
}
```
- Pur, déterministe, testable, **sans UI ni accès plateforme direct** (passer par `cx` : fichiers, temporaires, progression, annulation, journal).
- Erreurs typées avec **codes** traduits dans `locales/` (jamais de message brut à l'utilisateur).
- Les traitements lourds tournent en **Web Worker** (web) ou thread natif (desktop) : l'UI ne gèle jamais.
- Gros fichiers : streaming, OPFS, pas de copie inutile.

### 6.4 Interface
- `ui = "auto"` → `kortex_ui::AutoTool` génère tout depuis le contrat.
- `ui = "custom"` → `ui.rs` exporte `#[component] pub fn Ui() -> Element`, construit **avec les composants `kortex-ui`** et le hook `use_tool()` (état, run, résultats, mode).
- **Modes Simple / Medium / Pro** : une seule UI. Chaque paramètre a un `level`. Simple = aucun paramètre (valeurs par défaut intelligentes), grande zone d'action, gros textes ; Medium (par défaut) = options courantes ; Pro = tout + options avancées, lot, préréglages, raccourcis. Le mode se règle globalement (menu, compte, synchronisé) **et** par outil. Fonctionne identiquement sur desktop.

---

## 7. Exécution, coopération entre outils et modes d'hébergement

1. **Build par outil** : `engine.wasm` (headless, minimal) + `ui` web (wasm Dioxus) + binaire desktop (Dioxus desktop). Tailles mesurées ; cible historique indicative, pas de blocage au-delà de 400 Ko depuis la consigne utilisateur, sauf taille inadmissible. **Écart décidé le 2026-10-21 (ADR 0035, instruction de l'utilisateur)** : seul `engine.wasm` est produit par outil ; l'interface web et le binaire desktop viennent des lecteurs partagés `kortex-player` (mêmes sources Dioxus, compilés une fois par cible) auxquels le paquet de l'outil est ajouté.
2. **Isolation recommandée** (à confirmer par spike Phase 0) : les bundles d'outils sont servis depuis **`run.kortex.site`** et chargés dans des **iframes** par les hôtes (page outil, Desk, Flow, Mind). Communication via `kortex-bus` (postMessage typé). Avantages : un outil qui plante ne casse pas l'hôte, cache partagé entre tous les hôtes, aucune donnée de session accessible aux outils.
3. **Headless** : Flow, Mind, la CLI et les lots utilisent `engine.wasm` dans un Web Worker (web) ou le binaire de l'outil en mode `--engine` (JSON-RPC sur stdio) sur desktop. Web et desktop exécutent le même `engine.wasm` (ABI v1, ADR 0035).
4. **Desktop** : chaque outil installé = binaire autonome (mode Standalone, lecteur partagé + paquet de l'outil, ADR 0035) + son bundle UI web + manifeste dans `~/.kortex/tools/<id>/`. Desk/Flow desktop découvrent les **outils installés** via `~/.kortex/registry.json` : s'il n'y en a que 2, seuls ces 2 sont proposés. Sur desktop, accès système complet (créer/modifier des fichiers) **dans la limite des permissions du contrat**, avec sélection explicite des dossiers.
5. **Threads wasm** (SharedArrayBuffer) seulement pour les outils qui en ont vraiment besoin : exigent COOP/COEP → à valider dans le spike.
6. **Préparer Kortex OS** : tout accès plateforme passe par `kortex-platform` ; aucun outil n'appelle `web_sys`/`std::fs` directement hors de ce crate (vérifié par `xtask check`).

---

## 8. Site web — `kortex.site` (Astro + TypeScript)

### 8.1 Génération automatique des pages
- Une seule page modèle : `web/site/src/pages/[...tool].astro` (+ `en/[...tool].astro`) qui génère **toutes** les pages d'outils et de variantes via `getStaticPaths()` à partir de `registry/index.json` (produit par `cargo xtask registry` depuis tous les `tool.toml`).
- Pages générées aussi : accueil, `/[univers]` (catalogue par univers), `/outils` (catalogue complet filtrable), pages des apps (`/desk`, `/flow`, `/mind`…), `/telecharger`, pages légales.
- **Ajouter un outil ne modifie jamais `web/site`.**

### 8.2 Page d'outil (gabarit `ToolPage`)
- Haut de page : icône composée, nom, accroche, badges (« 100 % local », « hors ligne », « gratuit », plateformes), **bouton principal « Ouvrir »**, boutons secondaires « Ouvrir dans Desk », « Installer sur mon ordinateur ».
- **Le Stage** (le « lancement » signature de Kortex) : au clic sur « Ouvrir », la carte de l'outil se **transforme (shared-element/View Transition)** en une **fenêtre flottante** qui s'élève au-dessus de la page (≈ 92 % de l'écran sur desktop, plein écran fluide sur mobile), fond flouté-dépoli derrière, coins 32 px, ombre diffuse. Barre de fenêtre minimaliste : fermer, plein écran, « envoyer vers Desk », mode Simple/Medium/Pro. Fermer = l'animation inverse, le résultat reste accessible.
  - **Glisser un fichier n'importe où sur la page** ouvre directement le Stage avec le fichier.
  - Le bundle est **préchargé au survol / à l'apparition du bouton** → ouverture quasi instantanée.
  - URL mise à jour (`?open=1`) pour partager un lien qui ouvre directement l'outil (avec paramètres préremplis éventuels, utilisés par Seek).
- Sous le héros : « Comment ça marche » (3 étapes), contenu SEO (`content/fr.md`), FAQ, outils liés, variantes.
- Données structurées : `SoftwareApplication`, `FAQPage`, `HowTo`, `BreadcrumbList`. Image Open Graph générée par outil.

### 8.3 Accueil
Héros spectaculaire mais sobre (logo, promesse, recherche Seek intégrée façon Spotlight `⌘K`), animations au scroll (révélations, parallaxe légère, cartes qui flottent en 3D douce), univers en grandes cartes, outils populaires, démonstration de Desk et Flow, argument confidentialité, téléchargement desktop, pied de page complet.

### 8.4 Navigation globale
En-tête flottant en verre dépoli : logo, Univers (mega-menu élégant), Apps, recherche `⌘K`, sélecteur de mode Simple/Medium/Pro, thème, langue, compte. Sur mobile : barre inférieure ou menu plein écran animé.

### 8.5 Performance
- Pages de contenu : **zéro JS par défaut** (îlots Astro uniquement là où nécessaire).
- Objectifs : LCP < 1,2 s, CLS < 0,02, INP < 100 ms, Lighthouse ≥ 98 partout.
- Polices auto-hébergées, sous-ensembles, `preload` ciblé ; images AVIF/WebP ; CSS critique.
- **PWA** : manifeste, service worker qui met en cache le site et les outils déjà ouverts (hors ligne).
- Sécurité : CSP stricte, `Permissions-Policy`, HSTS, SRI.

### 8.6 i18n & SEO
FR racine, EN `/en/`, `hreflang`, sitemap(s) générés, URLs propres et stables (redirections déclarées si un slug change : champ `aliases` du contrat). Contenu SEO de qualité par outil (pas de texte creux généré en masse).

### 8.7 Hébergement des sous-domaines
| Hôte | Contenu | Type |
|---|---|---|
| `kortex.site` | Astro | Static Assets |
| `run.kortex.site` | bundles d'outils | Static Assets (cache long, immutables hashés) |
| `workspace.kortex.site` | Kortex Desk (Dioxus web) | Static Assets |
| `flow.kortex.site` | Kortex Flow | Static Assets |
| `agent.kortex.site` | Kortex Mind | Static Assets |
| `search.kortex.site` | Kortex Seek | Static Assets |
| `dashboard.kortex.site` | Kortex Hub + compte | Static Assets |
| `api.kortex.site` | API | Worker |
Tous les domaines sont définis dans **un seul fichier de config** (`brand/domains.toml`) pour pouvoir les changer. Les apps Dioxus web partagent l'en-tête `kortex-ui` identique à celui du site.

---

## 9. Les applications d'écosystème

Toutes sont dans `apps/<nom>/` avec un `app.toml` (même format que `tool.toml`, section `[app]`), compilées web + desktop, installables via Store/CLI.

### 9.1 Kortex Desk — `workspace.kortex.site`
- Bureau type OS : fenêtres déplaçables, redimensionnables, **aimantation** (moitiés, tiers, quarts, 2 en haut + 1 en bas, grilles), raccourcis clavier, double-clic barre = agrandir. Boutons de fenêtre : **croix rouge (fermer)** et **carré (plein écran)** (+ réduire dans le Dock). Style Kortex, pas une copie de macOS.
- Dock des outils, lanceur `⌘K` (Seek intégré), vue d'ensemble de toutes les fenêtres, **espaces** (layouts sauvegardés et synchronisés), restauration de session.
- **Coopération** : glisser la sortie d'un outil vers l'entrée d'un autre ; **liaisons persistantes** visualisées par des fils lumineux discrets (la sortie de A alimente B automatiquement à chaque changement) ; presse-papiers Kortex typé ; adaptateurs de types automatiques ; historique des fichiers produits dans la session.
- Barre supérieure de **stats premium** : outils ouverts, fichiers traités, volume traité localement, « données jamais envoyées : X Mo », temps de session, mémoire utilisée (si disponible), état IA/crédits.
- Mobile : fenêtres → cartes empilées balayables, liaisons conservées. Desktop : uniquement les outils installés.

### 9.2 Kortex Flow — `flow.kortex.site`
- Éditeur nodal (canvas, zoom, mini-carte, alignement, annulation/rétablissement), nœuds = outils (presque tous, sauf ceux marqués `flow = false` comme les éditeurs interactifs), + nœuds système : déclencheurs (manuel, fichier déposé, planification*, webhook*, surveillance de dossier*), logique (condition, boucle, fusion, filtre, délai, variables, expressions), **HTTP**, **IA** (prompt, classer, extraire, résumer), connecteurs (**Gmail, Dropbox, Google Drive, Telegram, Discord, Slack, Notion, webhooks**…), fichiers (lire/écrire sur desktop).
  (*) desktop uniquement ou onglet ouvert — affiché clairement.
- Exécution pas à pas, inspection des données entre nœuds, journal, réessais, modèles de workflows prêts à l'emploi, import/export JSON versionné, workflows synchronisés.
- Les connecteurs sont des **modules** avec leur propre contrat (`apps/flow/connectors/<nom>/connector.toml`), même logique d'ajout facile que les outils.

### 9.3 Kortex Mind — `agent.kortex.site`
- Conversation type ChatGPT (streaming, markdown, code, historique local synchronisable) **et** exécution d'actions : « Convertis mes 15 images en WebP, réduis leur taille et télécharge-les en ZIP ».
- Fonctionnement : les contrats sont convertis en **définitions d'outils (JSON Schema)** ; le modèle propose un **plan** (affiché comme un mini-workflow lisible) → l'utilisateur valide → exécution **locale** des moteurs → résultat. Confirmation obligatoire pour toute action `sensitive`, toute écriture disque, tout réseau.
- Les fichiers ne sont **jamais** envoyés au modèle ; il ne reçoit que des métadonnées (nom, type, taille) sauf consentement explicite par fichier.
- Un plan validé peut être **enregistré comme workflow Flow** en un clic.
- Compte requis, consommation de crédits visible en direct.

### 9.4 Kortex Seek — `search.kortex.site` (+ intégré partout via `⌘K`)
- L'utilisateur décrit son besoin (« réduire le poids d'une photo pour l'envoyer par mail ») → bon outil ouvert **avec paramètres préremplis**.
- Niveau 1 (gratuit, sans compte, instantané, hors ligne) : index local généré au build depuis `keywords`, `intents`, synonymes FR/EN, tolérance aux fautes.
- Niveau 2 (connecté, crédits) : compréhension par LLM si le niveau 1 n'est pas sûr de lui → choisit l'outil + produit les paramètres selon le JSON Schema du contrat.

### 9.5 Kortex Hub — `dashboard.kortex.site`
- Tableau de bord personnalisable : grille de **widgets** déplaçables/redimensionnables (courbes d'utilisation, outils favoris, récents, crédits IA, apps installées par machine, workflows, raccourcis, stats « données protégées »). Widgets définis comme des modules (`apps/hub/widgets/<nom>/`), faciles à ajouter.
- Page **Compte** : profil, sécurité (passkeys, mot de passe, sessions, appareils), préférences (mode, thème, langue), réglage « demander le mot de passe à chaque installation », export des données, suppression du compte.
- **Sobriété en base** : la disposition du dashboard = un petit JSON ; les statistiques détaillées sont calculées **localement** (journal d'événements en IndexedDB) ; le serveur ne reçoit que des **compteurs agrégés par jour** (outil, nombre), conservés 90 jours puis compactés.

### 9.6 Kortex Store + CLI `kortex`
- Installation de la CLI : `curl -fsSL https://kortex.site/install.sh | sh` et `irm https://kortex.site/install.ps1 | iex`.
- Commandes : `kortex login`, `kortex search <mots>`, `kortex install <outil>`, `kortex update [--all]`, `kortex list`, `kortex remove`, `kortex run <outil> --input … --param …` (headless), `kortex store` (ouvre l'appstore), `kortex doctor`.
- **Kortex Store** : app Dioxus (façon App Store, sobre) : univers, recherche, fiches, installer/mettre à jour/désinstaller, état par machine.
- Installation **liée au compte** : chaque installation demande le mot de passe/passkey (désactivable dans le compte). Jeton d'appareil stocké dans le trousseau du système.
- Paquets : artefacts sur les **Releases du dépôt public `kortex-releases`** (binaires uniquement, jamais de code), index signé `registry/index.json` servi par `kortex.site`, vérification **Ed25519** de chaque binaire avant installation. Raccourcis menu Démarrer / Applications / `.desktop` créés automatiquement, mises à jour automatiques optionnelles.

### 9.7 Kortex Sync
- Synchronise : préférences, mode, thème, favoris, récents (métadonnées seulement), layouts Desk, workflows Flow, conversations Mind (optionnel), dashboard.
- **Jamais les fichiers.** Blobs JSON petits (≤ 64 Ko par clé), versionnés, résolution de conflits par horodatage + fusion par champ. Chiffrement de bout en bout optionnel (clé dérivée localement).
- « Reprendre là où je m'étais arrêté » : la dernière session Desk/outil est proposée sur un autre appareil.

---

## 10. Backend Cloudflare

- **Workers** en TypeScript strict, sans framework lourd (routeur maison minimal), validation des entrées, réponses JSON typées partagées avec le client (types générés depuis Rust ou partagés via `services/shared`).
- **Auth (Kortex ID)** : inscription e-mail + passkey (recommandé) ou mot de passe (PBKDF2-SHA256 WebCrypto, sel unique, itérations max compatibles avec la limite CPU — à mesurer), vérification e-mail (fournisseur d'e-mail transactionnel à valider : Cloudflare Email Service si disponible, sinon offre gratuite d'un tiers), sessions en **cookie HttpOnly, Secure, SameSite=Lax, host-only sur `api.kortex.site`** (donc jamais envoyé à `run.kortex.site`), CORS en liste blanche des sous-domaines Kortex, protection CSRF, rate limiting, Turnstile sur les formulaires sensibles.
- **D1 (schéma minimal)** : `users`, `credentials` (passkeys), `sessions`, `devices`, `settings` (1 blob JSON), `sync_items` (clé, version, données ≤ 64 Ko), `installs` (appareil, outil, version), `usage_daily` (utilisateur, date, outil, compteur), `ai_ledger` (agrégé). Migrations versionnées.
- **Plan gratuit uniquement** : vérifier en Phase 0 que chaque service utilisé (Workers, Static Assets, D1, Durable Objects SQLite, Workers AI, Turnstile, Web Analytics) est disponible **sans abonnement ni moyen de paiement**. Sinon, alternative gratuite (ex. compteurs de crédits dans D1 au lieu des Durable Objects). Ne pas utiliser R2 s'il exige un moyen de paiement.
- **Durable Objects** (si gratuits) : compteur de crédits IA par utilisateur (seau rechargé toutes les 3 h) + compteur du budget global journalier.
- **AI gateway** : authentifie, vérifie/décompte les crédits, choisit le modèle selon la tâche (config centrale), streaming SSE, cache des réponses identiques, journalise l'agrégat (pas le contenu).
- **Infrastructure as code** : tous les `wrangler.toml`/`wrangler.jsonc` versionnés, environnements `preview` et `production`. GitHub Actions est la voie nominale ; la publication locale reproductible est le secours autorisé. Publier les lots terminés selon RULES.md §8.

---

## 11. CI/CD (GitHub Actions)

- `ci.yml` (chaque PR/push) : `cargo fmt --check`, `clippy -D warnings`, `cargo xtask check`, tests moteurs, build web des outils **modifiés**, build Astro, tests Workers, smoke e2e minimal (Playwright sur 3 pages clés — seule dépendance de test JS acceptée).
- `deploy.yml` (main) : build complet, déploiement des Workers et assets Cloudflare (preview sur PR, production sur main).
- `desktop.yml` (déclenché **uniquement** par tags `tool/<id>@x.y.z` / `apps/<id>@x.y.z` ou manuellement) : matrice **Windows / macOS (Apple Silicon + Intel via cross-compilation sur le même runner) / Linux** → binaires, paquets (zip/MSI, app/DMG, AppImage/deb/tar), signature ad hoc macOS, signatures Ed25519, publication sur `kortex-releases`, mise à jour du registre. Étape « signature éditeur » présente mais désactivée.
- **Économie de minutes (dépôt privé, ~2 000 min/mois)** — obligatoire :
  - `ci.yml` tourne sur **Linux uniquement** (le moins cher) et seulement sur les dossiers modifiés (filtres `paths`, détection par hash) ;
  - **aucun build desktop sur chaque push** : uniquement sur release ; releases **groupées** (un tag `release/AAAA-MM-JJ` construit tous les outils modifiés depuis la précédente en un seul job par OS) ;
  - macOS (×10) : un seul job, binaires universels/cross-compilés, le moins souvent possible ;
  - **builds Linux et Windows en cross-compilation depuis Linux** quand c'est fiable (ex. `cargo-xwin`), sinon runner Windows ;
  - cache agressif (cache Cargo + target, sccache), `concurrency` qui annule les runs obsolètes, `timeout-minutes` sur chaque job ;
  - le **déploiement web** (site, bundles wasm) se build sur Linux ; si les minutes manquent, `cargo xtask deploy` permet de déployer depuis ma machine locale avec Wrangler ;
  - `cargo xtask ci-budget` affiche la consommation estimée ; alerte dans PROGRESS.md si on approche de la limite.
- Releases automatisées : un changement dans `tools/x/` + bump de version → l'outil est inclus dans la prochaine release groupée, seul ce qui a changé est recompilé.

---

## 12. Garde-fous qualité : `cargo xtask check` (bloquant en CI)

Le check échoue avec un message **clair, actionnable, en français** (« ✗ tools/pdf/merge : `glyph.svg` manquant. Ajoute un glyphe 24×24 — voir docs/guides/add-tool.md#icone »). Il vérifie au minimum :

1. Chaque outil est dans `tools/<univers>/<nom>/`, univers existant dans `brand/universes.toml`.
2. `tool.toml` valide vs schéma ; `id` unique, stable (comparé à la branche principale : un id ne change jamais) ; slugs uniques par langue ; versions semver ; une rupture de contrat impose une version majeure.
3. Fichiers obligatoires présents : `glyph.svg` (format validé), `locales/fr.toml` + `en.toml` (toutes les clés), `content/fr.md` + `en.md` (sections obligatoires : intro, étapes, FAQ ≥ 3 questions), `tests/` avec au moins un test nominal et un test d'erreur.
4. Pas de couleur/rayon/ombre en dur hors tokens, pas d'accès plateforme direct hors `kortex-platform`, pas d'import d'un autre outil.
5. Dépendances : uniquement celles de `docs/deps.md` (toute nouvelle dépendance = ajout justifié dans ce fichier + ADR si importante).
6. Budgets : taille wasm, temps de build, taille des fixtures.
7. Les paramètres ont un `level`, les sorties un format, les permissions sont déclarées.
8. Le registre généré est à jour (pas de diff après `xtask registry`).
En complément, la macro `contract!` produit des **erreurs de compilation** si le moteur ne respecte pas le contrat.
Hook pré-commit optionnel : `cargo xtask check --changed`.

### Autres exigences qualité
- Zéro `unwrap()`/`panic!` dans les moteurs (clippy `deny`), erreurs typées partout.
- Tests : moteurs testés sur fixtures (nominal, limites, entrées corrompues, gros fichiers simulés) ; tests de propriétés pour les outils de calcul/conversion quand pertinent.
- Sécurité : entrées non fiables partout, pas de `unsafe` dans les outils, CSP, aucune exécution de code distant.
- Chaque outil terminé = **complet** : modes Simple/Medium/Pro, clair/sombre, mobile, clavier, lot si pertinent, états vides/chargement/erreur/succès soignés, annulation, textes FR/EN, page SEO.

---

## 13. Recette technique d’outil — ne pas exécuter pendant le gel du catalogue

```bash
cargo xtask new-tool --universe pdf --name merge      # crée tout le squelette depuis tools/_template
# 1. remplir tools/pdf/merge/tool.toml
# 2. coder src/engine.rs (+ src/ui.rs si ui = "custom")
# 3. dessiner glyph.svg, remplir locales/ et content/
# 4. écrire tests/
cargo xtask check pdf.merge && cargo test -p tool-pdf-merge
cargo xtask dev pdf.merge          # lance l'outil seul (web) avec rechargement
cargo xtask dev pdf.merge --desktop
```
Ensuite, **sans toucher à rien d'autre**, l'outil apparaît : page(s) du site + variantes, catalogue, univers, sitemap, Seek, Desk, Flow, Mind, Store, CLI, Hub.
Documenter cette recette dans `docs/guides/add-tool.md`, écrite pour qu'**une autre IA** puisse ajouter 5 outils d'un coup en la suivant. Même chose pour `add-universe.md`, `add-connector.md`, `add-widget.md`.

Autres commandes xtask : `new-app`, `registry`, `tokens`, `icons`, `og-images`, `build [--web|--desktop] [id]`, `release <id>`, `stats` (nombre d'outils par univers/statut).

---

## 14. Réservoir historique de catalogue — extension suspendue

Priorité au **potentiel viral** (⭐ = priorité haute). 🧠 = utilise l'IA (crédits). 🔧 = difficulté élevée. Les variantes SEO (paires de formats, etc.) ne comptent pas comme outils distincts.

**Convert (20)** — Convertisseur d'images universel ⭐ (variantes : png↔jpg, webp↔png/jpg, avif, bmp, tiff, gif…) · HEIC → JPG ⭐ · SVG → PNG · Image → ICO · GIF → MP4 · Vidéo → GIF ⭐ · Vidéo → MP3 (extraire l'audio) ⭐ · Convertisseur audio (mp3/wav/ogg/flac/m4a) · Convertisseur vidéo (mp4/webm/mov) 🔧 · CSV ↔ JSON · JSON ↔ YAML ↔ TOML · XML ↔ JSON · Markdown → HTML · HTML → Markdown · Tableur (xlsx/ods) → CSV · Unités ⭐ · Bases numériques · Timestamp ↔ date · Image ↔ Base64 / Data URI · Polices TTF/OTF → WOFF2

**PDF (20)** — Fusionner ⭐ · Diviser ⭐ · Compresser ⭐ · Pivoter · Réorganiser les pages · Extraire des pages · Supprimer des pages · PDF → images ⭐ · Images → PDF ⭐ · PDF → texte · Filigrane · Numéroter les pages · Signer un PDF ⭐ · Remplir un formulaire · Protéger par mot de passe · Retirer le mot de passe (connu) · Caviarder · Métadonnées · Annoter / éditer 🔧 · Comparer deux PDF

**Documents (6)** — Éditeur de documents (DOCX/ODT/MD) 🔧 · DOCX → PDF 🔧 · ODT → PDF 🔧 · Générateur de CV ⭐ · Générateur de factures ⭐ · Lettre de motivation (modèles) 

**Image (22)** — Compresseur ⭐ · Redimensionner (lot) ⭐ · Recadrer (formats réseaux sociaux) · Pivoter/retourner · **Kortex Image Studio** (éditeur photo : calques, réglages, filtres) 🔧 · Filtres & réglages · Supprimer l'arrière-plan ⭐ 🔧 (modèle local) · Agrandir une image (upscale) 🔧 · Voir/supprimer les EXIF ⭐ · Flouter/pixeliser (visages, zones) · Filigrane · Générateur de mèmes ⭐ · Créateur de collages · Découper pour grille Instagram ⭐ · Embellisseur de captures d'écran ⭐ · Extracteur de palette · Comparateur avant/après · Générateur de favicon · Image → ASCII art · Éditeur pixel art · Arrondir/encadrer · Image → texte (OCR) 🔧

**Video (12)** — **Kortex Video Studio** (montage timeline) 🔧 · Couper/rogner ⭐ · Compresser ⭐ · Redimensionner pour réseaux (9:16, 1:1) ⭐ · Fusionner des vidéos · Supprimer le son · Changer la vitesse · Pivoter · Extraire des images · Incruster des sous-titres · Enregistreur d'écran ⭐ · Enregistreur webcam

**Audio (12)** — Éditeur/découpeur audio ⭐ · Créateur de sonneries ⭐ · Normaliser le volume · Enregistreur vocal · Réduction de bruit 🔧 · Vitesse/hauteur · Fusionner des audios · Accordeur d'instrument ⭐ · Métronome · Détecteur de BPM · Synthèse vocale (texte → voix) · Transcription (voix → texte) 🧠

**Text (14)** — Compteur de mots/caractères ⭐ · Convertisseur de casse · Comparateur de textes (diff) · Supprimer les doublons · Trier les lignes · Rechercher/remplacer (regex) · Générateur de slug · Éditeur Markdown live · Lorem ipsum · Polices fantaisie pour réseaux sociaux ⭐ · Sélecteur d'emojis · Score de lisibilité · Texte → liste/tableau · Nettoyeur de texte (espaces, sauts, invisibles)

**Code (18)** — Formateur/validateur JSON ⭐ · Décodeur JWT ⭐ · Testeur de regex ⭐ · Base64 · Encodage URL · Entités HTML · Générateur UUID/ULID · Générateur d'expressions cron · Formateur SQL · Minifier JS/CSS/HTML · cURL → code · Testeur de requêtes HTTP · Générateur .gitignore · Calculateur chmod · Analyseur de User-Agent · Générateur de tableaux Markdown · Formateur XML/YAML · Différence de JSON

**Secure (12)** — Calculateur de hash (MD5, SHA-1/2/3, BLAKE3, CRC32 ; texte et fichiers) ⭐ · Vérificateur de somme de contrôle · Générateur de mots de passe ⭐ · Testeur de robustesse ⭐ · Générateur de phrases de passe · Chiffrer/déchiffrer (AES-GCM, texte & fichiers) · HMAC · Hash bcrypt/argon2 · Générateur de clés (Ed25519/RSA) · Générateur de codes 2FA (TOTP) · Nettoyeur de métadonnées (tous fichiers) · Partage de secret (Shamir)

**QR (6)** — Générateur de QR code premium ⭐ (variantes : Wi-Fi, vCard, lien, e-mail, SMS, géoloc) · Scanner QR (caméra/image) ⭐ · Générateur de codes-barres · Scanner de codes-barres · QR en lot (depuis CSV) · QR avec logo

**Calc (18)** — Calculatrice scientifique · Pourcentages ⭐ · Salaire brut ↔ net (France) ⭐ · Simulateur de prêt immobilier ⭐ · TVA · Intérêts composés · Âge exact ⭐ · Écart entre deux dates · Compte à rebours · IMC · Pourboire & partage d'addition · Moyenne scolaire / bac ⭐ · Coût d'un trajet (carburant) · Prix au kilo comparé · Fuseaux horaires · Conversion de devises (taux publics) · Règle de trois · Calculatrice de dates (ajouter jours ouvrés)

**Web (8)** — Générateur de balises meta + aperçu ⭐ · Aperçu Open Graph · Générateur d'images Open Graph · Constructeur d'UTM · robots.txt · JSON-LD (schema.org) · Requêtes DNS (DoH) · Densité de mots-clés

**Color (8)** — Générateur de palettes ⭐ · Générateur de dégradés CSS ⭐ · Convertisseur de couleurs (HEX/RGB/HSL/OKLCH) · Vérificateur de contraste (WCAG) · Simulateur de daltonisme · Générateur d'ombres CSS · Générateur glassmorphism ⭐ · Optimiseur SVG

**Data (6)** — Visionneuse/éditeur CSV · Créateur de graphiques ⭐ · Visionneuse JSON en arbre · Bac à sable SQL (SQLite local) · Générateur de fausses données · Fusion/nettoyage de tableaux

**Life (8)** — Minuteur Pomodoro · Chronomètre/minuteur · Liste de tâches locale · Tableau blanc ⭐ · Signature e-mail · Générateur d'équipes · Planificateur de repas/courses · Convertisseur de recettes (portions)

**Play (8)** — Roue de tirage au sort ⭐ · Dés & pile ou face · Test de vitesse de frappe ⭐ · Test de temps de réaction ⭐ · Générateur de noms/pseudos · Tirage au sort pour concours (commentaires collés) ⭐ · Test d'écran (pixels morts) · Test clavier/souris/manette

**AI (8, 🧠)** — Résumer un texte · Traduire · Reformuler/corriger · Décrire une image (alt text) · Discuter avec un PDF · Extraire des données structurées · Renommer intelligemment des fichiers · Générateur d'images

Liste historique de candidats uniquement. La consigne actuelle interdit tout ajout : terminer et vérifier les 38 outils existants. Réactivation uniquement sur instruction explicite de l’utilisateur.

**Outils pilotes (Phase 3)**, choisis pour couvrir toute la variété du contrat :
1. Convertisseur d'images universel (fichier → fichier, lot, variantes SEO, Worker)
2. Générateur de QR code premium (UI custom, paramètres riches, modes)
3. Calculateur de hash (texte et fichiers, streaming gros fichiers)
4. Fusionner des PDF (multi-fichiers, réordonnancement)
5. Générateur de mots de passe (aucune entrée, sortie texte)
6. Salaire brut ↔ net (calcul, UI auto-générée pure)

---

## 15. Feuille de route (une phase = une ou plusieurs sessions)

Chaque phase se termine par : critères et preuves de PLAN.md satisfaits, PROGRESS.md à jour, contrôles requis réussis, démo testable et limites explicites. Une compilation isolée ne valide pas toute une phase.

### Phase 0 — Cadrage, spikes, nettoyage
- Lire la doc à jour : Dioxus (dernière stable, web + desktop), Astro (dernière stable), Workers Static Assets, D1, Durable Objects, Workers AI (modèles, tarifs, quota gratuit), limites du plan gratuit.
- **Spikes jetables** (≤ 1 h chacun) : (a) un composant Dioxus compilé web ET desktop avec `kortex.css` ; (b) Stage = iframe `run.` + bus postMessage + View Transition ; (c) moteur en Web Worker appelé depuis l'UI ; (d) taille wasm réelle ; (e) build desktop sur les 3 OS en CI ; (f) WebCodecs depuis Rust.
- Rédiger les ADR fondateurs (iframe vs montage direct, format du contrat, stratégie desktop, stratégie codecs, auth).
- Propositions **logo (4)** et **nommage (2 alternatives)** → attendre ma validation.
- **Nettoyage de l'ancien projet** : inventaire complet (repos GitHub, Actions, secrets, Workers, Pages, D1/KV/R2, routes, DNS, règles) → je valide → suppression → confirmation. Garder une archive (zip du repo) avant suppression. Ne jamais toucher à l'enregistrement du domaine.
- Poser les questions ouvertes du §17.

### Phase 1 — Fondations
Monorepo, toolchains figées, tokens → `kortex.css`, `kortex-ui` (boutons, champs, cartes, fenêtres, menus, onglets, toasts, dialogues, dropzone, sliders, switch, color picker, progress, skeletons), `kortex-contract` + macros, `kortex-runtime`, `kortex-platform`, `kortex-i18n`, xtask (`new-tool`, `check`, `registry`, `tokens`, `icons`, `dev`, `build`), `tools/_template`, CLAUDE.md/AGENTS.md, CI de base. Galerie de composants (page `/_design` non indexée).

### Phase 2 — Site kortex.site
Layouts, en-tête/pied, accueil animée, catalogue, pages univers, **modèle de page outil + Stage**, i18n, SEO (sitemap, JSON-LD, OG), PWA, pages légales, pages vitrines des apps, déploiement Cloudflare (preview + prod).

### Phase 3 — 6 outils pilotes
Complets web + desktop, testés, modes S/M/P, pages SEO. Ajuster contrat/UI auto selon les retours. Vérifier que l'ajout via la recette est fluide.

### Phase 4 — Backend & compte
API, Kortex ID (passkeys + mot de passe), sessions, Sync, compteurs d'usage, AI gateway + crédits 3 h + budget global, pages connexion/inscription élégantes.

### Phase 5 — Desktop & distribution
Matrice de build, paquets, signatures, registre signé, CLI `kortex`, scripts d'installation, Kortex Store, mises à jour, confirmation par mot de passe à l'installation.

### Phase 6 — Kortex Seek
### Phase 7 — Kortex Desk
### Phase 8 — Kortex Mind
### Phase 9 — Kortex Flow (+ connecteurs, proxy, OAuth)
### Phase 10 — Kortex Hub (dashboard + widgets + compte)

### Phase 11 — Catalogue
Catalogue gelé à 38 outils plus `image.compress` (2026-10-20) : terminer les fonctions/contrats existants, vérifier les cibles web et desktop, compiler les changements et leurs dépendants, publier les lots validés. Aucun nouvel outil. Mettre à jour les preuves du plan et PROGRESS.md.

### Phase 12 — Finition & lancement
Audit perf (Lighthouse), audit accessibilité, audit sécurité (en-têtes, CSP, auth), relecture des textes FR/EN, test sur vrais appareils (iOS Safari, Android Chrome, Windows, macOS, Linux), mentions légales, monitoring d'erreurs léger (journal client agrégé, sans données personnelles).

---

## 16. Conventions

- Rust edition 2024, `clippy::pedantic` raisonné, `rustfmt` par défaut. TypeScript `strict`. Prettier/Biome (un seul outil, choisi par ADR).
- Nommage : crates `kortex-*`, outils `tool-<univers>-<nom>`, ids `<univers>.<nom>` (anglais, minuscules).
- Commits **Conventional Commits** (`feat(pdf.merge): …`), branches courtes, PR même en solo pour profiter de la CI et des previews.
- Pas de code mort, pas de TODO sans ticket dans PROGRESS.md.
- Dépendances : minimum absolu. Autorisées seulement si essentielles et irréalistes à réécrire (ex. décodeurs d'images/PDF, zip, crypto auditée). Toujours lister dans `docs/deps.md` avec : pourquoi, alternatives, poids.

---

## 17. Questions à me poser en Phase 0 (ne pas bloquer le reste)

1. Choix du logo et du système de noms.
2. Anglais dès le lancement ou FR seul d'abord ?
3. Fournisseur d'e-mails transactionnels **gratuit** (sinon : démarrer avec passkeys + mot de passe sans vérification d'e-mail, et ajouter la vérification plus tard).
4. Nombre de crédits IA par utilisateur toutes les 3 h (le budget global est fixé par les ~10 000 neurons/jour gratuits).

Déjà décidé, ne pas redemander : dépôt **privé**, licence propriétaire, **budget zéro** (pas de signature payante, Cloudflare gratuit uniquement), binaires publiés via le dépôt public `kortex-releases`.
7. Accès : jeton API Cloudflare (préféré) et accès GitHub (CLI `gh`) — l'extension Chrome en secours.

En attendant les réponses : avancer avec les choix recommandés de ce document et les noter comme hypothèses dans PROGRESS.md.

---

## 18. Gabarit de `docs/PROGRESS.md`

```markdown
# Progress
_Last update: YYYY-MM-DD — session N_

## Current phase
Phase X — <name> (≈ NN %)

## Done this session
- …

## Next action (exact)
- …

## Blockers / waiting for user
- …

## Hypotheses in use
- …

## Catalogue stats
stable: N · beta: N · draft: N — by universe: …
```

---

**Rappel final** : architecture propre, durable, séparée par sections ; outils isolés dans `tools/` ; tout piloté par les contrats ; thème global modifiable en un point ; ajout d'outil trivial et protégé par des erreurs explicites ; exécution 100 % locale ; design d'exception, cohérent du site aux apps bureau. Vise la perfection, mais livre par étapes.
