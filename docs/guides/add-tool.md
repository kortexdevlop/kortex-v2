# Ajouter un outil (recette)

Écrit pour qu'une IA (ou toi) puisse ajouter plusieurs outils d'un coup.

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

Ne jamais modifier `web/site` : l'outil apparaît automatiquement partout via le registre.
