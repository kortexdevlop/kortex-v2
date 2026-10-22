# Ajouter un outil (recette)

> **Catalogue gelé à 38 outils + `image.compress`** (RULES.md §2.1). Cette recette ne s'applique à un nouvel outil qu'après une autorisation explicite de l'utilisateur ; elle sert aussi de référence pour corriger les outils existants.

Écrit pour qu'une IA (ou toi) puisse suivre la procédure sans connaissance préalable du dépôt.

```bash
cargo xtask new-tool --universe pdf --name merge     # copie tools/_template (placeholders remplacés)
```
Puis, dans `tools/pdf/merge/` :
1. **`tool.toml`** (le contrat — source unique) : `id` = `<univers>.<nom>` (jamais modifié après publication), noms/accroches/slugs **fr + en**, `[[inputs]]`, `[[params]]` (chacun avec `level` simple|medium|pro ; `default` obligatoire pour `simple`), `[[outputs]]` (`formats` obligatoire pour un fichier), `[permissions]`, `[[variants]]` pour les pages SEO supplémentaires. Types de ports : voir `crates/kortex-contract/src/port.rs`.
2. **`src/engine.rs`** : moteur pur. Pas d'UI, pas de `std::fs`/`web_sys`, pas d'`unwrap`/`panic!`. Erreurs = enum avec codes traduits dans `locales/`.
3. **`glyph.svg`** : 24×24, `viewBox="0 0 24 24"`, trait 1.5, `currentColor`.
4. **`locales/fr.toml` + `en.toml`** : mêmes clés. **`content/fr.md` + `en.md`** : 3 sections `##` (intro, étapes, FAQ) et ≥ 3 questions `###`.
5. **`tests/engine.rs`** : au moins un test nominal et un test d'erreur.

```bash
cargo xtask registry                 # régénère registry/contracts.json (à committer)
cargo xtask check <univers>.<nom>    # garde-fous
cargo test -p tool-<univers>-<nom>
```
Vérifier l'outil en ligne de commande : `cargo xtask build-desktop <univers>.<nom>`, l'installer dans un `KORTEX_HOME` de test, puis `kortex run <univers>.<nom> --input … --param …`. La CLI ne lie aucun outil : elle lance le binaire installé en mode `--engine` (ADR 0034).

## Interface, moteur wasm et lecteurs partagés (ADR 0035)
- L'interface d'un outil est **générée depuis son contrat** (`ui = "auto"`) par les lecteurs partagés : `kortex-player` (web et desktop, mêmes sources). Une interface personnalisée n'est plus prise en charge (RULES §3.8) : `cargo xtask check` refuse `ui = "custom"` et `new-tool --custom` échoue.
- Un outil ne compile donc **ni interface ni binaire** : seul son moteur Rust pur devient un `engine.wasm` (ABI v1, `kortex_runtime::export_engine!`), le même pour le web, le desktop, Flow, Mind et la CLI.
- Le binaire desktop de l'outil est le lecteur compilé une fois par cible dont la zone réservée est remplie par le paquet de l'outil (`tool.toml`, locales, glyphe, `engine.wasm`) : `cargo xtask build-desktop <id>`.
- Le web sert le lecteur unique (`/assets/`) et, par outil, `tool.toml`, `locales/`, `engine/engine.wasm` et `engine/worker.js` : `cargo xtask build <id>`.
- Vérifier qu'un moteur répond de la même façon en natif et en wasm : `cargo xtask engines-check <id>`. Les requêtes viennent du contrat (valeurs par défaut, chaque option, fichiers générés) ; pour un outil dont les entrées demandent un format précis (JSON, jeton…), ajouter `tests/fixtures/requests.json` : `[{"inputs": {"text": {"Text": "…"}}, "params": {…}}]` (valeurs au format serde du runtime).

Ne jamais modifier `web/site` : l'outil apparaît partout via le registre, sans branche par identifiant (RULES.md §3.7).

Tous les moteurs web tournent dans un Worker isolé (annulation immédiate, délai de `[limits].timeout_s` qui inclut le chargement du moteur) ; le champ `web_worker` du contrat n'a plus d'effet (conservé pour la compatibilité des contrats). Les codes d'erreur doivent figurer dans les locales FR/EN. Voir ADR 0008 et 0035.

Pour compiler un lot web sans reconstruire les autres outils : `cargo xtask build text.cleaner calc.tip convert.units`. Le lecteur web n'est compilé qu'une fois (reçu `.kortex/build/_player/`), chaque moteur seulement si ses sources ont changé (`.kortex/engines/<id>/receipt.json`).

Les builds conservent un reçu local : si les sources, les bibliothèques liées et tous les fichiers produits sont inchangés, la compilation est évitée. Un fichier de sortie supprimé ou modifié invalide le reçu.

## Publication et disponibilité (ADR 0034)

Ajouter ou modifier un outil ne demande **aucune recompilation des apps ni de la CLI**.

1. Écrire l'outil (contrat, moteur, glyphe, locales, contenu, tests), puis `cargo xtask registry` et `cargo xtask check <id>`. Pour une modification, **monter `version`** dans `tool.toml` : la CI (`versions`) refuse un outil modifié dont la version n'a pas changé.
2. Pousser sur `main`. Après la CI verte, `feed.yml` s'enchaîne seul :
   - `cargo xtask feed-plan` compare au dernier index signé publié (commit source, sequence) et ne retient que les outils touchés ; une crate partagée (plateforme, UI, runtime, bus, contrat), le verrou ou la toolchain retiennent tout, par fermeture des dépendances ;
   - web : fichiers (contrat, locales, `engine/`) de ces seuls outils ; desktop : binaires assemblés (lecteur partagé + paquet) de ces seuls outils pour Linux, Windows, macOS ARM et Intel ;
   - release GitHub `feed-<sequence>` : uniquement les paquets reconstruits, le nouvel index signé (fusionné avec les entrées précédentes, URLs absolues) et le catalogue de contrats signé `catalog.json` ;
   - les hôtes statiques (`run`, apps, site) sont reconstitués à partir des archives web de l'index fusionné ; le lecteur web (`xtask build-player-web`) est recompilé à cette étape car les pages de chaque outil nomment ses fichiers ; puis publiés.
3. Web : l'outil apparaît dans Workspace, Hub, Seek, Store, Flow et Mind au prochain chargement (ils lisent `kortex.site/registry/catalog.json`, vérifié par signature, avec cache hors ligne et repli sur les contrats embarqués). Flow et Mind exécutent son moteur par l'hôte de moteurs de l'origine `run` (`/engines/host.html`).
4. Machines installées : le Store et `kortex install <id>` installent l'outil depuis l'index signé ; `kortex update` propose les versions supérieures ; les apps desktop découvrent l'outil par son `contract.json`.

Manuellement : `feed.yml` (`workflow_dispatch`) avec `channel=test` publie une prerelease sans toucher à l'index ni aux hôtes de production (recette). `release.yml` reste la voie complète manuelle.

Une modification de l'interface ou d'une crate du lecteur reconstruit le lecteur et les binaires desktop assemblés, jamais les moteurs wasm ; une modification de `kortex-runtime` ou du macro de contrat reconstruit aussi les moteurs.
