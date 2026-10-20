# Ajouter une application installable

Les applications vivent dans `apps/<nom>/`, en Rust/Dioxus, avec les mêmes sources pour le web et le desktop. Une application Astro/TypeScript n'est jamais un substitut (RULES.md §3.3).

```bash
cargo xtask new-app --name flow     # crée apps/flow depuis xtask/templates/app, statut draft
```

Ensuite :
1. Ajouter l'entrée dans `[apps]` de `brand/names.toml` si elle manque.
2. `app.toml` : contrat (id `apps.<nom>`, noms/accroches/slugs FR + EN, permissions réelles). Ne pas déclarer plus de permissions que nécessaire.
3. `src/engine.rs` : règles pures, sans `dioxus::`, `web_sys`, `std::fs`, `unwrap` ni `panic!` (le check le refuse).
4. `src/ui.rs` : composants Dioxus avec `kortex-shell` (`Frame`, apparence) et `kortex-ui`. Aucun accès système direct.
5. `locales/fr.toml` + `en.toml` (mêmes clés), `content/fr.md` + `en.md`, `glyph.svg`, `tests/`.
6. Toute dépendance nouvelle : la justifier dans `docs/deps.md` avant de l'ajouter.

```bash
cargo xtask registry && cargo xtask check
cargo xtask dev apps.<nom>              # web avec rechargement (dx serve)
cargo xtask dev apps.<nom> --desktop    # fenêtre native
cargo test -p kortex-<nom>
cargo xtask build-app apps.<nom> --web  # bundle web + reçu
cargo xtask build-app apps.<nom>        # binaire desktop
```

Une app reste `draft` ou `beta` tant que ses compilations web **et** desktop, ses tests et sa recette ne sont pas prouvés (PLAN.md, étape 11.5).
