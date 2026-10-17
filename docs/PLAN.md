# Plan de réalisation et de remise en conformité — Kortex

État au 17 octobre 2026. Référence : [MASTER_PROMPT.md](MASTER_PROMPT.md), relu intégralement, avec les consignes utilisateur plus récentes (gel des 38 outils, publication par lot, suppression des implémentations non conformes, règles réconciliées dans RULES.md). Ce fichier remplace les listes contradictoires de l’ancienne ROADMAP comme suivi opérationnel.

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
| 1.6 | Garder budget gratuit, dépôt privé et licence propriétaire | Partiel | Non — contrôle opérationnel restant | LICENSE et ADR présents ; plan gratuit rapporté au dernier inventaire. Auditer visibilité et quotas à chaque publication de nouvelle ressource ; aucune dépense. |
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
| 2.5 | Générer une nouvelle app par xtask new-app | Absent | Non | Implémenter un gabarit réel Rust/Dioxus, avec features web/desktop, contrat et tests ; pas de dossiers vides. |
| 2.6 | Compléter xtask dev pour web et desktop | Absent | Non | Commande absente de xtask/src/main.rs ; prévoir lancement reproductible par ID et cible. |
| 2.7 | Centraliser tokens, noms et domaines | Partiel | Non | Sources de tokens et marque présentes ; relever toutes les valeurs/destinations dupliquées dans site, packaging et générateurs. |
| 2.8 | Documenter architecture et recettes d’extension | Partiel | Non | Guide add-tool présent ; compléter add-app, add-universe, add-widget et add-connector après validation du socle. |

### 3. Plateforme, exécution et fichiers

Référence : §§5.2, 6.3, 7.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 3.1 | Rendre les moteurs indépendants de l’UI et du système | Présent pour les outils contrôlés | Oui — contrôle statique actuel | Moteurs Rust + scan xtask ; approfondir les appels indirects et dépendances avant clôture. |
| 3.2 | Faire passer tous les accès système runtime par kortex-platform | Réalisé sur le périmètre Rust audité | Oui — sources et parcours ciblés | ADR 0015/0016 : natif, runtime, UI et entrées générées migrés ; check, 220 tests Rust, wasm UI/runtime, 38 bundles web et un desktop macOS compilés ; fichier/copie/téléchargement/Worker/aléatoire recettés. [Empreintes et versions](validation/2026-10-17-platform-boundary.json). Scan textuel non exhaustif ; interfaces complètes et permissions restent en 3.3/10.4. |
| 3.3 | Fournir les backends plateforme web et desktop effectifs | Partiel | Non — stockage et presse-papiers ciblés, étape globale ouverte | [Stockage validé](validation/2026-10-17-preference-storage.json), ADR 0017. [Presse-papiers et Copier publiés](validation/2026-10-17-clipboard-results.json), ADR 0018 : 19 tests ciblés, aller-retour AppKit privé, 38 reçus à jour, un binaire macOS compilé, recettes UI locale et Stage public FR/EN réussies. Préférences UI, dialogues, notifications, trousseau, Windows/Linux/Safari et interfaces complètes restent ouverts. |
| 3.4 | Unifier Executor InProcess, WebWorker et NativeIpc | Partiel | Non | Runtime/Cx et exécuteur Worker présents ; pas d’interface complète des trois backends démontrée. |
| 3.5 | Exécuter les trois traitements web lourds dans des Workers | Présent | Oui — preuves ciblées historiques | image.convert, pdf.merge, secure.hash-calculator : trois moteurs wasm séparés constatés ; essais hash/PDF/conversion consignés dans PROGRESS.md. |
| 3.6 | Exécuter les traitements desktop lourds hors thread UI | Écart / non démontré | Non | Le runner desktop généré appelle run_map dans une future UI ; aucun déport natif robuste démontré. Livrer thread/exécuteur + annulation avant validation desktop. |
| 3.7 | Créer kortex-bus typé partagé web/desktop | Partiel | Non — protocole complet absent | Crate kortex-bus présente et handshake ready/init typé avec validation origine/source. Valeurs, transferts, connexions et IPC natif restent à réaliser ; ADR 0019. |
| 3.8 | Créer kortex-shell pour Standalone, Stage, Window, Node, Headless | Partiel | Non — modes complets absents | Crate shell partagée Hub/Desk : frame, catalogue, recherche, préférences et hôte iframe ; tests et deux cibles. Wrappers Standalone/Stage encore distincts, Node/Headless et embedding natif non unifiés. |
| 3.9 | Fournir engine.wasm headless pour chaque outil | Partiel | Non | Seulement les trois Workers lourds ont un moteur séparé constaté ; 38 UI wasm ne signifient pas 38 moteurs headless. |
| 3.10 | Ajouter le mode desktop --engine JSON-RPC/stdio | Absent / non constaté | Non | Le wrapper desktop inspecté ne gère pas ce protocole. La CLI lie les moteurs, ce qui ne remplace pas le mode autonome demandé. |
| 3.11 | Fournir kortex-io : streaming, OPFS, ZIP et signatures MIME | Partiel | Non | Buffers transférables et validations propres aux codecs présents ; service commun et grandes tailles sans copie non livrés. |
| 3.12 | Valider gros fichiers, mémoire, délais, progression et reprise | Partiel | Non | Tests ciblés existants, dont hash 64 Mio ; limites maximales des contrats, progression détaillée et mesures mémoire non validées globalement. |
| 3.13 | Réaliser le spike WebCodecs Rust et décision codec | Absent / non validé | Non | Développer le spike avant kortex-media ; ne pas ajouter de nouveaux outils médias pendant le gel du catalogue. |

