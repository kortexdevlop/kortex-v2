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

## UI sur mesure (`ui = "custom"`)

```bash
cargo xtask new-tool --universe qr --name generator --custom   # ajoute feature `ui`, src/ui.rs, clés locales
```
- `src/ui.rs` exporte `#[component] pub fn Ui() -> Element` et lit l'hôte avec `kortex_ui::use_tool()` : `lang()`, `t(clé)`, `mode()`, `visible_param_ids()`, `run(inputs, params)`, `running()`, `result()`, `cancel()`, `save(fichier)`.
- Le module est derrière la feature `ui` (`#[cfg(feature = "ui")] pub mod ui;`) : CLI et moteur Worker ne lient jamais Dioxus. Le moteur reste pur.
- Les wrappers web et desktop générés rendent `ToolHost { tool::ui::Ui {} }` (mode, langue, exécuteur, annulation et sauvegarde natives fournis par l'hôte). Aucune couleur/rayon/ombre en dur : tokens `k-*` uniquement.
- `cargo xtask check` exige `ui.rs`, la feature `ui` et la porte `cfg` pour un outil `custom`, et refuse un `ui.rs` orphelin sur un outil `auto`.

Ne jamais modifier `web/site` : l'outil apparaît partout via le registre, sans branche par identifiant (RULES.md §3.7).

Pour un moteur lourd (image, PDF, hash de gros fichiers), déclarer `web_worker = true` dans `[tool]`. `cargo xtask build <id>` compile automatiquement le moteur headless et son Worker ; `wasm-bindgen` doit correspondre à la version verrouillée du dépôt. Le délai de `[limits].timeout_s` inclut le chargement du moteur. L'UI propose alors une annulation immédiate ; les codes d'erreur doivent figurer dans les locales FR/EN. Voir ADR 0008.

Pour compiler un lot web sans reconstruire les autres outils : `cargo xtask build text.cleaner calc.tip convert.units`. La compilation est séquentielle et réutilise le cache commun.

Les builds web conservent désormais un reçu local dans `.kortex/build/<id>/build-receipt.json`. Si les sources, les bibliothèques partagées, la configuration et tous les fichiers produits sont inchangés, `xtask build` évite la compilation. Un fichier de sortie supprimé ou modifié invalide également le reçu. Le cache concerne les builds réalisés après cette mise à jour.

## Publication et disponibilité (ADR 0034)

Ajouter ou modifier un outil ne demande **aucune recompilation des apps ni de la CLI**.

1. Écrire l'outil (contrat, moteur, glyphe, locales, contenu, tests), puis `cargo xtask registry` et `cargo xtask check <id>`. Pour une modification, **monter `version`** dans `tool.toml` : la CI (`versions`) refuse un outil modifié dont la version n'a pas changé.
2. Pousser sur `main`. Après la CI verte, `feed.yml` s'enchaîne seul :
   - `cargo xtask feed-plan` compare au dernier index signé publié (commit source, sequence) et ne retient que les outils touchés ; une crate partagée (plateforme, UI, runtime, bus, contrat), le verrou ou la toolchain retiennent tout, par fermeture des dépendances ;
   - web : bundle (UI + moteur `engine/`) de ces seuls outils ; desktop : binaires de ces seuls outils pour Linux, Windows, macOS ARM et Intel ;
   - release GitHub `feed-<sequence>` : uniquement les paquets reconstruits, le nouvel index signé (fusionné avec les entrées précédentes, URLs absolues) et le catalogue de contrats signé `catalog.json` ;
   - les hôtes statiques (`run`, apps, site) sont reconstitués à partir des archives web de l'index fusionné, sans recompilation, puis publiés.
3. Web : l'outil apparaît dans Workspace, Hub, Seek, Store, Flow et Mind au prochain chargement (ils lisent `kortex.site/registry/catalog.json`, vérifié par signature, avec cache hors ligne et repli sur les contrats embarqués). Flow et Mind exécutent son moteur par l'hôte de moteurs de l'origine `run` (`/engines/host.html`).
4. Machines installées : le Store et `kortex install <id>` installent l'outil depuis l'index signé ; `kortex update` propose les versions supérieures ; les apps desktop découvrent l'outil par son `contract.json`.

Manuellement : `feed.yml` (`workflow_dispatch`) avec `channel=test` publie une prerelease sans toucher à l'index ni aux hôtes de production (recette). `release.yml` reste la voie complète manuelle.

Les reçus de build évitent de recompiler les outils inchangés ; seule une modification d'une crate partagée invalide tous les bundles.
