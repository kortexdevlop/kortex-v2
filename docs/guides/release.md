# Compilation et distribution desktop

## État actuel — 17 octobre 2026

Le catalogue reste gelé à **38 outils**. Le Store web est publié sur https://store.kortex.site/ ; l’installation et les téléchargements desktop publics restent fermés. Une compilation n’établit pas une installation fonctionnelle ni une recette de l’interface native.

Les artefacts vérifiés sont extraits dans `.kortex/binaries/<target>/<id>/` :

| Cible | Exécutables récupérés et vérifiés | Provenance |
|---|---:|---|
| Linux x86_64 | 42 | release 39031294481, commit 7ed41ae |
| macOS Intel | 42 | même run ; signatures ad hoc vérifiées |
| macOS ARM | 42 | même run ; signatures ad hoc vérifiées |
| Windows x86_64 | 42 | run 39049702070 vert, commit ca3d623 ; index signé vérifié |

Chaque cible comporte 38 outils, trois apps (Store, Hub, Desk) et la CLI : **168 exécutables de release récupérés et vérifiés**. Ces binaires précèdent les nouvelles modifications du trousseau ; celles-ci suivent une compilation/recette native dédiée, explicitement séparée des releases. Les archives originales restent dans `.kortex/ci-artifacts/<run>/<os>/`. Les preuves versionnées dans `docs/validation/2026-10-17-desktop-*.json` indiquent le commit exact, les versions, cibles et SHA-256 des archives/binaires. Ne pas attribuer un ancien artefact à un commit plus récent.

## Workflow GitHub

`release.yml` compile les mêmes sources Rust/Dioxus pour Linux, macOS ARM/Intel et Windows, ainsi que les 38 outils et trois apps web. Les apps sont découvertes depuis leurs contrats. Les paramètres manuels sont `tools=all`, `only=all|linux|macos|windows|web`, `intel_mac` et `publish=false` par défaut.

La release desktop ne tourne pas à chaque push : uniquement manuellement ou sur un tag `release/*`. Conserver les résultats réussis de chaque cible pour éviter les reconstructions inutiles ; une relance par OS exécute aussi le job web, dont les caches sont réutilisés. `only=web` compile uniquement les bundles outils/apps et ignore les jobs desktop/signature ; utiliser cette option pour obtenir des reçus compatibles après un changement partagé sans recompilation native inutile. Les artefacts privés sont conservés sept jours : les récupérer avant expiration.

Le vérificateur impose l’inventaire sélectionné, deux fichiers réguliers par archive (manifeste et binaire), versions/cibles et empreintes. Les métadonnées JSON sont lues explicitement en UTF-8 sur tous les OS. Le job de signature génère ensuite l’index Ed25519 à partir des paquets du run. Un run Windows seul ne produit pas un index multiplateforme complet.

## Secrets et publication

- `KORTEX_SIGNING_KEY` : clé de signature de release, déjà configurée dans le dépôt privé. La clé publique est `crates/kortex-pkg/release.pub` ; ne jamais committer la clé privée.
- `RELEASES_TOKEN` : nécessaire à la publication dans le dépôt public de binaires `kortexdevlop/kortex-releases`, actuellement absent. Ne pas remplacer ce jeton par un accès large aux sources privées.
- `publish=false` ne crée aucune release publique. Un index signé privé ou un paquet de test ne doit jamais activer le drapeau public `desktop_releases`.

Les installateurs `installers/install.sh` et `install.ps1` sont présents comme sources ; leurs routes publiques restent exclues lorsque la distribution est fermée. Les commandes publiques de téléchargement ne sont donc pas proposées comme un parcours disponible.

## Blocages avant ouverture desktop

Le remplacement de `credentials.json` par le trousseau système est implémenté mais attend sa validation native complète (8.5). La CLI refuse désormais les installations anonymes dans les sources en validation ; confirmations/permissions complètes restent à recetter (10.4). les bundles/lanceurs doivent être complets (10.6), puis installation, désinstallation, mises à jour, raccourcis et interfaces doivent être recettés par OS (10.7/10.9). L’index nécessite encore une protection contre le retour à une ancienne release. Un lanceur Windows `.cmd` ne vaut pas le raccourci `.lnk` demandé.

La compilation et les tests actuels ne ferment aucun de ces points à eux seuls. Voir `docs/PLAN.md` pour les validations distinctes.