### 4. Contrats, registre et garde-fous

Référence : §§6, 12–13.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 4.1 | Générer Inputs/Params/Outputs depuis tool.toml | Présent | Oui — inspection et check | kortex-macros et macro contract! utilisés ; signature #[engine] contrôlée. Renforcer les cas négatifs de compilation si évolution. |
| 4.2 | Unifier le schéma des outils et applications | Réalisé | Oui — tests natifs + compilation wasm du schéma | parse_app/validate_app réutilisent types et règles ; registre apps.json généré ; ancien parseur ad hoc release remplacé. Apps sans ports de sortie autorisées, outils toujours avec sortie obligatoire. |
| 4.3 | Couvrir tous les types de ports et leurs contraintes | Partiel | Non | PortType et ValueMap présents ; plusieurs types deviennent String. Validation complète formats/ranges/secrets à vérifier. |
| 4.4 | Exécuter réellement les adaptateurs sûrs entre ports | Partiel | Non | converts_to décrit des compatibilités ; cela ne prouve pas l’existence des transformations et du transport typé. |
| 4.5 | Produire JSON Schema et définitions IA depuis les contrats | Non constaté | Non | Génération exportable et tests à réaliser avant Mind/Seek IA. |
| 4.6 | Valider les formats de fichiers par signature | Partiel | Non | Codecs image/PDF contrôlent leurs données ; détection générique de tous les ports fichiers non démontrée. |
| 4.7 | Bloquer dépendances, imports croisés et accès plateforme interdits | Partiel | Non | xtask scanne certains fichiers outils ; ne couvre pas assez apps, crates et code généré, ni toutes les dépendances de docs/deps.md. |
| 4.8 | Contrôler versions, ruptures et identifiants stables | Partiel | Non | Unicité présente ; comparaison de contrat avec référence et semver de rupture à compléter. |
| 4.9 | Valider registre et génération CLI sans modifier le site | Présent | Oui — check courant | cargo xtask check valide registre et génération CLI actuels ; conserver cette propriété pour les apps et chaque changement de contrat. |
| 4.10 | Compiler les UI custom quand ui=custom | Pipeline incomplet | Non | Le build web inspecté instancie AutoTool ; établir le branchement ui.rs + use_tool et une preuve web/desktop avant nouvelle UI spécialisée. |

### 5. Design system et UI communes

