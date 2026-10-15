# Kortex — règles du projet (condensé)

Autorité : `docs/MASTER_PROMPT.md` (lire en entier au début d'une nouvelle phase). Mémoire : `docs/PROGRESS.md` (à mettre à jour à CHAQUE fin de session). Décisions : `docs/adr/`.

## Langue
- Échanges avec l'utilisateur : français. Code, identifiants, commits, commentaires techniques : anglais. Site : FR (racine) + EN (`/en/`).

## Interdits
- Aucune action destructive/irréversible (suppression repo/Workers/DNS/DB, force-push, dépense) sans accord explicite dans le chat + inventaire préalable.
- Aucun secret dans git (`wrangler secret`, GitHub Secrets, `.dev.vars` ignoré).
- Budget zéro : Cloudflare plan gratuit uniquement, aucun service payant.
- Dépôt privé, licence propriétaire. Pas d'argon2/bcrypt dans les Workers ; pas de KV pour écritures utilisateur.
- Pas de couleur/rayon/ombre en dur (tokens), pas d'accès plateforme hors `kortex-platform`, pas d'import d'un outil par un autre, pas de dépendance hors `docs/deps.md`.
- Pas d'`unwrap()`/`panic!` dans les moteurs.

## Structure
`tools/<univers>/<nom>/` (outils uniquement) · `apps/` · `crates/kortex-*` · `web/site` (Astro) · `services/` (Workers TS) · `registry/` (généré) · `xtask/` · `brand/` · `design/tokens` · `docs/`.
Un outil = `tool.toml` (contrat) + `engine.rs` + glyph + locales + content + tests. Ajouter un outil ne modifie jamais `web/site`.

## Commandes (cibles, à créer en Phase 1)
`cargo xtask new-tool|check|registry|tokens|icons|dev|build|stats` · `cargo fmt --check` · `cargo clippy -D warnings` · `cargo test`.

## Fin de session
Résumé ≤ 5 lignes + ce que l'utilisateur doit tester + prochaine étape ; PROGRESS.md à jour.
