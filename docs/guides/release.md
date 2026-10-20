# Compilation et distribution desktop

## État actuel — 20 octobre 2026

Le catalogue reste gelé à **38 outils**. Les téléchargements desktop sont **ouverts au public depuis le 19 octobre 2026** (autorisation explicite de l’utilisateur, PROGRESS.md) : `desktop_releases = true` dans `brand/features.toml`, pages `/telecharger` et `/en/download`, installateurs `/install.sh` et `/install.ps1` et index `/registry/*` servis par kortex.site.

Release publique courante : [`manual-39454686081`](https://github.com/kortexdevlop/kortex-v2/releases/tag/manual-39454686081) (run 39454686081, commit 880bb19, lot 36). Elle contient 187 fichiers : 38 outils, 6 apps et la CLI pour Linux x86_64, Windows x86_64, macOS ARM et macOS Intel, plus l’index signé (44 entrées). Le site sert un index identique à celui de la release. Les archives sont nommées `<id>-<version>-<cible>.tar.gz`.

Une compilation n’établit ni une installation fonctionnelle ni une recette de l’interface native : voir les limites plus bas.

## Publication incrémentale (`feed.yml`, ADR 0034)

Voie nominale : après une CI verte sur `main`, `feed.yml` ne reconstruit que ce qui a changé depuis le commit source du dernier index signé publié (`xtask feed-plan`), publie une release `feed-<sequence>` ne contenant que ces paquets (archives desktop, archives `web-<id>-<version>.tar.gz`), l'index signé fusionné (URLs absolues vers les releases précédentes, `sequence` + 1, `channel`) et le catalogue de contrats signé `catalog.json`, puis redéploie `run`, les apps et le site depuis les archives web de l'index fusionné. Une release référencée par l'index courant ne doit jamais être supprimée. Le canal `test` (dispatch manuel) crée une prerelease `test-feed-<n>` sans toucher à la production. Premier passage : les archives web n'existent pas encore, il construit tout le web et réutilise les paquets desktop publiés.

Anti-retour (ferme 10.2) : clients (CLI, Store, apps) conservent le plancher `(sequence, sha256)` de l'index et du catalogue acceptés par canal (`~/.kortex/floors.json`, cache web) et refusent une séquence antérieure, un contenu différent à séquence égale ou un autre canal.

## Workflow GitHub (release complète manuelle)

`release.yml` compile les mêmes sources Rust/Dioxus pour Linux, macOS ARM/Intel et Windows, ainsi que les 38 outils et trois apps web. Les apps sont découvertes depuis leurs contrats. Les paramètres manuels sont `tools=all`, `only=all|linux|macos|windows|web`, `intel_mac` et `publish=false` par défaut.

Le job macOS tourne sur `macos-15-intel` et compile les deux cibles Apple (arm64 et x86_64) : les runners macOS ARM hébergés n’étaient pas attribués (commit 880bb19).

La release desktop ne tourne pas à chaque push : uniquement manuellement ou sur un tag `release/*`. Conserver les résultats réussis de chaque cible pour éviter les reconstructions inutiles ; une relance par OS exécute aussi le job web, dont les caches sont réutilisés. `only=web` compile uniquement les bundles outils/apps et ignore les jobs desktop/signature ; utiliser cette option pour obtenir des reçus compatibles après un changement partagé sans recompilation native inutile. Les artefacts de workflow sont conservés sept jours : les récupérer avant expiration.

Le vérificateur impose l’inventaire sélectionné, deux fichiers réguliers par archive (manifeste et binaire), versions/cibles et empreintes. Les métadonnées JSON sont lues explicitement en UTF-8 sur tous les OS. Le job de signature génère ensuite l’index Ed25519 à partir des paquets du run. Un run Windows seul ne produit pas un index multiplateforme complet.

## Secrets et publication

- `KORTEX_SIGNING_KEY` : clé de signature de release, configurée dans les secrets GitHub du dépôt. La clé publique est `crates/kortex-catalog/release.pub` ; ne jamais committer la clé privée.
- Publication : depuis le passage du dépôt en public (ADR 0024), `publish=true` crée une GitHub Release de `kortex-v2` avec le jeton intégré (`contents: write`) ; aucun jeton personnel n'est nécessaire. L'index signé pointe vers `https://github.com/kortexdevlop/kortex-v2/releases/download/<tag>`. `kortex-releases` n'est plus utilisé.
- `publish=false` ne crée aucune release publique. Un index signé privé ou un paquet de test ne doit jamais activer le drapeau public `desktop_releases`.

## Installation

`installers/install.sh` (Linux, macOS) et `install.ps1` (Windows) installent uniquement la CLI `kortex` dans `~/.kortex/bin` (ou `$KORTEX_HOME/bin`), sans droits administrateur : lecture de `cli-latest.txt` à côté de l’index, téléchargement de l’archive de la cible depuis la release GitHub, contrôle SHA-256, extraction du seul fichier `kortex`. Les outils et apps s’installent ensuite avec `kortex install <id>` ou le Store desktop, qui vérifient la signature Ed25519 de l’index (clé publique compilée dans le binaire). Un compte est requis (`kortex login`).

## Limites ouvertes

- Recette Windows, Linux et macOS ARM par l’agent absente ; l’utilisateur a déclaré l’installation validée sur les trois systèmes le 19 octobre (non rejouée).
- Binaires macOS signés ad hoc et Windows non signés : avertissements Gatekeeper / SmartScreen attendus.
- Anti-retour de l’index : fait côté clients (séquence, canal, plancher) ; vaut pour les clients contenant ce code.
- Windows : lanceur `.cmd`, pas le raccourci `.lnk` demandé (10.7).
- La CLI publiée est en 0.1.0 : `kortex update --self` la remplace dès qu’un index propose une version supérieure (0.2.0 : CLI sans outils liés).
- Connexion au compte depuis le Store absente : passer par `kortex login`.
- Consentement par permission déclarée, annulation et dossiers explicites à l’installation restent ouverts (10.4).

Ces points sont suivis dans `docs/PLAN.md` (8.5, 10.2, 10.4, 10.6, 10.7, 10.9).
