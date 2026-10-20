# Ajouter un outil (recette)

> **Catalogue gelé à 38 outils** (RULES.md §2.1, décision de l'utilisateur du 2026-10-16). Cette recette ne s'applique qu'après une levée explicite du gel par l'utilisateur ; elle sert aussi de référence pour corriger les outils existants.

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
cargo xtask registry                 # régénère registry/index.json (à committer)
cargo xtask cli-gen                  # relie l'outil à la CLI `kortex` (crates/kortex-cli, à committer)
cargo xtask check <univers>.<nom>    # garde-fous
cargo test -p tool-<univers>-<nom>
```
Vérifier l'outil en ligne de commande : `cargo run -p kortex-cli -- run <univers>.<nom> --input … --param …`.

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

## Publication et disponibilité (état au 2026-10-20)

Aucune étape n'est automatique après `git push` : `ci.yml` vérifie `main` sans publier.

1. **Web** : lancer `release.yml` avec `only=web` (bundles outils et apps vérifiés par reçus), puis `deploy.yml` avec `source_run=<ce run>` et `publish=true` ; secours local : `docs/guides/deploy.md`.
2. **Desktop** : lancer `release.yml` avec `publish=true` ; la GitHub Release reçoit les archives `<id>-<version>-<cible>.tar.gz` des quatre cibles et l'index signé. Republier ensuite le site avec `KORTEX_RELEASE_DIR` pointant vers les fichiers de cette release, pour que kortex.site serve le nouvel index (`docs/guides/release.md`).
3. **Apps** : `crates/kortex-shell` embarque `registry/index.json` à la compilation (`include_str!` dans `catalogue()`). Hub, Desk, Store, Seek, Flow et Mind ne voient donc un nouvel outil qu'après recompilation et republication, sur le web comme sur desktop (où `available()` filtre ce catalogue embarqué par les outils installés).
4. **Machines installées** : `kortex install <id>` ou le Store desktop installent l'outil depuis l'index signé ; `kortex update` propose une nouvelle version seulement si la version du contrat a été montée.

Les reçus de build évitent de recompiler les outils inchangés ; seule une modification d'une crate partagée (plateforme, UI, runtime, bus, contrat) invalide tous les bundles.