Référence : §§4, 6.4.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 5.1 | Générer CSS et tokens depuis design/tokens | Présent | Oui — fichiers et pipeline | CSS partagée Astro/Dioxus et génération xtask tokens constatées ; vérifier toutes les constantes Rust attendues. |
| 5.2 | Compléter les composants Dioxus partagés | Partiel | Non | Inputs, boutons et AutoTool existent ; fenêtres, menus, onglets, toasts, dialogues, sliders, skeletons à compléter selon usages réels. |
| 5.3 | Respecter Simple/Medium/Pro sur web et desktop | Partiel | Non | Modes présents dans l’UI ; cohérence globale/par outil, préférences de compte et usages desktop à recetter. |
| 5.4 | Centraliser textes UI, erreurs FR/EN et états | Partiel | Non | Locales outils contrôlées ; apps et écrans de compte ne disposent pas encore d’une source Dioxus commune. |
| 5.5 | Compléter clavier, focus, taille tactile et contrastes WCAG AA | Partiel | Non | Quelques recettes Chromium ; audit systématique et appareils réels manquants. |
| 5.6 | Finaliser polices locales et typographie demandée | Non validé | Non | Ne pas confondre absence de Google Fonts avec Inter auto-hébergée effectivement fournie ; inventaire/fonts et rendu à vérifier. |
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
| 6.7 | Valider PWA et caches sans données privées | Partiel | Non | Recettes Chromium ciblées et tests de routage présents ; matrice appareils/offline complète non réalisée. |
| 6.8 | Publier les apps Dioxus sur leurs hôtes prévus | Absent | Non | Seuls root/run/api sont actifs. workspace/flow/agent/search/dashboard ne sont pas des applications opérationnelles. |
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
| 8.5 | Stocker les jetons desktop dans le trousseau système | Écart important | Non | credentials.json en clair avec permissions Unix 0600 : ne satisfait pas le trousseau demandé. Audit et séquence 8.5.1–8.5.6 dans KEYCHAIN_MIGRATION.md ; backends par OS et migration non implémentés. |
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
| 9.5 | Créer kortex-ai commun avec consentement et streaming | Absent côté Rust | Non | Passerelle API et crédits présents ; client commun, orchestration et consentements à intégrer avant Mind/Seek IA. |
| 9.6 | Activer et vérifier Workers AI sans dépasser le gratuit | Désactivé | Non | Aucun binding de production. Tests de faux modèle ne valident pas un appel réel ou un coût réel. |
| 9.7 | Valider crédits 3 h, budget global, cache et agrégats sans contenus | Partiel | Non | Réservation/règlement D1 testés ; dispositif complet, compactage et intégration réelle AI restent à confirmer. |

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
| 11.2 | Créer les features web et desktop sur les mêmes sources | Présent Hub/Desk/Store | Oui — web, Linux, macOS ARM/Intel | Les trois apps 0.2 compilent les mêmes sources Rust/Dioxus ; preuves CI par cible et reçus web vérifiés. Vérification finale Windows en cours ; distribution et recette native distinctes. |
| 11.3 | Générer les registres, lancements et paquets depuis app.toml | Partiel | Non | Registre apps.json, métadonnées et packaging all découvrent les contrats ; lancement et paquets des deux cibles restent à compléter. |
| 11.4 | Partager header, shell, bus, auth, sync et composants | Socle partiel utilisé | Non — interfaces complètes ouvertes | Hub/Desk utilisent shell/UI/bus ready-init et clients Rust de session/snapshots explicites. Auth native, transferts typés, synchronisation automatique et autres hôtes restent absents. |
| 11.5 | Prouver un parcours web + desktop minimal avant de poursuivre | À faire | Non | Compilation wasm et native du même exemple/app, recette, artefacts identifiés ; condition bloquante pour les phases applicatives suivantes. |

### 12. Kortex Seek Rust

