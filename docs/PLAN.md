# Plan de réalisation et de remise en conformité — Kortex

État au 17 octobre 2026. Référence : [MASTER_PROMPT.md](MASTER_PROMPT.md), relu intégralement, avec les consignes utilisateur plus récentes (gel des 38 outils, publication par lot, suppression des implémentations non conformes, règles réconciliées dans RULES.md). Ce fichier remplace les listes contradictoires de l’ancienne ROADMAP comme suivi opérationnel.

## Arrêt demandé — 17 octobre 2026

**Projet en pause à la demande de l’utilisateur.** Cette session met uniquement à jour le suivi ; aucune reprise, compilation, publication, commit ou push.

- **10.1/10.7/17.4 — acquis conservés :** 168 exécutables de release vérifiés et extraits dans `.kortex/binaries/`, soit 42 par cible (38 outils, Hub, Desk, Store et CLI), pour Linux x86_64, Windows x86_64 et macOS ARM/Intel. Ils précèdent le correctif du trousseau ; compilation et intégrité ne valent pas recette d’installation complète.
- **8.5 — implémentation partielle :** backend natif via `kortex-platform`, migration/relecture/rollback, masquage des secrets et propagation des erreurs implémentés ; gate local complet réussi. Premiers allers-retours des trousseaux Linux/macOS/Windows réussis en CI, mais collecte des preuves native interrompue par un défaut du script, corrigé ensuite. Validation finale du lot toujours ouverte ; correctif non publié.
- **10.4 — partiel :** contournement d’installation anonyme retiré ; transaction complète de permissions/installation et gestion des échecs d’enregistrement serveur encore ouvertes.
- **17.7 — écart signalé :** relance de tous les 38 bundles web trop large pour ce correctif natif. Run web `39056138828` et run ciblé trousseau `39056138215` tous deux confirmés `completed / cancelled` après demande d’arrêt. Aucun de ces runs ne constitue une validation finale.
- **Production conservée :** dernières versions publiées du site, Store, Hub, Desk et outils inchangées ; téléchargements/installation desktop toujours fermés. Préparation de déploiement des nouvelles sources bloquée par le garde-fou de provenance, sans remplacement des artefacts publics.

**Reprise — 17 octobre 2026 :** l’utilisateur a demandé de corriger les violations et écarts de structure. Lot A (2.5, 2.6, 2.8 partiel, 17.7 partiel) et lot 2 (4.10, 17.2b, 17.9) réalisés localement ; lot suivant à annoncer avant modification. Aucun lancement de CI général ; commit/publication en attente de validation.

## Conclusion immédiate de l’audit

**Desk était un écart structurel important** : application réimplémentée dans Astro/TypeScript au lieu de Rust/Dioxus dans `apps/desk`. Son code, ses tests et l’ADR de cette implémentation sont retirés. **Hub présentait le même écart** ; son implémentation Astro/TS, favoris/récents, tests et ADR sont désormais retirés à la demande de l’utilisateur. **Seek n’a pas encore son app Rust** : seule la palette du site est réalisée. **Store 0.2 possède désormais une interface Rust/Dioxus commune ; installation sécurisée et distribution restent ouvertes.** Les accès système et le pipeline apps/compilation doivent aussi être corrigés avant de construire les apps suivantes.

Les **38 moteurs d’outils sont bien en Rust** et leurs **38 bundles UI wasm sont présents**. Il n’est donc pas exact de dire qu’aucun outil n’a été compilé. En revanche, leur conformité complète et leurs 38 versions desktop ne sont pas validées. L’inventaire initial constatait un seul outil desktop et dix reçus web. La remise en conformité a depuis produit 38 reçus web actuels, puis les deux applications Rust Hub/Desk en web et macOS ; leur recette native complète et leur distribution restent ouvertes. Les compilations/test historiques ne suffisent pas à prouver que tous les artefacts actuels correspondent aux sources actuelles.

Règles consolidées et frontières détaillées : [RULES.md](RULES.md). Astro/TypeScript pour le **site vitrine/SEO** et TypeScript pour les **Workers** sont expressément prévus par le prompt. Les bindings JavaScript générés et les petits adaptateurs de plateforme n’impliquent pas une seconde implémentation métier. La règle à restaurer est : **logique Rust et UI Dioxus communes pour tout outil ou application installable**, avec accès système derrière `kortex-platform`.

Première livraison Hub/Desk publiée et recettée le 17 octobre : [sources, builds et versions](validation/2026-10-17-hub-desk.json). Elle ne clôt pas leurs exigences complètes. Clients de session et sauvegarde manuelle web 0.2 ensuite livrés : [preuve du lot compte](validation/2026-10-17-account-backups.json).

Ce plan rend chaque manque visible et définit les preuves nécessaires. La remise en conformité a commencé par les règles consolidées, le retrait des prototypes et le schéma/registre/contrôle des applications Rust. Le reste demeure non validé tant que ses preuves manquent.

## Lecture des validations

- **Oui — périmètre précisé** : le fait indiqué a une preuve identifiée ; cela ne valide pas toute la phase.
- **Non** : critère de sortie non satisfait, partiel, non testé ou preuve actuelle manquante.
- **À vérifier en production** : modification préparée, statut à mettre à jour après déploiement et contrôle.
- **Suspendu** : travail non autorisé par les consignes actuelles (notamment extension du catalogue).

Les preuves historiques sont signalées. Une présence de fichier, un test moteur, une compilation wasm, une compilation native et une recette d’installation sont **cinq preuves distinctes**. Ne pas transformer l’une en validation des quatre autres.

## Ordre de travail obligatoire après cet audit

1. Retirer les prototypes Desk/Hub, réconcilier les règles et vérifier le site nettoyé. Aucun nouveau Desk/Hub avant le socle Rust partagé.
2. Renforcer contrats d’apps, garde-fous et preuve source/build ; empêcher qu’un nouvel écart structurel passe inaperçu.
3. Compléter plateforme, bus, shell et exécuteurs partagés avec une preuve web/native ; corriger Store et le desktop généré.
4. Établir la matrice des 38 outils et fermer les pilotes puis les compilations/recettes desktop nécessaires ; ne créer aucun outil.
5. Compléter auth/sync clients Rust, trousseaux et distribution signée ; garder IA et fonctions non opérationnelles désactivées.
6. Développer les apps Rust dans l’ordre des dépendances : Seek → Desk → Mind/Flow → Hub, avec les fondations communes prêtes. Aucune app n’est validée avant ses preuves web **et** desktop.
7. Fermer les audits de lancement, les vrais appareils et le légal.

**Priorité utilisateur du 17 octobre :** « refait le hub et fait le workspace ». Les étapes 13 et 16 sont donc avancées avec le shell/bus/stockage réellement nécessaires. Cela change l’ordre, sans valider les fondations complètes, auth/sync, distribution ou fonctions avancées encore absentes. ADR 0019 décrit le périmètre et les limites.

**Priorité utilisateur Store / GitHub du 17 octobre :** lot 10.3/11.2 pour le Store commun et 10.1/10.7/17.4 pour compiler les 38 outils, les trois apps et la CLI sur GitHub. Web, Linux, Windows, macOS ARM/Intel ; inventaires de paquets liés au commit, sans publication publique de binaires non recettés.

**Politique de compilation :** pas de recompilation générale après chaque frappe. Regrouper le travail ; compiler les sources Rust modifiées et leurs dépendants pour les cibles concernées avant validation/publication ; reconstruire ce dont le reçu est manquant ou invalide. Un changement Astro seul n’oblige pas à recompiler 38 moteurs inchangés. Un changement runtime/UI/bus commun peut au contraire invalider tous leurs bundles : le pipeline doit le détecter, pas le supposer.

## Étapes et validations

### 1. Reprise, règles et périmètre

