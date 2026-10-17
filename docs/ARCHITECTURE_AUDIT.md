# Audit de conformité et corrections — 16 octobre 2026

## Référence

Instructions utilisateur récentes, [RULES.md](RULES.md) consolidé et [MASTER_PROMPT.md](MASTER_PROMPT.md). [PLAN.md](PLAN.md) est le suivi des 151 étapes ; une règle, un test et un critère de sortie ne sont pas interchangeables.

## Périmètre et limites

Prompt lu intégralement lors de l’audit. Inspection des dossiers, contrats, générateurs, workflows, accès plateforme et artefacts locaux. Contrôles et versions dans PROGRESS.md. Pas de revue ligne par ligne exhaustive, de reconstruction des 38 binaires ou de CI distante réinterrogée : aucune déclaration de conformité totale n’est faite.

## Constats et état courant

| Constat initial | Correction effectuée | Reste non validé |
|---|---|---|
| Desk développé dans Astro/TS au lieu de apps/desk Rust/Dioxus | Code, tests et ADR supprimés, retrait publié ; page purement informative en préparation | Future application Rust, après socle partagé |
| Hub développé dans Astro/TS au lieu de apps/hub | Hub/Favorite, stockage et récents Stage, tests, ADR et artefacts publics retirés ; statut indisponible | Future application Rust, widgets et sync |
| Seek confondu avec la palette TS | Composant renommé SiteSearch et module site-search ; app Seek déclarée indisponible | Moteur/UI Rust réutilisables de apps/seek |
| Contrat Store réduit et parseur apps distinct | AppContract partage les types/règles Rust des outils ; contrat Store complet/draft, discovery validée, apps.json généré ; ancien parseur réduit remplacé | Gabarit new-app, cibles effectives communes et exécuteurs |
| Store mélangeait logique pure, UI et textes dupliqués | engine.rs séparé de ui.rs, boutons kortex-ui, textes FR/EN chargés par kortex-i18n ; tests catalogue/rendu réussis | Entrée desktop et accès plateforme, web, auth/trousseau, recette multiplateforme |
| Les garde-fous n’interceptaient pas les prototypes d’apps | Check apps : contrat, fichiers, moteur sans Dioxus/OS, ui.rs custom, locales, source TS interdite ; composants Astro portant un nom d’app refusés | Contrôle complet des dépendances, appels indirects, crates et code runtime généré |
| Accès système en dehors de kortex-platform | Backend natif et adaptateurs document centralisés ; runtime → platform sans cycle ; fichiers choisis, horloge, aléatoire, presse-papiers, téléchargement et entrée web migrés. Contrôles et recette du lot dans PROGRESS.md, ADR 0015/0016 | Les interfaces complètes web/desktop, permissions, trousseau et isolation native restent ouverts en 3.3/3.4/3.6/8.5 ; le garde-fou textuel ne remplace pas une analyse exhaustive des dépendances. |
| Bus/shell, NativeIpc et headless incomplets | Aucun nouveau bureau/liaison construit sur ce socle manquant | Crates réelles, trois backends, thread natif, moteur headless séparé par outil et --engine |
| CI sans compilation wasm des outils modifiés | Écart inscrit au plan, pas masqué par un build Astro réussi | Pipeline ciblé et reçu source/artefact |
| Jeton natif dans credentials.json au lieu du trousseau | Écart explicitement ouvert ; aucun nouveau stockage de ce type ajouté | Backend de trousseau par OS et migration sûre |
| Instructions et état mêlés | RULES.md consolidé, AGENTS/CLAUDE synchronisés, MASTER réconcilié, ROADMAP renvoie au plan | Vérifier continuellement les nouveaux écarts, jamais approuver une dérogation par ADR seul |

## Inventaire des retraits autorisés

Supprimés sans sauvegarde de leur code : Desk.astro, desk-browser.ts, desk-state.ts, tests Desk et ADR 0013 ; Hub.astro, Favorite.astro, hub-browser.ts, hub-state.ts, tests Hub et ADR 0012 ; génération de hub-tools.json, imports et comportements correspondants dans le Stage et les pages d’outils. Les anciennes sorties des recettes et captures locales dédiées ont aussi été retirées.

Les builds Astro et dist/publish ont été reconstruits sans ces bundles. Les pages /desk et /hub sont des vitrines noindex de fonctions futures ; ni le nom ni le projet d’app Rust ne sont supprimés du cahier des charges. La recherche locale du site, le Stage et le compte Astro/TS restent dans les couches explicitement autorisées du site. Le backend reste TypeScript conformément au prompt.

Le site efface les anciennes clés locales kortex-desk-v1 et kortex-hub-v1 à la prochaine visite ; cache public v4 pour renouveler les anciennes pages/scripts. Ces migrations ne conservent aucun comportement applicatif. Les profils jamais revisités ne peuvent pas être purgés à distance. Les historiques Git et versions inactives Cloudflare ne sont pas réécrits : ils contiennent le site entier. Aucun Worker/D1/DNS, outil Rust, secret ou donnée de compte supprimé.

## Artefacts : ce qui est prouvé

38 moteurs Rust, contrats, fichiers de tests et bundles UI wasm avec HTML/cache constatés ; trois moteurs wasm séparés, dix reçus web, un outil avec binaire desktop local plus Store/CLI. Détail dans ARTIFACT_AUDIT.json. Les sources communes de contrat viennent de changer : les vieux bundles ne deviennent pas automatiquement des builds des nouvelles sources. Aucune release de ces moteurs ou du Store n’est publiée dans ce lot.

Le contrat commun est vérifié en natif et sur la cible wasm ; les tests Store compilent son code Dioxus natif et vérifient son rendu. Cela ne constitue ni un Store web ni une recette d’installation sur trois OS. Les prochains travaux doivent fermer les lignes du plan avec leurs preuves, sans prétendre que le retrait des prototypes a réparé tout le runtime existant.