Référence : §9.4 ; phase 6.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 12.1 | Distinguer palette de recherche du site et application installable | Écart de statut corrigé | Non — app absente | La palette TS du site fonctionne et peut rester une intégration ; elle ne valide pas apps/seek Rust. |
| 12.2 | Implémenter moteur de recherche local dans Rust partagé | À faire | Non | Contrat, index généré, accents, tolérance, synonymes et recherches FR/EN dans une bibliothèque/app réutilisable. |
| 12.3 | Construire UI Dioxus et intégration clavier commune | À faire | Non | App web/desktop et lanceur partagé ; tests de recherche pertinents et recette clavier. |
| 12.4 | Ouvrir avec paramètres validés par le contrat | À faire | Non | Aucun paramètre IA non validé ; lier identifiant et préréglage typé. |
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
| 14.1 | Créer apps/mind avec contrat et UI Dioxus commune | Absent | Non | Dépend du socle apps, auth, ai, bus et moteur headless. |
| 14.2 | Livrer conversation streaming, Markdown/code et historique | À faire | Non | Stockage local, synchronisation optionnelle explicite ; ne pas envoyer tout l’historique par défaut. |
| 14.3 | Générer les outils IA et plans à partir des contrats | À faire | Non | Schémas validés, plan visible, refus de l’inconnu et inspection des permissions. |
| 14.4 | Exiger validation du plan et consentement par fichier | À faire | Non | Métadonnées par défaut ; contenu vers modèle uniquement avec consentement précis ; réseau/écriture/sensitive confirmés. |
| 14.5 | Exécuter localement, annuler, récupérer erreurs et produire résultats | À faire | Non | Utiliser exécuteur commun et outils installés desktop ; fichiers jamais traités sur le serveur. |
| 14.6 | Exporter un plan en workflow Flow puis recetter deux cibles | À faire | Non | Contrat de workflow versionné commun ; publication seulement avec artefacts et parcours réels. |

### 15. Kortex Flow Rust et connecteurs

Référence : §§2.4–5, 9.2 ; phase 9.

| Étape | Livrable / action | État | Validée ? | Preuve actuelle ou condition de validation |
|---|---|---|---|---|
| 15.1 | Créer apps/flow et un modèle de graphe Rust versionné | Absent | Non | Contrat app, moteur pur, types des ports et permissions réutilisés. |
| 15.2 | Livrer éditeur nodal Dioxus commun | À faire | Non | Canvas, zoom, mini-carte, alignement, undo/redo et navigation accessible. |
| 15.3 | Exécuter graphe local avec validation, annulation et réessais | À faire | Non | Cycles, ordre, erreurs, données intermédiaires, pas à pas et journal local sans secrets. |
| 15.4 | Fournir nœuds système et expressions bornées | À faire | Non | Condition/boucle/fusion/filtre/délai/variables ; pas d’exécution de code distant. |
| 15.5 | Distinguer déclencheurs web et desktop | À faire | Non | Web : onglet ouvert ; desktop : planification, surveillance et webhooks selon capacités réelles. Aucune promesse de fond web. |
| 15.6 | Créer contrats et modules de connecteurs extensibles | À faire | Non | apps/flow/connectors/<nom>/connector.toml ; Gmail/Drive/Dropbox/Telegram/Discord/Slack/Notion/webhooks selon lots. |
| 15.7 | Fournir OAuth PKCE et broker minimal si indispensable | À faire | Non | Tokens chez l’utilisateur, stockage sécurisé et permissions explicites ; pas de nouveaux services payants. |
| 15.8 | Fournir proxy réseau authentifié et limité pour le web | À faire | Non | Liste blanche et limites ; desktop réseau natif via plateforme. Refuser SSRF et accès internes. |
| 15.9 | Intégrer nœuds IA avec consentement et crédits | À faire | Non | Dépend de kortex-ai et de la validation de production ; aucun contenu fichier envoyé implicitement. |
| 15.10 | Livrer import/export, modèles, synchronisation et deux builds | À faire | Non | JSON versionné, migrations et recette d’un workflow réel web/desktop avant publication. |

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
| 17.3 | Valider les apps dans la même CI et les garde-fous | Réalisé — contrôles et builds | Oui — périmètre compilation | Check couvre contrats app, UI/fichiers/plateforme ; release compile les trois apps depuis le registre pour web et desktop. Recette native fonctionnelle distincte. |
| 17.4 | Compiler/recetter le desktop lors des releases groupées | Trois cibles vérifiées ; Windows à reprendre | Non — vérification Windows et recette OS ouvertes | Run 39031294481 : Linux et macOS ARM/Intel, 126 paquets vérifiés (38 outils + trois apps + CLI par cible), 84 signatures Mac vérifiées. Windows run 39032642157 : 42 paquets construits, vérificateur bloqué par encodage CP1252 ; correctif UTF-8 et relance ciblée. Preuves par cible dans validation/2026-10-17-desktop-*.json. |
| 17.5 | Finaliser déploiement reproductible et preview | Préparation CI validée | Non — secrets et publication CI ouverts | Run 39053032032 vert : artefact public récupéré, 223 fichiers SHA-256 et cinq configurations vérifiés, provenance commit de merge PR conservée. Publication statique séparée prévue avec reçus de versions et 41 contrôles publics ; secrets Cloudflare absents, publication/preview publique non validées. Preuve validation/2026-10-17-static-ci-preparation.json. |
| 17.6 | Documenter publication locale comme secours autorisé | Clarifié | Oui — règle consolidée | RULES.md §8 et MASTER réconciliés : CI nominale, secours local reproductible autorisé, publication des lots déjà autorisée. La CI/CD elle-même reste incomplète (17.5). |
| 17.7 | Optimiser filtres, caches et budget CI réel | Partiel — équivalence ponctuelle prouvée | Non | CI Linux et cache source/sortie ; après correction des seules traductions Hub, réutilisation prouvée par ancien/nouveau hash et entrées .d du compilateur excluant kortex-shell. Invalidation générale encore trop large ; détection ciblée par dépendances et budget CI restent à finaliser. |
| 17.8 | Créer une preuve par lot avant statut validé | Appliqué aux lots actuels | Oui — périmètre documenté | Preuves datées Store, Hub/Desk, frontières plateforme, web CI et cibles desktop ; commit/run, empreintes, recette, publication et limites explicités. Aucune fermeture globale du projet. |

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

