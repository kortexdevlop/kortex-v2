# Architecture de Kortex

État courant de la structure. Les exigences viennent de MASTER_PROMPT.md §5 et de RULES.md ; l'avancement et les preuves sont dans PLAN.md. Ce fichier décrit l'organisation, pas la validation d'une fonction.

## Couches

| Couche | Dossier | Langage | Rôle |
|---|---|---|---|
| Outils (38, gelés) | `tools/<univers>/<nom>/` | Rust (moteur pur) + Dioxus | Contrat `tool.toml`, moteur, UI auto ou custom, locales, contenu, tests |
| Applications installables | `apps/<nom>/` | Rust + Dioxus | `app.toml`, mêmes sources compilées web et desktop (features `web` / `desktop`) |
| Bibliothèques partagées | `crates/kortex-*` | Rust | contrat, macros, runtime, plateforme, UI, shell, bus, i18n, sync, auth, pkg, cli |
| Site vitrine | `web/site/` | Astro + TypeScript | SEO, catalogue, Stage, palette du site, pages légales |
| Workers de données | `services/api`, `services/shared` | TypeScript strict | Auth, compte, sync, quotas, crédits IA (ADR 0006) |
| Workers de contenu statique | `services/{site,run,desk,hub,store}` | `wrangler.jsonc` seul | Static Assets servis sans script (MASTER_PROMPT §8.7), un hôte chacun |
| Génération | `xtask/`, `registry/`, `design/` | Rust / JSON / TOML | tokens, registre, build, paquets, `new-tool`, `new-app`, `dev`, `ci-budget` |
| Distribution | `installers/`, `.github/workflows/` | sh, ps1, YAML | Installation CLI, CI, releases groupées |

Les hôtes et domaines sont dans `brand/domains.toml`, les noms dans `brand/names.toml`.

## Règles de frontière (résumé de RULES.md)

- Un outil n'importe jamais un autre outil ; la coopération passe par contrats, registre et bus.
- Tout accès système, réseau, stockage ou processus à l'exécution passe par `kortex-platform`.
- Couleurs, rayons et ombres viennent des tokens (`design/tokens/*.toml` → `design/dist/kortex.css`).
- Une application installable n'est jamais réécrite en Astro/TypeScript.
- Ajouter un outil ou une app alimente le registre ; le site, la CLI et le packaging n'ont pas de branche par identifiant.

## Écarts connus avec la structure du prompt

Voir [AUDIT_CONFORMITE.md](AUDIT_CONFORMITE.md) pour la liste à jour. Principaux manques structurels : crates `kortex-io`, `kortex-media`, `kortex-ai` (à créer quand un usage réel existe), Workers `ai-gateway`, `proxy`, `oauth-broker` (l'IA vit dans `api`, ADR 0006), dossier `tests/` e2e et `web/shell/`.

Les numéros d'ADR 0012 et 0013 sont volontairement absents : ils décrivaient les prototypes Hub/Desk Astro retirés (voir `docs/adr/README.md`).
