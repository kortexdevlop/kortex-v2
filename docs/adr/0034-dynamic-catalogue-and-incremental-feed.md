# 0034 — Catalogue signé dynamique, exécution sans liaison statique et publication incrémentale
**Statut** : accepté (2026-10-20) par l'utilisateur (« 1: oui 2: oui 3: oui », canal de test à l'écart, vrai outil `image.compress` pour la recette). Mission « un outil ajouté est disponible partout sans recompiler les apps ».

## Contexte (constaté dans le code le 2026-10-20)
- `kortex-shell::catalogue()` embarque `registry/index.json` (`include_str!`) ; les six apps et `kortex-search` en dépendent. Sur desktop, `available()` filtre ce catalogue embarqué : un outil installé mais inconnu de la compilation reste invisible. Test figé `catalogue().len() == 38`.
- `ToolHost` ouvre un outil web par `https://<run>/<id>/` : le bundle UI d'un outil est déjà indépendant des apps.
- Flow et Mind web chargent `/engines/<id>/worker.js` depuis **leur propre origine** : `xtask build-app` copie tous les `dist/engines` dans leur bundle (`bundles_engines = true`). Flow et Mind desktop lancent déjà le binaire installé en `--engine` (`kortex_workflow::process::ProcessRunner`).
- `kortex-cli` lie statiquement les 38 crates d'outils (`cargo xtask cli-gen`) : toute modification d'un outil la rend « modifiée » pour `xtask changed`.
- Collision de noms : `registry/index.json` du dépôt = contrats ; `https://kortex.site/registry/index.json` = index de paquets signé (URLs, SHA-256).
- Les clients publiés (CLI 0.1.0, Store 0.3.0) refusent un index `schema != 1` mais ignorent les champs inconnus (pas de `deny_unknown_fields`).
- `release.yml` (manuel ou tag `release/*`) publie une release complète ; `deploy.yml` exige un `source_run` ; les secrets Cloudflare de CI sont absents (PLAN 17.5). L'index n'a aucune protection anti-retour (10.2).

## Décision

### 1. Deux documents signés, noms sans collision
| Fichier | Rôle | Emplacement |
|---|---|---|
| `registry/contracts.json` (renommé depuis `registry/index.json`) | Contrats générés par `xtask registry`, versionnés dans Git ; **catalogue embarqué de repli** | dépôt |
| `catalog.json` + `catalog.json.sig` | **Catalogue de contrats publié** : contrats des outils dont le bundle web est réellement publié | `https://kortex.site/registry/catalog.json` |
| `index.json` + `index.json.sig` | Index de paquets (inchangé pour les clients installés), artefacts desktop **et** web | `https://kortex.site/registry/index.json` |