Référence : §§0, 1, 15–18.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 1.1 | Retirer l’implémentation Desk Astro/TypeScript et ses tests | Retiré | Oui — code et production | Six fichiers supprimés ; aucune UI active, ancien bundle 404 et ancienne clé locale effacée en Chromium. Site d5e6b67c-f4af-4db4-bc94-84431c9ba09c. Limites des copies historiques : ARCHITECTURE_AUDIT.md. |
| 1.2 | Relire le prompt et confronter les livrables à sa structure | Réalisé | Oui — audit documentaire | Prompt lu intégralement ; code, commandes, workflows et artefacts inspectés. Cet audit ne remplace pas une revue exhaustive de chaque ligne. |
| 1.3 | Appliquer Rust + Dioxus aux outils et aux applications installables | Écart identifié | Non | Outils Rust présents ; prototypes web retirés ; applications Rust absentes sauf Store, encore incomplet. Aucun nouveau développement applicatif avant correction du socle. |
| 1.4 | Réserver Astro/TypeScript au site et TypeScript aux Workers | Règle explicite | Oui — règle du prompt | §8 : Astro/TS autorisés pour vitrine/SEO/intégration ; §10 : Workers TS. Aucun remplacement d’une application installable par une UI séparée. |
| 1.5 | Geler le catalogue aux 38 outils existants | En vigueur | Oui — inventaire | Amendement utilisateur prioritaire : cible historique 150+ suspendue. Ne créer aucun outil supplémentaire. |
| 1.6 | Garder budget gratuit, dépôt privé et licence propriétaire | Modifié par décision de l’utilisateur | Non — contrôle opérationnel restant | Le 2026-10-19 l’utilisateur a choisi de passer `kortex-v2` en **public** (CI bloquée par la facturation d’un dépôt privé ; minutes Actions gratuites en public). Audit préalable de l’historique (72 commits) : aucun secret détecté. La licence reste propriétaire (« tous droits réservés », aucune licence accordée) ; le budget reste nul. Décision à faire figurer dans un ADR. |
| 1.7 | Maintenir plan, mémoire et décisions concordants | Repris | Oui — fichiers actuels | PLAN.md devient le suivi opérationnel ; ROADMAP.md renvoie ici ; PROGRESS.md conserve les faits historiques et les corrections. |
| 1.8 | Signaler immédiatement un écart de structure avant de continuer | Règle ajoutée | Oui — instruction | Un ADR ne vaut pas autorisation de contredire le prompt. À chaque lot : indiquer couche, source Rust commune et cibles avant implémentation. |
| 1.9 | Préserver les modifications hors Desk et les ressources existantes | Appliqué | Oui — périmètre du retrait | Pas de reset global, force-push, suppression de Worker/D1/DNS ou réécriture d’historique. 123 entrées de travail Git constatées avant nettoyage : ne pas confondre avec 123 défauts. |

### 2. Fondations et structure du dépôt