Inventaire sur disque dans [ARTIFACT_AUDIT.json](ARTIFACT_AUDIT.json), complété le 17 octobre 2026 par la reconstruction des 38 UI web, les 38 reçus vérifiés et les [empreintes du lot](validation/2026-10-17-platform-boundary.json). « Oui » ici signifie uniquement fichier/artefact constaté. La validation fonctionnelle complète web + desktop reste **Non pour chacun**, tant que la matrice de recette n’est pas close. Les 38 outils restent bêta. Les artefacts actuels Linux et macOS ARM/Intel du run 39031294481 sont récupérés et vérifiés (126 paquets, versions/cibles/empreintes et 84 signatures macOS). Windows a compilé les 42 paquets dans le run 39032642157 ; son contrôle final et la récupération sont relancés dans 39049702070.

| Outil | Moteur Rust / contrat / tests présents | UI wasm présente | Moteur wasm séparé présent | Reçu web présent | Artefacts desktop CI récupérés et vérifiés | Validation complète |
|---|---|---|---|---|---|---|
| `calc.age` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.average` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.compound-interest` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.date-add` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.date-difference` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.fuel-cost` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.gross-net-salary` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.loan` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.percentage` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.rule-of-three` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.tip` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.unit-price` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `calc.vat` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.base64` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.chmod` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.html-entities` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.json-formatter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.markdown-table` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.number-base` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.url-encoder` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `code.uuid-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `color.contrast-checker` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `color.converter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `convert.timestamp` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `convert.units` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `image.convert` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `pdf.merge` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `qr.generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `secure.hash-calculator` | Oui | Oui | Oui | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `secure.password-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.case-converter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.cleaner` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.deduplicate` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.find-replace` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.lorem-ipsum` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.slug-generator` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.sort-lines` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |
| `text.word-counter` | Oui | Oui | Non | Oui | Linux x86_64, macOS ARM/Intel | Non |

## Définition d’une prochaine étape validée

Pour chaque identifiant `N.M`, inscrire les fichiers concernés, la cible compilée et son reçu/version, le contrôle indispensable exécuté et son résultat, la recette réellement faite, puis les limites restantes. Une étape structurelle exige un contrôle de structure ; une étape fonctionnelle exige une exécution ; une étape multiplateforme exige ses différentes cibles. Les déclarations historiques restent des déclarations tant que leur portée n’a pas été rapprochée des sources/artefacts actuels.

Le suivi courant du retrait et de sa publication est dans [PROGRESS.md](PROGRESS.md). Les constats d’écart et l’inventaire de suppression sont dans [ARCHITECTURE_AUDIT.md](ARCHITECTURE_AUDIT.md).