- Même clé Ed25519 que l'index (`crates/kortex-catalog/release.pub`, secret CI `KORTEX_SIGNING_KEY`) : aucun nouveau secret.
- Champs signés communs : `schema = 1`, `channel` (`stable` | `test`), `sequence` (entier monotone, **identique** dans les deux documents d'une même publication), `generated`, `source_commit`. L'index garde `schema = 1` et ajoute ces champs, ignorés par les anciens clients.
- L'index ajoute la cible `web` dans `targets` de chaque outil/app : archive `web-<id>-<version>.tar.gz` du bundle publié (sert à la reconstruction, §5). Les anciens clients ne lisent que leur cible native.

### 2. Vérification côté client (web wasm et desktop)
- Nouvelle crate pure `kortex-catalog` (aucune E/S, compile en wasm et natif) : décodage base64, vérification Ed25519 stricte, schéma, `channel` attendu, plancher anti-retour. `kortex-pkg::parse_verified` est déplacé dedans et réutilisé ; `kortex-shell` et `kortex-pkg` en dépendent. `ed25519-dalek` et `sha2` sont déjà autorisés (docs/deps.md, usage wasm ajouté à leur justification).
- **Anti-retour (ferme 10.2)** : le client conserve le plancher `(sequence, sha256)` du dernier document accepté. Il refuse `sequence < plancher`, et `sequence == plancher` avec un contenu différent. Plancher séparé pour l'index et pour le catalogue, et par canal.
- **Canal** : un document dont `channel` diffère du canal du client est refusé. Le canal `test` n'est accepté que si `KORTEX_CHANNEL=test` (desktop) ou une configuration de build de test (web) le demande ; la clé reste celle de production, donc aucun document de test ne peut être rejoué vers `stable`.
- Gel non détecté (serveur qui reste sur une ancienne version **déjà acceptée**) : limite connue, sans date d'expiration à ce stade.

### 3. Source au runtime, cache hors ligne et repli
Ordre, réévalué à chaque démarrage, jamais un document non vérifié :
1. **réseau** : `catalog.json` + `.sig` téléchargés via `kortex-platform` (web : `fetch` sans identifiants ; desktop : client HTTP natif existant, hors du fil de l'UI) ;
2. **cache** : derniers octets + signature acceptés, **revérifiés** à chaque lecture (web : stockage de l'origine via `kortex-platform::storage` ; desktop : `~/.kortex/catalog/`, partagé par les apps et la CLI) ;
3. **embarqué** : `registry/contracts.json` compilé, utilisé seulement si aucun document vérifié n'est disponible.
- Un catalogue vérifié **remplace** l'embarqué (il ne fusionne pas avec lui : un outil retiré ne doit pas réapparaître). Un échec réseau ou de signature n'affiche aucune erreur bloquante : l'app reste sur le cache ou l'embarqué et le signale discrètement (FR/EN).
- `kortex-shell` expose le catalogue par un contexte Dioxus réactif (`use_catalogue()`, signal) : rendu immédiat avec cache/embarqué, remplacement à l'arrivée du document vérifié.
- Web : `kortex.site/registry/*` reçoit `Access-Control-Allow-Origin: *` (données publiques, sans cookie) ; le CSP `connect-src` des apps ajoute l'origine racine.
- **Desktop (RULES §4.7)** : la liste des outils d'une app = outils du registre local de paquets. Métadonnées de chaque outil installé : `contract.json` écrit par la CLI/le Store à l'installation (copie de l'entrée du catalogue vérifié), sinon catalogue vérifié en cache, sinon embarqué. Un outil installé absent de l'embarqué apparaît donc. Le catalogue complet ne sert qu'au Store pour proposer l'installation.

### 4. Exécution sans liaison statique
- **Web, UI** : inchangé (iframe `https://<run>/<id>/`).
- **Web, moteurs (Flow, Mind)** : chaque bundle d'outil publie son moteur headless sous `https://<run>/engines/<id>/`. Une page **hôte de moteurs** `https://<run>/engines/host.html` (adaptateur minimal, RULES §3.6) est chargée en iframe cachée par Flow/Mind ; elle crée un Worker **de la même origine run** par invocation, renvoie résultat/erreur, et termine le Worker sur annulation ou délai (sémantique ADR 0008 conservée). Messages typés `kortex-bus` (`EngineRun`, `EngineCancel`, `EngineReply`), origine et fenêtre source vérifiées des deux côtés, `targetOrigin` explicite, octets en `Uint8Array` transférés. Le code d'outil reste isolé sur l'origine run (RULES §4.6) : pas de Worker cross-origin dans l'origine de l'app, pas d'ouverture du `script-src` des apps. CSP run : `frame-ancestors` ajoute Flow et Mind. Les apps ne copient plus `dist/engines` (`bundles_engines` retiré). Hors ligne : service worker de portée `/engines/` sur run, mise en cache à la demande.
- **Desktop (Flow, Mind, CLI)** : binaire installé appelé en `--engine`. `ProcessRunner` est déplacé dans `kortex-runtime` (backend NativeIpc prévu par le prompt) ; Flow, Mind et la CLI l'utilisent.
- **CLI** : la liaison statique des 38 crates et `xtask cli-gen` sont **supprimées**. `kortex run <id>` exécute le binaire installé ; s'il est absent, erreur claire proposant `kortex install <id>`. Conséquence : la CLI ne dépend plus d'aucun outil et n'est recompilée que pour son propre code. Changement visible : `kortex run` exige désormais l'outil installé.

### 5. Publication incrémentale
- **Base de détection** = `source_commit` du catalogue publié (vérifié), et non le push précédent : un run échoué est repris par le suivant. `cargo xtask changed --base <source_commit>` donne les ids touchés ; toute crate partagée (plateforme, UI, runtime, bus, contrat, catalogue), le verrou, la toolchain, les tokens ou les générateurs donnent `all`, détecté par la fermeture des dépendances et les empreintes des reçus (17.2b/17.7), jamais supposé.
- **Règle de version** (CI, PR et `main`) : un id dont l'empreinte change doit avoir une version strictement supérieure à la version publiée, sinon échec avec message actionnable. `cargo xtask bump --affected <patch|minor>` monte les versions concernées (y compris CLI et apps).
- **Construction** : seuls les ids touchés, en web (bundle + moteur) et pour les quatre cibles desktop ; aucun job d'OS sans id à construire.
- **Release par lot** : GitHub Release `feed-<sequence>` (prerelease `test-feed-<sequence>` pour le canal test) contenant **uniquement** les paquets reconstruits (archives desktop et web), le nouvel index et le nouveau catalogue. `xtask release-index --merge <index publié>` remplace les entrées reconstruites et **conserve les autres avec leurs URLs absolues** vers les releases précédentes ; `sequence = publiée + 1`. Une release référencée par l'index courant n'est jamais supprimée (contrôle avant publication).
- **Déploiement** : les répertoires statiques complets (run, apps) sont **reconstruits** à partir des archives `web` de l'index fusionné (SHA-256 vérifiés), sans recompiler les ids inchangés ; le site Astro est reconstruit (pas de Rust) avec l'index et le catalogue de la release. Ordre : run → apps modifiées → site. Une app n'est republiée que si son archive a changé.
- **Amorçage** : la première publication incrémentale est complète (`all`), pour créer les archives `web` de l'existant.

### 6. Déclenchement
- Workflow `feed.yml` : `workflow_run` du workflow `ci` terminé **avec succès sur `main`**, et `workflow_dispatch` (`channel = stable|test`, `force_all`). Rien à construire → aucun job coûteux. `concurrency: feed-<channel>`, sans annulation (séquence strictement ordonnée).
- Aucun `pull_request_target`, aucun secret exposé aux PR (ADR 0024). Secrets : `KORTEX_SIGNING_KEY` (existant), `CLOUDFLARE_ACCOUNT_ID` (enregistré le 2026-10-20), `CLOUDFLARE_API_TOKEN` (fourni par l'utilisateur, à enregistrer par lui dans GitHub). Sans eux, la CI s'arrête à la release et le déploiement passe par le secours local reproductible (`publish.mjs`), consigné comme tel.
- La publication `stable` sur `main` vaut publication de production d'un lot terminé (autorisation persistante, RULES §8.1). Le canal `test` ne touche ni l'index ni le catalogue de production, ni les hôtes Cloudflare.

## Écarts aux règles actés (instruction de l'utilisateur du 2026-10-20, RULES mis à jour)
- RULES §7.7 et MASTER §11 (« builds desktop groupés, pas sur chaque push ») : remplacés par « builds desktop des seuls ids modifiés, enchaînés après la CI verte de `main` » ; le dépôt public rend ces minutes gratuites (ADR 0024).
- RULES §6.6 (« les Releases ne contiennent que binaires, index signé et notes ») : précisé pour inclure les archives de bundles web compilés et le catalogue signé (aucune source).

## Alternatives écartées
- Contrats intégrés à l'index de paquets (un seul document) : alourdit chaque `kortex update` et lie la visibilité web aux paquets desktop. Écarté au profit de deux documents à séquence commune.
- Worker cross-origin chargé depuis run dans l'origine de l'app (blob + `import`) : exécute le code d'outil avec les privilèges de l'app (CORS de l'API). Écarté.
- Garder la liaison statique de la CLI pour les 38 outils : la CLI resterait recompilée à chaque modification d'outil. Écarté.
- Serveur de staging Cloudflare pour le canal test : nouvelles ressources DNS/Workers. Écarté au profit d'une prerelease GitHub (desktop) et d'une recette web locale (`*.localhost`).

## Conséquences et limites
- Les clients déjà installés (CLI 0.1.0, Store 0.3.0, apps) ne voient le catalogue dynamique qu'après **une** dernière mise à jour contenant ce code ; ils continuent de fonctionner avec l'index enrichi.
- Le web d'un outil nouveau est visible dès la publication du catalogue ; son installation desktop dès l'index. Les deux partagent la séquence, mais un outil peut être publié en web avant que toutes ses cibles desktop soient présentes : le Store ne propose que les cibles réellement listées.
- Le renommage `registry/index.json` → `registry/contracts.json` touche xtask, le site (`sync.mjs`), `prepare-publish.mjs`, `kortex-shell` et les contrôles.
- Les preuves restent distinctes (RULES §7.1) : tests, compilation web, compilation native, recette et publication ; « disponible partout » n'est déclaré qu'avec la recette de `image.compress` web et desktop (PLAN 19.13). Ce 39e outil, autorisé explicitement, est publié en production comme tout lot terminé.