Référence : §§3, 5, 15 phases 0–1.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 2.1 | Valider marque, noms et logo choisis | Présent | Oui — source et mémoire | brand/names.toml, brand/domains.toml, logo B ; choix utilisateur consigné. Pas de renommage arbitraire. |
| 2.2 | Conserver le workspace Rust et les versions figées | Présent | Oui — structure | Cargo.toml inclut crates/*, tools/*/* et apps/* ; rust-toolchain.toml et Cargo.lock présents. |
| 2.3 | Maintenir les outils dans tools/<univers>/<nom> | Présent | Oui — contrôle courant | cargo xtask check : 38 outils validés durant cet audit ; ne prouve pas tous les comportements à l’exécution. |
| 2.4 | Compléter le modèle apps/<nom>/app.toml | Contrat et structure | Oui — périmètre structure | AppContract partage les types et la validation avec ToolContract ; Store app.toml complet, moteur pur engine.rs, UI Dioxus ui.rs, locales/contenus/glyphe/tests. Cibles effectives web/desktop restent à terminer (11.2). |
| 2.5 | Générer une nouvelle app par xtask new-app | Présent | Oui — périmètre gabarit | `cargo xtask new-app --name <nom>` crée apps/<nom> depuis xtask/templates/app (Rust/Dioxus, features web/desktop, contrat, locales, contenu, tests). Une app jetable a passé `xtask check`, ses tests, clippy `-D warnings`, `cargo check` natif et wasm32, puis a été supprimée. Test xtask du gabarit. Ne valide aucune app réelle ni build web/desktop complet. |
| 2.6 | Compléter xtask dev pour web et desktop | Présent, non recetté | Non — lancement non exécuté | `cargo xtask dev <id> [--desktop]` : apps via `dx serve` (web) ou `cargo run` (desktop) ; outils via build à reçu puis serveur statique, ou build-desktop puis exécution. Compilé, clippy propre ; aucune session de dev réellement lancée, rechargement automatique seulement pour les apps. |
| 2.7 | Centraliser tokens, noms et domaines | Partiel | Non | Sources de tokens et marque présentes ; relever toutes les valeurs/destinations dupliquées dans site, packaging et générateurs. |
| 2.8 | Documenter architecture et recettes d’extension | Partiel | Non | docs/ARCHITECTURE.md et guides add-tool + add-app présents. add-universe, add-widget et add-connector restent à écrire après validation du socle. |

### 3. Plateforme, exécution et fichiers

Référence : §§5.2, 6.3, 7.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 3.1 | Rendre les moteurs indépendants de l’UI et du système | Présent pour les outils contrôlés | Oui — contrôle statique actuel | Moteurs Rust + scan xtask ; approfondir les appels indirects et dépendances avant clôture. |
| 3.2 | Faire passer tous les accès système runtime par kortex-platform | Réalisé sur le périmètre Rust audité | Oui — sources et parcours ciblés | ADR 0015/0016 : natif, runtime, UI et entrées générées migrés ; check, 220 tests Rust, wasm UI/runtime, 38 bundles web et un desktop macOS compilés ; fichier/copie/téléchargement/Worker/aléatoire recettés. [Empreintes et versions](validation/2026-10-17-platform-boundary.json). Scan textuel non exhaustif ; interfaces complètes et permissions restent en 3.3/10.4. |
| 3.3 | Fournir les backends plateforme web et desktop effectifs | Partiel | Non — stockage et presse-papiers ciblés, étape globale ouverte | [Stockage validé](validation/2026-10-17-preference-storage.json), ADR 0017. [Presse-papiers et Copier publiés](validation/2026-10-17-clipboard-results.json), ADR 0018 : 19 tests ciblés, aller-retour AppKit privé, 38 reçus à jour, un binaire macOS compilé, recettes UI locale et Stage public FR/EN réussies. Préférences UI, dialogues, notifications, trousseau, Windows/Linux/Safari et interfaces complètes restent ouverts. |
| 3.4 | Unifier Executor InProcess, WebWorker et NativeIpc | Partiel | Non | InProcess (`block_on`), WebWorker (`kortex-platform::worker`) et exécuteur natif sur thread + protocole stdio (`--engine`) existent séparément ; pas d’interface commune unique ni NativeIpc entre apps (Desk/Flow) démontrée. |
| 3.5 | Exécuter les trois traitements web lourds dans des Workers | Présent | Oui — preuves ciblées historiques | image.convert, pdf.merge, secure.hash-calculator : trois moteurs wasm séparés constatés ; essais hash/PDF/conversion consignés dans PROGRESS.md. |
| 3.6 | Exécuter les traitements desktop lourds hors thread UI | Réalisé pour les wrappers générés | Non — recette en fenêtre absente | `kortex_runtime::native::run_isolated` : moteur sur thread dédié (`kortex-platform::native::task`), annulation immédiate côté UI et coopérative côté moteur, délai issu de `[limits].timeout_s`, panique convertie en `worker_failed`. 7 tests (thread distinct, erreurs/panique, délai, annulation) stables sur trois exécutions ; wrapper desktop régénéré pour tous les outils et vérifié sur `text.cleaner` (compilé, fenêtre lancée sans erreur). CI : le workflow credentials (run 39073469728) compile et teste la plateforme sur Linux, macOS et Windows. Non prouvé : clic Annuler en fenêtre native, moteur long réel interrompu, Windows/Linux. Un moteur qui ne vérifie jamais l’annulation garde son thread jusqu’à sa fin. |
| 3.7 | Créer kortex-bus typé partagé web/desktop | Partiel | Non — protocole complet absent | Crate kortex-bus présente et handshake ready/init typé avec validation origine/source. Valeurs, transferts, connexions et IPC natif restent à réaliser ; ADR 0019. |
| 3.8 | Créer kortex-shell pour Standalone, Stage, Window, Node, Headless | Partiel | Non — modes complets absents | Crate shell partagée Hub/Desk : frame, catalogue, recherche, préférences et hôte iframe ; tests et deux cibles. Wrappers Standalone/Stage encore distincts, Node/Headless et embedding natif non unifiés. |
| 3.9 | Fournir engine.wasm headless pour chaque outil | Réalisé pour les 38 outils | Non — consommation par Flow/Mind absente, CI non exécutée | `cargo xtask build-engine <id…\|all>` : `engine.wasm` + `engine.js` + `worker.js` par outil dans `dist/engines/<id>/`, indépendant de l’UI et du drapeau `web_worker`, avec reçu (empreinte précise + hash de sortie). 38 moteurs compilés (7,1 Mo au total, ≈115–125 Ko chacun pour les plus simples). `tests/e2e/engines.mjs` (`pnpm e2e:engines`) exécute des moteurs réels dans un Worker de navigateur : nettoyage de texte, erreur typée `empty_input`, compteur de mots, SHA-256 de « abc » conforme à la valeur connue. Étape ajoutée à release.yml (artefact séparé `headless-engines`, non publié) : non exécutée sur GitHub. Limites : les moteurs ne sont pas servis par l’hôte `run` (publication fermée), Worker cross-origin et CSP à traiter avant que Flow web les consomme, 4 cas exécutés sur 38 outils. |
| 3.10 | Ajouter le mode desktop --engine JSON-RPC/stdio | Réalisé et exécuté sur un binaire réel | Oui — protocole sur macOS Intel | `<binaire> --engine` : JSON-RPC 2.0 une ligne par message, méthodes `run`, `contract`, `shutdown`, erreurs -32700/-32600/-32601/-32602/-32000, aucune fenêtre. Tests de la boucle avec flux en mémoire ; exécution réelle de `text.cleaner` : résultat, erreur typée `empty_input`, JSON invalide, contrat, arrêt. Limites : requêtes servies une à une, annulation par fin de processus, valeurs au format serde du runtime (`{"Text":…}`), Windows et Linux : plateforme compilée et testée par credentials (runs 39073469728, 39096557365), pas de binaire d’outil exécuté ; la CLI `kortex` reste distincte. |
| 3.11 | Fournir kortex-io : streaming, OPFS, ZIP et signatures MIME | Partiel — signatures seulement | Non | `kortex-io` existe avec la détection par signature (4.6). Streaming, OPFS, ZIP et fichiers volumineux sans copie ne sont pas livrés ; ils seront ajoutés quand un outil ou une app en a l’usage réel. |
| 3.12 | Valider gros fichiers, mémoire, délais, progression et reprise | Partiel | Non | Tests ciblés existants, dont hash 64 Mio ; limites maximales des contrats, progression détaillée et mesures mémoire non validées globalement. |
| 3.13 | Réaliser le spike WebCodecs Rust et décision codec | Absent / non validé | Non | Développer le spike avant kortex-media ; ne pas ajouter de nouveaux outils médias pendant le gel du catalogue. |

### 4. Contrats, registre et garde-fous

Référence : §§6, 12–13.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 4.1 | Générer Inputs/Params/Outputs depuis tool.toml | Présent | Oui — inspection et check | kortex-macros et macro contract! utilisés ; signature #[engine] contrôlée. Renforcer les cas négatifs de compilation si évolution. |
| 4.2 | Unifier le schéma des outils et applications | Réalisé | Oui — tests natifs + compilation wasm du schéma | parse_app/validate_app réutilisent types et règles ; registre apps.json généré ; ancien parseur ad hoc release remplacé. Apps sans ports de sortie autorisées, outils toujours avec sortie obligatoire. |
| 4.3 | Couvrir tous les types de ports et leurs contraintes | Partiel | Non | PortType et ValueMap présents ; plusieurs types deviennent String. Validation complète formats/ranges/secrets à vérifier. |
| 4.4 | Exécuter réellement les adaptateurs sûrs entre ports | Partiel — texte, table et fichier | Non — autres conversions absentes | `kortex-flow::engine::adapt` transforme texte→fichier (text/plain), table→CSV (échappement testé) et fichier→lot ; les autres paires de `converts_to` passent sans transformation. Utilisés par l’exécution de Flow et testés ; Desk n’a pas encore de transfert typé. |
| 4.5 | Produire JSON Schema et définitions IA depuis les contrats | Réalisé pour les définitions d’outils | Oui — génération et tests | `kortex_contract::tool_definition` : nom, titre/description localisés, `input_schema` (types, bornes, enums, couleurs, défauts, requis), permissions et sorties sous `x-kortex` ; fichiers = références opaques, secrets en écriture seule. `cargo xtask schema <id|all> [--lang]` : les 38 contrats génèrent. 5 tests. Non couvert : schéma des sorties, consommation par Mind/Seek (absents), validation d’un modèle réel. |
| 4.6 | Valider les formats de fichiers par signature | Réalisé pour les motifs vérifiables | Non — CI et recette UI non exécutées | Crate `kortex-io` (sans dépendance) : PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, AVIF, HEIC, PDF, ZIP, texte UTF-8 ; `check(octets, motif)`. Le `run_map` généré par `contract!` valide chaque port fichier avant le moteur (`file_type_mismatch`), donc UI, CLI et `--engine`. 5 tests de la crate + 2 tests de bout en bout sur pdf.merge et image.convert (faux fichier refusé, vrai PDF accepté) ; la vraie CLI refuse un faux PDF. Messages FR/EN et `accept` du sélecteur. Limites : motifs hors image/*, PDF, ZIP acceptés sans contrôle (JSON, CSV…), SVG et formats exotiques non typés, validation du sélecteur navigateur non recettée. |
| 4.7 | Bloquer dépendances, imports croisés et accès plateforme interdits | Partiel | Non | xtask scanne certains fichiers outils ; ne couvre pas assez apps, crates et code généré, ni toutes les dépendances de docs/deps.md. |
| 4.8 | Contrôler versions, ruptures et identifiants stables | Réalisé, exécuté en CI | Oui — contrôle de compatibilité | `cargo xtask check` compare les contrats au registre d’une référence git (`KORTEX_BASELINE_REF`, défaut `origin/main`) : identifiant disparu, version qui baisse, rupture (port supprimé, type changé, nouvelle entrée obligatoire) sans version majeure. 5 tests ; réel : conforme face à origin/main (13 outils de référence présents) et à HEAD ; échec vérifié sur une version abaissée volontairement puis annulée ; ignoré avec message sans référence (clone partiel). ci.yml : checkout complet et référence de la branche de base — étape `cargo xtask check` du job rust verte dans le run 39073472981 (commit ce903ea). Non couvert : renommage de slug sans alias, bornes de plages, compatibilité des apps. |
| 4.9 | Valider registre et génération CLI sans modifier le site | Présent | Oui — check courant | cargo xtask check valide registre et génération CLI actuels ; conserver cette propriété pour les apps et chaque changement de contrat. |
| 4.10 | Compiler les UI custom quand ui=custom | Mécanisme présent, aucun outil réel concerné | Non — rendu desktop non inspecté | `kortex_ui::ToolHost` + `use_tool` ; feature `ui` du crate d’outil ; wrappers web et desktop branchés ; `new-tool --custom` ; `check` exige ui.rs/feature/porte cfg (3 tests). Outil jetable `text.zz-probe` : tests SSR du hook, `check` OK, wasm compilé (1142 Ko) et rendu vérifié dans le navigateur (UI custom, pas AutoTool), binaire macOS x86_64 compilé et lancé sans erreur ; outil supprimé ensuite. Non prouvé : fenêtre desktop inspectée, `run`/annulation exécutés de bout en bout, Windows/Linux. Les 38 outils restent `auto` ; l’UI QR spécialisée relève de 7.6. |

### 5. Design system et UI communes

Référence : §§4, 6.4.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 5.1 | Générer CSS et tokens depuis design/tokens | Présent | Oui — fichiers et pipeline | CSS partagée Astro/Dioxus et génération xtask tokens constatées ; vérifier toutes les constantes Rust attendues. |
| 5.2 | Compléter les composants Dioxus partagés | Partiel | Non | Inputs, boutons et AutoTool existent ; fenêtres, menus, onglets, toasts, dialogues, sliders, skeletons à compléter selon usages réels. |
| 5.3 | Respecter Simple/Medium/Pro sur web et desktop | Partiel | Non | Modes présents dans l’UI ; cohérence globale/par outil, préférences de compte et usages desktop à recetter. |
| 5.4 | Centraliser textes UI, erreurs FR/EN et états | Partiel | Non | Locales outils contrôlées ; apps et écrans de compte ne disposent pas encore d’une source Dioxus commune. |
| 5.5 | Compléter clavier, focus, taille tactile et contrastes WCAG AA | Partiel | Non | Quelques recettes Chromium ; audit systématique et appareils réels manquants. |
| 5.6 | Finaliser polices locales et typographie demandée | Inter auto-hébergée ; monospace en repli système | Non — monospace et recette multi-OS absentes | `design/fonts/inter-latin-wght-normal.woff2` (@fontsource-variable/inter 5.3.0, sha512 vérifié, OFL-1.1) intégrée en `data:` par `cargo xtask tokens` (kortex.css ≈ 73 Ko) ; CSP des trois hôtes avec `font-src 'self' data:`. Vérifié dans le navigateur sur le site construit et sur un bundle d’outil reconstruit (`text.cleaner`) : `Inter Variable` en statut `loaded`. Non fait : JetBrains Mono, sous-ensemble latin-ext, rendu desktop/Safari, CSP appliquée en production (en-têtes générés à la publication). |
| 5.7 | Respecter thème et mouvement réduit dans chaque enveloppe | Partiel | Non | Thèmes et Stage présents ; validation commune desktop/mobile/apps à effectuer. |

### 6. Site Astro et hébergement

Référence : §8.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 6.1 | Conserver site vitrine/SEO en Astro/TypeScript | Présent | Oui — conforme au prompt | Cette couche est explicitement prévue ; ne pas la réécrire en Rust par confusion avec les apps installables. |
| 6.2 | Générer catalogue, univers, outils et variantes depuis le registre | Présent | Oui — build et sources | 38 outils ; build courant attendu à 206 pages ; génération ne nécessite pas de modifier une page par outil. |
| 6.3 | Conserver Stage comme intégration du site | Partiel | Non | Iframe isolée, ouverture, plein écran, fichiers et thème présents. Le Stage Astro est une enveloppe site autorisée ; shell partagé Rust encore manquant. |
| 6.4 | Ajouter les vrais liens Installer / Ouvrir dans Desk quand disponibles | En attente | Non | Ne pas afficher de bouton opérationnel vers une app ou une release indisponible. |
| 6.5 | Compléter navigation desktop/mobile et accueil | Partiel | Non | Navigation responsive présente ; mega-menu, menu mobile prévu, démonstrations réelles Desk/Flow non livrés. |
| 6.6 | Finaliser SEO : JSON-LD, hreflang, aliases, sitemap, OG | Partiel | Non | Plusieurs éléments présents ; HowTo/BreadcrumbList et redirections aliases à vérifier et compléter. |
| 6.7 | Valider PWA et caches sans données privées | Outils : recettes ciblées ; apps : PWA installables | Non — matrice appareils/offline complète absente | **Apps** (lot 14) : `xtask build-app --web` écrit `manifest.webmanifest` (nom/description du contrat, couleurs des tokens), icônes PNG 192/512/180 composées depuis le glyphe (teinte dérivée du token d’accent), et un `sw.js` propre aux apps (gabarit `web/shell/sw.template.js` : navigation réseau d’abord avec coquille en secours, fichiers listés en cache d’abord, moteurs de Mind mis en cache à la demande, uniquement GET de même origine). `tests/e2e/pwa.mjs` sur les 6 apps : manifeste valide, icônes 192/512 aux bonnes dimensions, service worker qui contrôle la page, ouverture hors ligne en FR et EN. Limites : Chromium seul (pas Safari/Firefox/mobile), icône « maskable » absente, pas de test d’invite d’installation réelle. |
| 6.8 | Publier les apps Dioxus sur leurs hôtes prévus | Publié pour les six apps | Non — recette complète par app et desktop absentes | Hôtes ouverts le 2026-10-18 : workspace (Desk), dashboard (Hub), store (Store), **search (Seek), flow (Flow), agent (Mind)** ; bundles issus du run de release 39156702154 (commit 75e82b7), 38 outils et 6 apps vérifiés par reçus, CSP/en-têtes générés. Mind vérifié sous la CSP réelle : page, police Inter, moteur `text.cleaner` exécuté dans un Worker de son origine. Distribution desktop toujours fermée. |
| 6.9 | Disposer d’un preview et d’un retour arrière reproductibles | Partiel | Non | Publication locale présente ; preview et rollback de versions cohérentes API/DB/assets à formaliser. |
| 6.10 | Mesurer performance et respecter les objectifs | Non mesuré globalement | Non | LCP <1,2 s, CLS <0,02, INP <100 ms, Lighthouse ≥98 sont des cibles, pas des résultats acquis. |
| 6.11 | Compléter légal et liens de contact | Bloqué partiellement | Non | admin@kortex.site présent ; adresse postale manquante, mentions légales toujours brouillon/noindex. |

### 7. Catalogue existant et preuves de compilation

Référence : §§6–7, 12, 14–15 ; gel utilisateur.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 7.1 | Conserver les 38 moteurs Rust existants | Présent | Oui — inventaire et check courant | 38 engine.rs, contrats et fichiers de tests ; voir matrice ci-dessous et ARTIFACT_AUDIT.json. |
| 7.2 | Conserver les 38 bundles UI WebAssembly constatés | Reconstruits | Oui — compilation web courante | 38 UI et 3 moteurs Worker reconstruits, publiés ; caches et reçus vérifiés. Ne valide pas les 38 recettes complètes ni les cibles natives. |
| 7.3 | Établir source → build → artefact pour chacun des 38 | Réalisé pour le web | Oui — périmètre web | 38 reçus source/sortie vérifiés ; 37 compilations évitées et 1 remise à jour au contrôle final. SHA256 sources, fichiers générés, verrous et artefacts avec versions dans validation/2026-10-17-platform-boundary.json. Desktop complet reste en 7.4. |
| 7.4 | Compiler les 38 interfaces desktop et les paquets requis | Très incomplet localement | Non | Un seul outil a un binaire desktop local constaté : secure.password-generator sur x86_64-apple-darwin. Les rapports CI historiques ne prouvent pas les 38 versions actuelles. |
| 7.5 | Recetter les 38 outils sur web et desktop | Partiel | Non | Pilotes et plusieurs lots ont des tests ; aucune matrice complète de recettes actuelles, modes, langues, erreurs et plateformes. |
| 7.6 | Fermer les fonctionnalités manquantes des six pilotes | Partiel | Non | HEIC, QR Wi-Fi/vCard, UI spécialisée QR, taux vieillesse/APEC et limites lourdes restent à vérifier selon contrat et promesses effectives. |
| 7.7 | Tester lots, paramètres, annulation et export selon chaque contrat | Partiel | Non | Établir la matrice par outil ; pas de statut stable tant que les sorties de qualité demandées ne sont pas validées. |
| 7.8 | Maintenir tous les outils en bêta jusqu’aux preuves | Appliqué | Oui — registre actuel | 38 beta, aucune validation stable déduite d’un simple build ou test moteur. |
| 7.9 | Reprendre la cible historique 150+ outils | Suspendu | Non — volontaire | Consigne de gel prioritaire. Ne pas recréer un catalogue étendu pendant la remise en conformité. |

### 8. Compte et backend

Référence : §§9.5–9.6, 10.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 8.1 | Maintenir Workers TypeScript strict et D1 | Présent | Oui — structure prévue | services/api ; TypeScript est explicitement requis pour le backend, indépendamment des clients Rust. |
| 8.2 | Inscription/connexion mot de passe, sessions et suppression/export | Présent publié | Oui — recettes consignées | Parcours QA public réussi sur la nouvelle base vide demandée le 2026-10-17 : inscription, connexion, réglages, export, suppression ; zéro compte après nettoyage. [Preuve](validation/2026-10-17-fresh-account-database.json). API tests historiques ; ne signifie pas que le client Rust est terminé. |
| 8.3 | Passkeys et gestion de sécurité | Présent web/API | Oui — portée ciblée | Signature ES256, origine/RP, UV, challenge unique, rotation et révocation testés ; vrais appareils Safari/Firefox et client Rust restent non validés. |
| 8.4 | Créer le client Rust kortex-auth réutilisable | Client de session web Rust présent | Non — auth complète/native/passkeys encore absentes | kortex-auth reconnaît le cookie HttpOnly existant via plateforme, profil typé et contrôle du compte attendu ; kortex-pkg reste distinct. Formulaires Rust, passkeys et jetons natifs/trousseau non livrés. |
| 8.5 | Stocker les jetons desktop dans le trousseau système | Écart important | Non | credentials.json en clair avec permissions Unix 0600 : ne satisfait pas le trousseau demandé. Audit et séquence 8.5.1–8.5.6 dans KEYCHAIN_MIGRATION.md ; backends keyring natifs, migration/relecture/rollback, diagnostics masqués et erreurs typées implémentés ; gate local réussi et premiers allers-retours OS réussis, mais preuves finales incomplètes et relance annulée. Travail en pause ; correctif non publié. |
| 8.6 | Activer Turnstile sur formulaires sensibles | Bloqué | Non | Validation préparée, clé publique vide, API Cloudflare 403 avec le jeton courant ; rester désactivé sans clé/secret et recette. |
| 8.7 | Décider et fournir vérification e-mail / récupération de compte | Non livré | Non | Fournisseur gratuit à déterminer ; pas d’affirmation d’e-mail vérifié ni de récupération inexistante. |
| 8.8 | Mesurer CPU et charge réels sur le plan gratuit | Non validé | Non | Succès de requêtes ne constitue pas une mesure de CPU sous charge ; campagne limitée à prévoir avant ouverture large. |
| 8.9 | Contrôler CSRF/CORS, cookies, rétention et taille des entrées | Présent partiellement vérifié | Oui — portée API existante | Tests et recettes consignés, cookie host-only API, rétention quotidienne ; audit sécurité global demeure à faire. |

### 9. Synchronisation et IA communes

Référence : §§2, 5.2, 9.7, 10.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 9.1 | Créer kortex-sync et intégrer les clients Rust | Client Rust de sauvegarde manuelle présent | Non — Sync complet absent | kortex-sync : snapshots typés, schéma/genre/version, limite 64 Kio et API existante ; UI commune Hub/Desk. Pas de continuité automatique ni de client natif authentifié. |
| 9.2 | Fusionner conflits par champ et gérer horodatages | Conflits refusés explicitement | Non — fusion par champ absente | Version attendue vérifiée par API ; aucune relance avec écrasement automatique. Prévisualiser la version courante puis confirmer manuellement ; horodatage/fusion à réaliser. |
| 9.3 | Chiffrer optionnellement de bout en bout les métadonnées | Absent | Non | Clés dérivées localement, migration, perte/récupération et secrets exclus des journaux à traiter. |
| 9.4 | Synchroniser préférences, layouts, workflows, favoris sans fichiers | Sauvegarde/restauration web publiée et recettée | Non — tous clients non couverts | Snapshots explicites des métadonnées Hub/Desk et langue/thème/mode ; aucun fichier, résultat ou compteur local d’usage transmis. Compteurs de l’appareil conservés à la restauration. Prévisualisation/confirmation et refus de restaurer Desk avec fenêtres ouvertes ; pas de Sync automatique. |
| 9.5 | Créer kortex-ai commun avec consentement et streaming | Réalisé (logique sans réseau) | Oui — crate et tests | `crates/kortex-ai` : format du dialogue avec la passerelle, analyse SSE incrémentale et assemblage du tour (texte + appels d’outils fragmentés), définitions d’outils issues des contrats (`a.b` → `a__b`), protocole de références (`@texteN`, `@fichierN`, `@cN.port`) validé strictement contre les contrats (référence inventée refusée, paramètres typés et bornés), consentement par pièce jointe, extraction des textes entre guillemets, résumés d’outils par références, export en workflow Flow, crédits. 23 tests sur les vrais contrats. Les appels HTTP restent faits par les hôtes via kortex-platform ; cache de réponses non fait. |
| 9.6 | Activer et vérifier Workers AI sans dépasser le gratuit | Déployé en production, chat réel non testé en production | Non — conversation réelle en production non faite | Passerelle `POST /v1/ai/chat` sur `@cf/google/gemma-4-26b-a4b-it` (raisonnement coupé, outils, flux SSE, règlement de `usage.neurons`) ; binding `AI` déclaré en production et `remote: true` en dev. Appels réels via `wrangler dev` : réponse simple, appel d’outil par référence, flux de 23 événements ; ligne de base ADR 0023 (79 → 3 jetons sans raisonnement, recopie non fiable sans références). 57 tests du Worker. Worker `api` déployé en production le 2026-10-18 (version 81d085b6, binding `AI` présent, aucune migration) : santé OK, chat et crédits refusés sans session (401). Aucune conversation réelle n’a été faite en production, car l’agent ne crée pas de compte de production ; [preuve](validation/2026-10-18-mind-publication.json). |
| 9.7 | Valider crédits 3 h, budget global, cache et agrégats sans contenus | Partiel — mesuré en réel | Non — compactage, cache et production absents | Réservation puis règlement sur les neurons réels du fournisseur ; flux interrompu = estimation une seule fois ; remboursement en cas d’échec ; aucun prompt ni réponse en base (test). Mesure réelle de Mind : ≈ 7 neurons par appel (prompt système + 8 outils + historique), soit environ 170 appels par seau d’utilisateur et 1 300 appels par jour pour tout le compte gratuit. Cache des réponses et compactage non faits. |

### 10. Distribution et Store Rust

Référence : §§2.12, 7.4, 9.6, 11.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 10.1 | Maintenir CLI Rust headless et moteurs communs | Présent | Oui — structure | kortex-cli lie les mêmes run_map ; compléter NativeIpc et mode --engine séparément. |
| 10.2 | Maintenir signatures Ed25519 et index de paquets | Présent partiel | Non | kortex-pkg et paquets de test constatés ; séquence anti-retour et publication authentique à compléter. |
| 10.3 | Compléter Store dans apps/store avec contrat et UI communes | Catalogue 0.2 compilé et recetté | Non — installation/distribution ouvertes | Interface Rust/Dioxus commune, catalogue contractuel 38 outils, recherche/univers/fiches/permissions et détection native réelle. Web/macOS Intel compilés avec reçus ; recette Chromium FR/EN/mobile passée. Installation publique fermée. ADR 0021, preuve validation/2026-10-17-store.json. |
| 10.4 | Limiter installations et exécutions aux permissions | Partiel | Non | Confirmer consentement par installation, annulation, dossiers explicites et contrôle côté app ; déplacer accès système dans plateforme. |
| 10.5 | Découvrir uniquement les outils réellement installés en desktop | Partiel | Non | Registre local de paquets présent ; vérifier la découverte dans chaque app future, sans remplacer cela par le catalogue web complet. |
| 10.6 | Produire binaires et bundles UI/manifeste par installation | Partiel | Non | Vérifier contenu réel des paquets contre §7.4 ; un binaire seul n’établit pas la coopération entre apps. |
| 10.7 | Recetter Windows/macOS arm64+Intel/Linux, raccourcis et mise à jour | Partiel | Non | Rapports CI historiques à rapprocher des artefacts actuels ; Windows .cmd ne vaut pas vrai raccourci demandé. |
| 10.8 | Publier les binaires seuls et l’index signé | Non ouvert | Non | Distribution publique désactivée ; paquets de test locaux exclus du site. Vérifier dépôt public binaire-only et secrets CI avant publication. |
| 10.9 | Valider installateurs, désinstallation, doctor et mises à jour optionnelles | Partiel | Non | Code existant, tests historiques ; recette à partir d’une vraie release sur trois OS requise. |

### 11. Socle obligatoire des applications installables

Référence : §§1, 5.1, 7, 9.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 11.1 | Définir le contrat app commun avant une nouvelle app | Réalisé — contrat | Oui — périmètre schéma | Contrat app commun, métadonnées/ports/permissions partagés ; vérification FR/EN, UI custom et fichiers obligatoires. Exemple Store validé ; gabarit new-app et exécuteurs restent distincts. |
| 11.2 | Créer les features web et desktop sur les mêmes sources | Présent Hub/Desk/Store | Oui — web, Linux, macOS ARM/Intel | Les trois apps 0.2 compilent les mêmes sources Rust/Dioxus ; preuves CI par cible et reçus web vérifiés. Windows également vérifié au commit ca3d623. Ces preuves précèdent le lot trousseau ; distribution et recette native distinctes. |
| 11.3 | Générer les registres, lancements et paquets depuis app.toml | Partiel | Non | Registre apps.json, métadonnées et packaging all découvrent les contrats ; lancement et paquets des deux cibles restent à compléter. |
| 11.4 | Partager header, shell, bus, auth, sync et composants | Socle partiel utilisé | Non — interfaces complètes ouvertes | Hub/Desk utilisent shell/UI/bus ready-init et clients Rust de session/snapshots explicites. Auth native, transferts typés, synchronisation automatique et autres hôtes restent absents. |
| 11.5 | Prouver un parcours web + desktop minimal avant de poursuivre | Parcours d’installation desktop prouvé sur macOS Intel | Non — Windows/Linux/ARM et recette en fenêtre absents | Test `crates/kortex-cli/tests/install_apps.rs` : les 6 apps construites (Hub, Desk, Store, Seek, Flow, Mind) sont empaquetées, listées par un index signé (clé jetable), installées par la vraie commande `kortex install`, vérifiées (signature, empreinte, binaire identique octet pour octet, exécutable), lancées (toujours actives après 4 s) puis désinstallées sans reste. Web : PWA (6.7). Distribution publique toujours fermée. |

### 12. Kortex Seek Rust

Référence : §9.4 ; phase 6.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 12.1 | Distinguer palette de recherche du site et application installable | Réalisé | Oui — séparation et app présente | La palette du site reste une intégration Astro ; `apps/seek` est l’application Rust/Dioxus distincte (contrat `app.toml`, moteur pur, UI commune, tests). |
| 12.2 | Implémenter moteur de recherche local dans Rust partagé | Réalisé (niveau 1) | Oui — moteur testé | `apps/seek/src/engine.rs`, pur : accents/ligatures, mots vides FR/EN, synonymes FR/EN, pluriels, fautes (distance d’édition bornée), mots-clés/intentions/univers pondérés, variantes SEO classées selon ce qu’elles ont de spécifique, ordre des mots. 9 tests ; vérifié sur les 38 contrats réels (33 variantes) par la recette navigateur. Non couvert : recherche sémantique, classement appris, accès à d’autres langues que FR/EN. |
| 12.3 | Construire UI Dioxus et intégration clavier commune | Web et desktop compilés, recette web réussie | Non — recette desktop en fenêtre et clavier avancé absents | Même UI Dioxus pour web et desktop (champ de recherche, suggestions tirées des intentions des contrats, résultats avec lien, état vide). `tests/e2e/seek.mjs` : 7 recherches FR+EN (png en jpg → variante PNG→JPG, jpg en png, mdp, faute « fusioner pdf », phrase longue, anglais), état vide, suggestions, aucune erreur de page. Recette exécutée aussi sur GitHub (run ci 39096560340, job web-changed, Linux) après reconstruction des apps. Binaire macOS Intel compilé et lancé sans erreur ; fenêtre non inspectée ; Windows/Linux non compilés ici. Pas de validation par Entrée ni de navigation aux flèches (pas de navigation sans accès DOM direct), ⌘K du site et lanceur Desk non reliés. |
| 12.4 | Ouvrir avec paramètres validés par le contrat | Réalisé via les variantes | Non — recette d’ouverture réelle absente | Le lien d’un résultat vise la page de l’outil ou de sa variante (`slug` du contrat) avec `?open=1` ; le préréglage vient de la variante déclarée, aucun paramètre libre. Desktop : lance l’outil installé, sinon message. Non testé : ouverture réelle du Stage depuis l’app, hôte `search.kortex.site` publié le 2026-10-18. |
| 12.5 | Ajouter niveau IA seulement après clients/quotas opérationnels | À faire ultérieurement | Non | Compte, crédits, incertitude du niveau local et paramètres JSON Schema ; pas d’appel IA implicite sur fichiers. |

### 13. Kortex Desk Rust — reprise depuis zéro

Référence : §9.1 ; phase 7.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 13.1 | Commencer dans apps/desk avec app.toml et sources Rust | Sources présentes | Oui — structure et tests ciblés | apps/desk : app.toml, moteur Rust pur, UI Dioxus, locales/contenus/glyphe/tests. Aucune récupération du prototype Astro/TS. |
| 13.2 | Utiliser kortex-ui/shell/bus/platform pour fenêtres et hôtes | Socle réel partiel | Non — hôte natif intégré absent | kortex-shell commun et bus ready/init ; iframe run sur web. Desktop découvre les binaires installés et les ouvre séparément ; IPC/embedding non réalisés. |
| 13.3 | Livrer fenêtres, redimensionnement, plein écran et réduction | Web recetté | Non — recette native complète ouverte | Recette publique réelle : déplacement souris/flèches, resize clavier, agrandissement/restauration et réduction sans remonter les iframes ni perdre le hash. macOS compilé, intégration native complète non validée. |
| 13.4 | Livrer aimantation et dispositions demandées | Partiel | Non — tous critères non couverts | Modèle Rust testé : cascade, moitiés, tiers, quarts et grille ; vue d’ensemble et espaces présents. Aimantation par bord et disposition 2+1 restent à terminer. |
| 13.5 | Intégrer Dock et Seek, registre local desktop | Partiel | Non — Seek absent | Dock et lanceur local basés sur registre ; web 38 outils, desktop uniquement binaires installés. Seek/⌘K reste à intégrer. |
| 13.6 | Ajouter presse-papiers et transferts typés avec adaptateurs | À faire | Non | Ports validés, consentement et compatibilité MIME ; pas de transfert par manipulation fragile du DOM. |
| 13.7 | Ajouter liaisons persistantes et historique de session | À faire | Non | Événements typés, cycles et annulation maîtrisés, fichiers en mémoire locale, secrets jamais synchronisés. |
| 13.8 | Sauvegarder/synchroniser les espaces et reprendre une session | Sauvegarde locale et compte web publiées | Non — Sync automatique/natif absents | Métadonnées seules : espaces nommés et fenêtres. Snapshot compte explicite avec contrôle version/identité ; restauration uniquement après fermeture des fenêtres. Ni fichiers ni résultats restaurés. |
| 13.9 | Calculer les statistiques locales vérifiables | À faire | Non | Outils, fichiers, volume, temps, mémoire si disponible et crédits ; aucun compteur marketing inventé. |
| 13.10 | Compiler, recetter puis publier web + desktop | Web publié ; deux compilations obtenues | Non — distribution et recette native complète absentes | workspace.kortex.site publié et recetté HTTPS ; artefacts wasm/macOS x86_64 liés aux sources par reçus. Windows/Linux, embedding natif et paquets publics non validés. [Preuve](validation/2026-10-17-hub-desk.json). |

### 14. Kortex Mind Rust

Référence : §9.3 ; phase 8.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 14.1 | Créer apps/mind avec contrat et UI Dioxus commune | Réalisé | Oui — structure, tests, compilations | `apps/mind` : `app.toml` (réseau, écriture, `privacy = "ai-consent"`, `bundles_engines`), machine d’états pure, client de transport web/desktop, exécuteur local, UI Dioxus unique, locales/contenus/glyphe, 20 tests. Compilé en web (8,8 Mo avec les 38 moteurs, publié sur agent.kortex.site) et desktop macOS Intel ; binaire lancé sans erreur, fenêtre non inspectée. |
| 14.2 | Livrer conversation streaming, Markdown/code et historique | Réalisé | Non — pas de recette en CI ni en fenêtre native | Flux affiché au fil de l’eau (web : `fetch` en flux ; desktop : HTTP natif en flux, décodage UTF-8 incrémental), Markdown sûr sans HTML (gras, italique, code, listes, titres, liens http/https seulement, 5 tests), historique local persistant (textes seuls, jamais les fichiers). Limites : pas de synchronisation de l’historique, résultats non conservés au rechargement. |
| 14.3 | Générer les outils IA et plans à partir des contrats | Réalisé | Oui — tests et recette réelle | Définitions issues des contrats ; sélection de 8 outils candidats par le moteur de Seek ; plan = lot d’appels validés (outil, références, paramètres typés), affiché avant exécution ; appel invalide corrigé par retour d’erreur au modèle (2 corrections max, 8 tours max). |
| 14.4 | Exiger validation du plan et consentement par fichier | Réalisé | Oui — tests et recette réelle | Aucune exécution sans « Valider » ; confirmations supplémentaires (sensible, réseau, écriture) cochées par appel ; consentement général à l’envoi vers Gemma ; contenu d’une pièce jointe envoyé seulement si l’utilisateur le coche (texte ≤ 4 000 caractères, jamais un fichier binaire) ; les textes entre guillemets restent locaux. Recette réelle : le texte cité et les résultats ne figurent dans aucune requête vers le modèle. |
| 14.5 | Exécuter localement, annuler, récupérer erreurs et produire résultats | Réalisé | Non — annulation d’un outil en cours et fenêtre native non recettées | Web : `engine.wasm` servis par l’app elle-même (même origine) et exécutés en Worker ; desktop : binaires installés en `--engine`. Arrêt du flux modèle et de l’outil (Worker terminé), erreurs traduites, résultats affichés depuis la mémoire locale, fichiers enregistrables. Recette réelle web (3/3) et desktop (vrais binaires, 2,5 s) avec le vrai modèle. |
| 14.6 | Exporter un plan en workflow Flow puis recetter deux cibles | Réalisé, fichiers compris | Non — recette Flow de l’export de fichiers en fenêtre absente | Export JSON versionné : nœuds texte `t…`, **nœuds Fichier** `f…` (ou `b…` pour un lot de fichiers donné à une même entrée), outils `c…`, liaisons ; le type accepté est déduit des pièces jointes, aucun octet ni nom de fichier n’est écrit dans le JSON (l’utilisateur rechoisit les fichiers dans Flow) ; validé par `kortex_workflow::validate` avec les vrais contrats (`pdf.merge` en lot et seul). Deux cibles compilées ; publié sur agent.kortex.site le 2026-10-18 (sans l’export de fichiers, à republier).

### 15. Kortex Flow Rust et connecteurs

Référence : §§2.4–5, 9.2 ; phase 9.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 15.1 | Créer apps/flow et un modèle de graphe Rust versionné | Réalisé (v1) | Oui — modèle, validation et tests | `apps/flow/src/engine.rs`, pur : nœuds texte et outil, liaisons port→port, format JSON versionné (v1, migration prévue, versions futures refusées), validation par les contrats (outil/étape/port inconnus, types incompatibles via `converts_to`, entrée déjà liée, entrée obligatoire non reliée, boucle, paramètre invalide). 10 tests. Limites : pas de déclencheurs ni connecteurs (les nœuds fichier et logiques ont été ajoutés ensuite, voir 15.4). |
| 15.2 | Livrer éditeur nodal Dioxus commun | Canevas nodal réalisé (v2) | Non — fenêtre desktop et navigation clavier complète du graphe absentes | `apps/flow/src/canvas.rs` (modèle pur, 7 tests : géométrie des ports, courbes, zoom ancré/borné, ajustement à l’écran, connexion vérifiée — même nœud, types, boucle —, ajout « en chaîne », historique annuler/rétablir) et `ui.rs` : cartes positionnées (positions optionnelles `pos` dans le JSON, anciens fichiers rangés automatiquement), ports d’entrée à gauche et de sortie à droite, liaisons courbes, glisser une carte, glisser une sortie vers une entrée (cibles valides surlignées, refus muets des liaisons invalides), coupe d’une liaison par sa poignée, fond déplaçable, molette, zoom ±, « Tout voir », « Ranger », mini-carte, palette par univers avec recherche (un clic ajoute l’outil à droite du nœud sélectionné et le relie), panneau de propriétés (texte, entrées par liste pour le clavier, paramètres typés, réessais, résultat), Suppr, Ctrl+Z / Ctrl+Y. `tests/e2e/flow.mjs` : 13 vérifications en navigateur réel au pointeur (chaîne par clic, exécution, déplacement, coupe et reliure, refus, annuler/rétablir, zoom, export avec positions, persistance, import d’un ancien fichier). Ajouts du lot 16 : mini-carte cliquable et déplaçable, sélection multiple (Maj/Ctrl + clic, Ctrl+A), déplacement et suppression de groupe, copier/coller (liens internes conservés, nouveaux identifiants), glisser un nœud de la palette vers le canevas (`ondrop`), palette étendue aux 38 outils plus les nœuds intégrés. 13 + 5 vérifications navigateur. Limites : pas de sélection au lasso, pas de couper, navigation clavier du graphe limitée à Tab entre cartes + liste d’entrées. |
| 15.3 | Exécuter graphe local avec validation, annulation et réessais | Réalisé sur web et desktop | Non — recette en fenêtre et grands graphes absents | `kortex_workflow::Execution` pilote l’exécution pas à pas (validation d’abord, ordre topologique, paramètres typés, adaptateurs, réessais par nœud, annulation, journal, arrêt au premier échec). Flow consomme ce pilote avec `runner.rs` : moteur `engine.wasm` dans un Worker de son origine sur le web (Flow embarque maintenant les 38 moteurs, `bundles_engines = true`), binaire installé en `--engine` sur desktop. L’état de chaque carte est montré en direct (attente, en cours, réussi, échec) et le résultat se lit dans le panneau ; fichiers produits téléchargeables. Preuve : `text.cleaner` → `text.case-converter` → `text.slug-generator` exécutés dans Chrome (résultat `hello_kortex`). Limites : délai 120 s par nœud, annulation immédiate seulement sur le web, Windows/Linux non testés, erreurs des outils non traduites finement. |
| 15.4 | Fournir nœuds système et expressions bornées | Réalisé pour texte : condition, filtre, fusion, délai, modèle, boucle | Non — fichiers en boucle, variables globales, déclencheurs absents | `kortex-workflow::logic` : **Modèle de texte** (`{a}`, `{b}`, `{c}` avec filtres `upper, lower, trim, length, lines, first_line`, accolades échappées, sortie bornée à 1 M caractères, aucun calcul ni boucle dans le modèle), **Pour chaque ligne** (exécute un outil texte→texte une fois par ligne, 200 lignes maximum, réglages `clé=valeur`, arrêt au premier échec, annulation entre deux lignes, réessais par ligne) en plus de Condition, Filtre, Fusion et Délai ; branches non prises ignorées sans échec. 13 tests moteur et 2 recettes navigateur (modèle, boucle sur le vrai moteur `text.case-converter`). Restent : boucles sur des fichiers, variables partagées, nœuds déclencheurs.
| 15.5 | Distinguer déclencheurs web et desktop | À faire | Non | Web : onglet ouvert ; desktop : planification, surveillance et webhooks selon capacités réelles. Aucune promesse de fond web. |
| 15.6 | Créer contrats et modules de connecteurs extensibles | Contrat et 4 connecteurs de base réalisés | Non — OAuth non branché, pas de Gmail/Drive/Dropbox/Notion/Slack | Contrat `apps/flow/connectors/<nom>/connector.toml` (ADR 0026, `kortex_workflow::connector`) validé à la lecture (HTTPS, placeholders déclarés, filtres connus, FR + EN) ; connecteurs **Webhook**, **Telegram**, **Discord**, **GitHub** (6 actions) exposés comme outils `connector.<id>.<action>` du catalogue ; secrets d’appareil résolus avant l’insertion des valeurs ; 5 tests Flow + 1 test de contrat, recette navigateur (Telegram : texte d’un nœud + secret → requête exacte vers le proxy). Guide `docs/guides/add-connector.md`. Restent : jetons OAuth depuis l’interface, autres services.
| 15.7 | Fournir OAuth PKCE et broker minimal si indispensable | Broker réalisé (sans secrets fournisseurs) | Non — PKCE côté client et secrets de production absents | `routes/oauth.ts` (ADR 0025) : `exchange` et `refresh` pour six fournisseurs, secrets en variables Worker, `redirect_uri` limitée à nos origines ou à la boucle locale, jetons non stockés ni journalisés, seuls cinq champs relayés ; 4 tests (fournisseurs configurés, échange avec secret client, Basic/JSON pour Notion, refus avant tout appel réseau, erreur fournisseur réduite à son code). Aucun secret n’est configuré : le broker répond indisponible. Non déployé en production.
| 15.8 | Fournir proxy réseau authentifié et limité pour le web | Réalisé et déployé | Non — recette contre un vrai compte connecté absente | `routes/proxy.ts` (ADR 0025) : session obligatoire, liste blanche de connecteurs et mode libre GET/HEAD, anti-SSRF (IP littérales sous toutes écritures, noms internes, `kortex.site`, port ≠ 443, identifiants dans l’URL, redirections revalidées), limites de taille/débit/délai, enveloppe JSON ; 7 tests dont 15 destinations refusées sans aucun appel réseau. Déployé le 2026-10-19 (refus sans session vérifié en production). Le nœud HTTP de Flow l’utilise dans le navigateur (ADR 0026) ; sur ordinateur, requêtes natives avec la même politique.
| 15.9 | Intégrer nœuds IA avec consentement et crédits | Nœud Agent réalisé (Gemma, 9 tâches, 4 formats, outils locaux) | Non — recette contre la production, quotas réels et fenêtre desktop absents | ADR 0027 : nœud **Agent IA** (source, contexte, tâche, consigne, format texte/JSON/tableau/liste, champs, langue, outils autorisés, étapes) ; logique pure `kortex_ai::agent` (8 tests : instructions FR/EN, données balisées et balise fermante neutralisée, lecture JSON/tableau/liste, réparation unique, boucle d’outils par références, limite d’étapes, nom de sortie traduit) ; transport partagé `kortex-ai-client` (Mind et Flow) ; consentement explicite mémorisé, compte et crédits affichés ; outils réservés aux outils sûrs. Recettes : 5 vérifications navigateur sur passerelle simulée (`tests/e2e/flow.mjs`) et `tests/e2e/flow-ai.mjs` avec le **vrai Gemma** (résumé, extraction JSON, tableau, classification, outil local par référence). Limites : une seule langue de modèle, source ≤ 40 000 caractères, pas de fichiers ni d’images envoyés au modèle, pas de mémoire entre exécutions, pas de streaming affiché dans le nœud.
| 15.10 | Livrer import/export, modèles, synchronisation et deux builds | Import/export (positions comprises), deux builds ; modèles et Sync absents | Non — modèles, Sync et recette native absents | Export/import JSON versionné (refus d’un fichier invalide ou plus récent), compilation web et desktop macOS Intel, binaire lancé sans erreur. Bibliothèque « Mes workflows » (enregistrer, ouvrir, dupliquer, supprimer avec marqueur de suppression, travail en cours conservé automatiquement) et 12 modèles prêts à l’emploi FR/EN (nettoyage, listes, fichiers, PDF, IA, Telegram) réalisés au lot 22, testés dans Chrome ; Synchronisation chiffrée de bout en bout entre appareils réalisée au lot 25 (ADR 0028) ; historique des exécutions (lot 23) et sous-workflows (lot 24) ajoutés. |

### 16. Kortex Hub Rust et Kortex ID

Référence : §9.5 ; phase 10.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 16.1 | Reconnaître le Hub actuel comme prototype web non conforme à l’app cible | Retiré | Oui — sources et recette publique | Hub.astro, Favorite.astro, hub-state/browser, tests, ADR et suivi des récents Stage supprimés ; retrait vérifié en Chromium public, anciennes clés effacées, vitrine noindex en préparation. Aucune récupération de cette implémentation pour la future app. |
| 16.2 | Développer apps/hub Rust/Dioxus à partir du socle commun | Sources présentes | Oui — structure et tests ciblés | apps/hub : même modèle Rust/UI Dioxus web et desktop, contrat, locales/contenus/glyphe/tests ; shell partagé. |
| 16.3 | Créer widgets modulaires déplaçables/redimensionnables | Partiel | Non — modularité complète à finir | Quatre contrats widget.toml ; UI de réorganisation, masquage et largeur réglable, état sérialisé. Rendu encore centralisé dans ui.rs, modules de rendu extensibles à extraire. |
| 16.4 | Intégrer favoris, récents, usages et données protégées | Favoris/récents locaux présents | Non — périmètre complet absent | Favoris, récents limités à 12 et compteurs d’ouvertures depuis Hub ; tests modèle. Aucun faux volume/fichier traité ; journal détaillé IndexedDB et agrégats serveur non intégrés. |
| 16.5 | Intégrer compte/profil/sécurité/préférences via kortex-auth | Session et sauvegarde Rust web publiées | Non — profil/sécurité complets absents | Panneau Rust partagé publié/recetté : session/e-mail réels, sauvegarde/restauration explicite et préférences, protection du compte attendu et de la version. Gestion sécurité/formulaires/passkeys reste liée au site ; connexion native et compte complet Rust restent ouverts. |
| 16.6 | Intégrer crédits, installations par machine et workflows disponibles | À faire | Non | Afficher indisponibilités réelles ; aucune donnée ou app fictive. |
| 16.7 | Migrer ou retirer le prototype TS et publier deux cibles | Prototype retiré ; première app Rust web publiée | Non — distribution/recette native complète ouvertes | dashboard.kortex.site réel : favoris/widgets/recherche/récents recettés FR/EN ; stockage refusé géré. Même source compilée macOS ; pas de distribution publique ni validation complète du compte/Sync. [Preuve](validation/2026-10-17-hub-desk.json). |

### 17. CI/CD, compilation et traçabilité

Référence : §11.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 17.1 | Conserver qualité Rust et tests services/site en CI | Validé sur GitHub | Oui — run 39048924068 | Commit c95bf87 : fmt, Clippy, tests workspace, contrôle structurel, build/test site et services tous verts ; debug CI réduit pour respecter le disque du runner gratuit. |
| 17.2 | Compiler les bundles web modifiés dans CI | Release complète réussie | Partiel — détection PR ouverte | Run 39032642157 : 38 outils + trois apps WebAssembly, reçus et sorties vérifiés. Preuve validation/2026-10-17-github-web.json ; filtrage automatique des modifications en PR encore ouvert. |
| 17.2b | Détection ciblée des bundles web en PR | Exécuté sur GitHub, sélection ciblée non démontrée | Non — gain réel non prouvé | `cargo xtask changed --base <réf>` : fichiers → crates → fermeture des dépendances (outil + bibliothèques du wrapper) ; toolchain, verrou, tokens, marque et générateurs de bundles → tout. 7 tests unitaires ; essais réels : kortex-date → 4 outils, kortex-pkg → 3 apps, doc → rien, Cargo.lock → tout. Job `web-changed` (PR, Linux, 45 min max) ajouté à ci.yml, YAML valide ; Run ci 39070786316 (commit b6cf156, PR 1) : job `web-changed` réussi en 13 min ; la sélection a donné `all=true` (xtask/src/build.rs et release.rs modifiés), donc 38 outils et 3 apps compilés : l’exécution est prouvée, une sélection ciblée (ex. un seul outil) reste à observer sur une PR plus petite. Limite : `source_fingerprint` (reçus) hache encore tout xtask/src, le verrou et presque toutes les crates, donc la réutilisation locale reste trop invalidée (17.7). |
| 17.3 | Valider les apps dans la même CI et les garde-fous | Réalisé — contrôles et builds | Oui — périmètre compilation | Check couvre contrats app, UI/fichiers/plateforme ; release compile les trois apps depuis le registre pour web et desktop. Recette native fonctionnelle distincte. |
| 17.4 | Compiler/recetter le desktop lors des releases groupées | Quatre cibles récupérées et vérifiées | Non — recette OS/installations ouverte | 168 exécutables de release vérifiés : Linux/macOS ARM/Intel du run 39031294481 (7ed41ae), Windows du run 39049702070 (ca3d623), 42 par cible. 84 signatures Mac et index signé Windows vérifiés. Ces artefacts précèdent le lot trousseau ; preuves par cible dans validation/. |
| 17.5 | Finaliser déploiement reproductible et preview | Préparation CI validée | Non — secrets et publication CI ouverts | Run 39053032032 vert : artefact public récupéré, 223 fichiers SHA-256 et cinq configurations vérifiés, provenance commit de merge PR conservée. Publication statique séparée prévue avec reçus de versions et 41 contrôles publics ; secrets Cloudflare absents, publication/preview publique non validées. Preuve validation/2026-10-17-static-ci-preparation.json. |
| 17.6 | Documenter publication locale comme secours autorisé | Clarifié | Oui — règle consolidée | RULES.md §8 et MASTER réconciliés : CI nominale, secours local reproductible autorisé, publication des lots déjà autorisée. La CI/CD elle-même reste incomplète (17.5). |
| 17.7 | Optimiser filtres, caches et budget CI réel | Empreintes précises ; budget réel non mesuré | Non — gain CI non observé | `source_fingerprint` ne hache plus tout xtask/src, Cargo.lock et toutes les crates : crate de l’outil, fermeture des dépendances du workspace (mêmes règles que `xtask changed`), bibliothèques du wrapper pour un outil, entrées du verrou atteignables, fichiers xtask générateurs. Mesuré sur `text.cleaner` : inchangée après édition de kortex-pkg, d’un autre outil, d’un fichier xtask sans rapport ; changée après kortex-ui ou son propre moteur. `xtask fingerprint <id>` pour l’inspecter ; 2 tests du verrou. `xtask ci-budget` donne des plafonds (release 3715 min, credentials 1830 min, non mesurés) ; durées réelles de runs à relever avant d’ajuster. |
| 17.8 | Créer une preuve par lot avant statut validé | Appliqué aux lots actuels | Oui — périmètre documenté | Preuves datées Store, Hub/Desk, frontières plateforme, web CI et cibles desktop ; commit/run, empreintes, recette, publication et limites explicités. Aucune fermeture globale du projet. |
| 17.9 | Smoke e2e Playwright sur trois pages clés | Présent, réussi en CI | Oui — smoke de trois pages | `tests/e2e/smoke.mjs` (`pnpm e2e`) sert `web/site/dist` en local : accueil + palette, page outil FR et EN (titre, hreflang, JSON-LD, bouton Stage), aucune erreur de page ni réponse ≥ 400, hôtes externes bloqués. Réussi en local avec Chrome installé ; échec vérifié en altérant un titre attendu. `playwright` 1.63.0 épinglé (docs/deps.md). Étapes ajoutées au job `site` de ci.yml (Chromium via `playwright install --with-deps`) : job `site` réussi dans le run 39070786316 (3 min). Ne remplace pas les recettes de production de scripts/tests. |

### 18. Finition, audit et lancement

Référence : §15 phase 12.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 18.1 | Fermer d’abord les écarts structurels de cet audit | Prioritaire | Non | Ordre imposé ci-dessous ; aucun nouveau Desk/Hub/Flow/Mind en TS. |
| 18.2 | Constituer matrice web/desktop/mobile/FR/EN/modes | À faire | Non | Indiquer pour chaque outil et app : compilé, exécuté, erreurs/annulation, recette système et preuve. |
| 18.3 | Auditer sécurité, dépendances et confidentialité | Partiel | Non | API et en-têtes ciblés vérifiés ; secrets, plateformes, permissions, stockage, chaîne de release et CSP des apps restent à auditer. |
| 18.4 | Recetter Safari iOS, Android Chrome, Firefox et trois OS desktop | Non complet | Non | Chromium automatisé ne vaut pas recette de tous ces environnements. |
| 18.5 | Finaliser légal, RGPD, restauration et retour arrière | Partiel / bloqué | Non | Adresse postale manquante ; export/suppression présents ; procédures et restauration réelle à éprouver. |
| 18.6 | Mesurer performances et qualité visuelle/accessibilité | À faire | Non | Tests utiles ciblés ; ne pas multiplier les captures à chaque micro-édition, mais une recette complète en fin de lot. |
| 18.7 | Configurer monitoring agrégé et budgets sans données privées | À faire | Non | Aucun contenu utilisateur dans erreurs/télémétrie ; analytics sans cookie seulement si explicitement configurées. |
| 18.8 | Déclarer le projet terminé uniquement après critères de sortie | Non validé | Non | Aucune phase complète n’est déclarée close par cet audit ; toutes les lignes nécessaires doivent avoir leur preuve et leur validation. |

## Matrice factuelle des 38 outils

Inventaire sur disque dans [ARTIFACT_AUDIT.json](ARTIFACT_AUDIT.json), complété le 17 octobre 2026 par la reconstruction des 38 UI web, les 38 reçus vérifiés et les [empreintes du lot](validation/2026-10-17-platform-boundary.json). « Oui » ici signifie uniquement fichier/artefact constaté. La validation fonctionnelle complète web + desktop reste **Non pour chacun**, tant que la matrice de recette n’est pas close. Les 38 outils restent bêta. Les artefacts actuels Linux et macOS ARM/Intel du run 39031294481 sont récupérés et vérifiés (126 paquets, versions/cibles/empreintes et 84 signatures macOS). Windows : 42 paquets récupérés, contrôlés et extraits du run corrigé 39049702070 (ca3d623). Total de release vérifié : 168 exécutables, antérieurs au lot trousseau en développement.

| Outil | Moteur Rust / contrat / tests présents | UI wasm présente | Moteur wasm séparé présent | Reçu web présent | Artefacts desktop CI récupérés et vérifiés | Validation complète |
|---|---|---|---|---|---|---|
| `calc.age` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.average` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.compound-interest` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.date-add` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.date-difference` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.fuel-cost` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.gross-net-salary` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.loan` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.percentage` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.rule-of-three` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.tip` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.unit-price` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `calc.vat` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.base64` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.chmod` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.html-entities` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.json-formatter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.markdown-table` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.number-base` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.url-encoder` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `code.uuid-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `color.contrast-checker` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `color.converter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `convert.timestamp` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `convert.units` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `image.convert` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `pdf.merge` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `qr.generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `secure.hash-calculator` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `secure.password-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.case-converter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.cleaner` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.deduplicate` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.find-replace` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.lorem-ipsum` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.slug-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.sort-lines` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |
| `text.word-counter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel, Windows x86_64 | Non |

## Définition d’une prochaine étape validée

Pour chaque identifiant `N.M`, inscrire les fichiers concernés, la cible compilée et son reçu/version, le contrôle indispensable exécuté et son résultat, la recette réellement faite, puis les limites restantes. Une étape structurelle exige un contrôle de structure ; une étape fonctionnelle exige une exécution ; une étape multiplateforme exige ses différentes cibles. Les déclarations historiques restent des déclarations tant que leur portée n’a pas été rapprochée des sources/artefacts actuels.

Le suivi courant du retrait et de sa publication est dans [PROGRESS.md](PROGRESS.md). Les constats d’écart et l’inventaire de suppression sont dans [ARCHITECTURE_AUDIT.md](ARCHITECTURE_AUDIT.md).
