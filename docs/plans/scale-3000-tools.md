# Kortex — Plan pour 2 000 à 3 000 outils et des éditeurs médias complets

Proposition du 22 octobre 2026, fondée sur le commit `3723779ba48cfe0a495bec885e1fab653e42b5f8` et l’audit local de cette conversation. Ce document propose une évolution ; il ne déclare aucune fonction implémentée ou validée. Il n’autorise ni lancement de milliers d’outils ni remplacement immédiat des contrats existants.

## 1. Objectif et principe directeur

Faire de Kortex une plateforme où un outil peut être un calculateur, un convertisseur, un éditeur photo, un logiciel de montage, un outil audio, un traitement de données ou une application interactive. Chaque outil expose des capacités explicites, possède une présentation adaptée et coopère avec Flow et Mind lorsque ses fonctions sont automatisables.

**Un outil = identité + opérations + types de données + contraintes + présentation + paquets.** L’interface automatique est une présentation possible. Le moteur n’est pas obligé de correspondre à une seule action instantanée.

L’objectif de volume porte sur le catalogue. Il ne signifie pas installer 3 000 outils, charger leurs interfaces ou exécuter leurs moteurs en même temps. Le nombre de tâches lourdes simultanées doit dépendre de la machine et des ressources disponibles.

Le projet reste local pour le traitement des fichiers. Le compte sert aux données autorisées, à la distribution et à l’IA distante facultative. Une requête Flow vers un service externe reste un transfert explicite : elle ne bénéficie pas de la promesse d’un traitement entièrement local.

## 2. Ce qu’on conserve, étend et remplace

| Élément actuel | Décision proposée | Raison |
|---|---|---|
| Logique Rust, UI Dioxus, site Astro, API TypeScript | Conserver | Pas de justification à une réécriture générale |
| Contrats, macros, registre et validation | Étendre et versionner | Décrire plusieurs opérations, projets et ressources |
| ABI v1 à une entrée `run` | Conserver derrière un adaptateur | Les 44 outils continuent à fonctionner pendant la migration |
| Lecteur partagé | Conserver comme hôte ; étendre ses présentations | Formulaire pour les outils simples, interface dédiée pour les éditeurs |
| WASM portable + Wasmtime desktop | Conserver comme voie par défaut | Ajouter des capacités médias natives derrière la plateforme quand les mesures le justifient |
| Registre complet chargé par chaque app | Remplacer progressivement | Index léger puis contrats et paquets à la demande |
| Fichiers entièrement transportés dans des valeurs | Remplacer pour les gros volumes | Références locales et accès bornés aux médias |
| Binaire contenant une copie du lecteur pour chaque outil | Maintenir en compatibilité ; faire évoluer la voie principale | Installer un hôte commun et des paquets indépendants évite la répétition à grande échelle |
| Exécution par thread et attente d’un résultat unique | Étendre vers un gestionnaire de tâches | Progression, arrêt réel, reprise lorsque l’opération le permet |
| Flow/Mind/CLI comme clients des moteurs | Renforcer | Les interfaces dédiées utilisent la même autorité d’exécution |
| Signatures, canaux et protection anti-retour | Conserver et durcir | Ajouter une cohérence entre index, contrat, UI, moteur et dépendances |

Une interface spécifique peut être compilée séparément. Le lecteur générique ne doit pas être recompilé pour chaque petit outil ; un gros éditeur ne doit pas être contraint à un formulaire pour éviter toute compilation d’UI.

## 3. Architecture cible

```text
Interface automatique     Interface photo/vidéo     Flow     Mind     CLI
          \                      |                  |        |        /
           -------- Commandes et opérations typées communes ---------
                                  |
                  Validation + autorisations + versions
                                  |
                    Projets locaux + gestion des tâches
                                  |
        Moteurs WASM | services médias de la plateforme | workers natifs
                                  |
       Médias référencés | lecture par morceaux | caches | exports locaux
```

Le catalogue indique ce qui existe. L’hôte détermine ce qui est installé et compatible. La politique d’autorisation décide ce qui peut être exécuté sur les ressources choisies. Un appel proposé par l’IA ne contourne aucune de ces étapes.

### 3.1 Les notions à distinguer

| Notion | Exemple | Règle |
|---|---|---|
| Outil | Éditeur vidéo | Produit identifiable avec version et présentation |
| Opération | Découper un clip | Capacité appelée par l’UI, Flow ou Mind |
| Projet | Montage avec pistes et clips | État durable, indépendant de la fenêtre ouverte |
| Média | Vidéo source, photo, son | Ressource locale référencée et autorisée |
| Tâche | Export du montage | Travail suivi, éventuellement long |
| Variante | Export vertical pour un réseau social | Préréglage ou recette, pas obligatoirement nouveau moteur |
| Capacité machine | Encodeur disponible, espace temporaire | Détectée au moment de l’exécution |

## 4. Contrat v2 : plusieurs opérations et plusieurs modes d’usage

### 4.1 Description d’un outil

Ajouter au contrat : présentation `auto` ou `custom`, point d’entrée UI facultatif, types de projets pris en charge, opérations exposées, versions de protocole requises, dépendances et ressources facultatives. Les capacités réellement disponibles peuvent différer entre web et desktop ; elles doivent être affichées, jamais simulées.

### 4.2 Description de chaque opération

Décrire son identifiant stable, ses entrées et sorties typées, les contraintes, permissions, effets, ressources nécessaires et possibilités d’annulation/reprise. Distinguer lecture, modification de projet, export et action externe. Indiquer les cas où le résultat est déterministe, ceux où il dépend d’un codec ou du matériel, et ceux où une confirmation utilisateur reste nécessaire.

Une opération peut être :

- courte et sans état : calculer une valeur ;
- liée à un projet : ajouter un clip ou appliquer un réglage ;
- longue : générer des proxies ou exporter ;
- interactive : demander un choix ou une sélection, puis reprendre.

Ne pas exposer « cliquer à x/y » comme interface principale d’automatisation. Exposer l’intention métier : positionner un clip à un instant, modifier un masque, exporter une zone.

### 4.3 Compatibilité

Les outils actuels deviennent des outils v2 possédant une opération `run`, via un adaptateur. Versionner séparément contrat, protocole d’exécution et format de projet. Les fichiers Flow existants conservent leur sens ; les migrations sont testées et n’effacent pas les données originales.

### 4.4 Application des contraintes

Une même validation s’applique aux commandes provenant de l’UI, Flow, Mind et CLI. Vérifier types, enums, limites, références, permissions et compatibilité avant allocation ou exécution. Le contrat ne doit pas être uniquement une aide à dessiner les champs.

## 5. Stockage local et projets

### 5.1 Références de médias

Introduire des identifiants opaques de ressources locales plutôt que transporter les octets complets entre chaque nœud. Le gestionnaire de médias fournit métadonnées, plages de lecture, flux et création de sorties. Les handles sont limités au projet et au droit accordé ; connaître l’identifiant ne doit pas suffire à lire le fichier.

Prévoir : fichiers source déplacés, médias manquants, relocalisation, doublons, stockage externe débranché, quota navigateur, disque plein et nettoyage des temporaires. Le calcul d’empreinte des gros fichiers ne doit pas bloquer leur import : indexation et vérifications progressives selon le besoin.

### 5.2 Un projet indépendant des vues

Un projet contient un manifeste versionné, des références aux médias, les opérations/paramètres de montage, un historique borné et les informations permettant de retrouver les ressources. L’état des panneaux, du zoom et des fenêtres reste séparé des modifications du contenu.

Mettre en place commandes transactionnelles, annuler/rétablir, sauvegarde automatique, restauration après crash, verrouillage ou contrôle de version entre fenêtres et migration de schéma. Une commande IA et une commande manuelle doivent entrer dans le même historique.

### 5.3 Transport et partage entre origines

Conserver l’isolation des outils vis-à-vis du compte. Définir un courtier de ressources dans l’hôte de confiance ; les moteurs isolés demandent des plages ou opérations précises. Tester très tôt les transferts autorisés entre origines, Workers et stockage navigateur. Un stockage privé d’origine ne devient pas automatiquement accessible à une autre origine.

### 5.4 Synchronisation et portabilité

Au départ, sauvegarder/synchroniser uniquement les éléments explicitement permis et bornés. Un projet photo peut contenir masques ou vignettes dérivés de l’image : ces données ne doivent pas être rebaptisées « métadonnées » pour contourner la règle de confidentialité.

Les gros projets ne rentrent pas nécessairement dans les limites Sync actuelles. D’abord garantir sauvegarde locale et export/import de projet. Définir ensuite un éventuel découpage de métadonnées, son chiffrement et ses quotas ; les médias ne seront pas téléversés implicitement. Sur un autre appareil, demander de relier les médias locaux manquants.

## 6. Exécution et maîtrise des ressources

### 6.1 Gestionnaire de tâches commun

Créer un gestionnaire partagé avec identifiant, propriétaire, entrée versionnée, état, progression, résultat et diagnostic. États minimaux : en attente, en cours, terminé, échoué, annulé. Pause/reprise sont annoncées uniquement pour les opérations qui les implémentent.

Les tâches longues disposent d’une file locale, d’une priorité, d’un budget mémoire/disque, d’un délai et d’une politique de concurrence. L’export ne doit pas rendre la timeline inutilisable ; un petit calcul ne doit pas attendre derrière plusieurs exports lourds.

### 6.2 Arrêt réel

Annuler doit libérer le moteur, les décodeurs, fichiers temporaires et ressources GPU. Pour les services natifs, conserver le handle du processus et gérer l’arrêt puis l’attente de fin. Une réponse « annulé » envoyée à l’interface sans arrêter le calcul n’est pas suffisante.

### 6.3 Flux et mémoire

Remplacer le pipeline « tout lire → tout copier → tout traiter » par des flux bornés, une pression retour entre producteurs et consommateurs et des caches à taille contrôlée. Baser les limites sur les ressources effectivement allouées, y compris les buffers hôte, pas seulement la mémoire linéaire WASM.

Les flux et handles sont des capacités de l’hôte ; ils ne doivent pas donner au moteur un accès arbitraire au disque ou au réseau. L’ABI v1 peut rester pour les petits traitements tandis qu’un protocole étendu fournit ces accès contrôlés.

## 7. Interfaces personnalisées

### 7.1 Trois présentations sur le même socle

- Formulaire généré pour les outils simples.
- Présentation spécialisée réutilisable pour une famille : recadrage, comparaison, waveform, sélection de zone.
- Application complète pour montage vidéo, photo à calques, audio multipiste, etc.

Les présentations spécialisées et complètes restent dans la voie Rust/Dioxus commune web/desktop. Canvas/GPU et codecs peuvent utiliser des adaptateurs techniques plateforme ; cela ne crée pas une seconde logique métier en TypeScript.

### 7.2 SDK d’interface

Fournir thèmes, locales, raccourcis, sélection de fichiers, ouverture/sauvegarde de projet, commandes, historique, progression et gestion d’erreurs. L’UI appelle le SDK d’opérations ; elle ne réimplémente pas un second moteur.

Proposer des composants spécialisés quand un usage réel les justifie : timeline, pistes, visualiseur, histogramme, calques, outils de sélection, comparaison avant/après. Les charger uniquement pour les outils concernés.

### 7.3 Distribution et permissions de l’UI

Définir un module UI signé lié à la version de contrat/moteur compatible. Le chargement d’une UI ne lui donne pas les jetons du compte. Décider le mode de montage web/desktop par prototype avant de figer le format : vue isolée, origine, pont de commandes et accès aux aperçus.

Tester clavier, lecteur d’écran pour les commandes essentielles, zoom, tactile, écrans de petite taille, retour de focus et réduction du mouvement. Une timeline accessible demande des commandes alternatives aux gestes de souris.

## 8. Photo, vidéo et audio : moteurs spécialisés

### 8.1 Fondations médias communes

Créer les services communs à partir des pilotes, pas une collection de bibliothèques vides. Ils couvrent inspection de formats, décodage/encodage, géométrie, temps média, métadonnées, couleurs, miniatures, caches et export.

**Web :** WebCodecs lorsque la configuration requise est disponible, traitement en Worker et stockage temporaire local adapté. Tester l’encodeur/décodeur demandé plutôt que supposer sa présence. L’accélération matérielle est une préférence, pas une garantie donnée par le standard. [Spécification WebCodecs](https://www.w3.org/TR/webcodecs/).

**Desktop :** workers/processus médias natifs supervisés par `kortex-platform`, avec backend choisi après mesure. Évaluer bibliothèques Rust, codecs système et FFmpeg selon besoin réel, distribution, versions et conditions de licence. Ne pas rendre un FFmpeg arbitraire trouvé dans le PATH obligatoire sans stratégie de validation.

Le même modèle de projet et les mêmes opérations s’appliquent partout. Les backends peuvent différer ; leurs résultats sont comparés avec des tolérances définies. Une égalité binaire entre GPU/codecs différents n’est pas promise.

### 8.2 Éditeur photo

Prévoir image source immuable, transformations non destructives, calques, masques, sélections et historique. Rendu par tuiles et niveaux de résolution pour éviter de recalculer toute l’image à chaque geste. Aperçu rapide et export pleine résolution utilisent le même graphe d’opérations.

Traiter explicitement orientation EXIF, profils colorimétriques, alpha, profondeur de couleur, métadonnées et politique de suppression. Décider le support RAW/HDR/16 bits par format et backend : ne pas annoncer « tous formats » à partir d’un import réussi.

### 8.3 Éditeur vidéo

Prévoir import/démultiplexage, pistes vidéo/audio, clips, découpage, transitions, titres, miniatures et waveform. Utiliser une représentation temporelle précise, gérer timestamps, timebases, cadence variable et synchronisation audio/vidéo.

Séparer aperçu et export : proxies légers, décodage à la demande, cache de frames borné, recherche à une position, limitation des files d’encodeur. Fermer explicitement les frames et ressources. Ajouter un chemin logiciel lorsque pertinent et exposer les capacités réellement disponibles.

Un export interrompu n’est repris que si le format et le pipeline permettent une reprise correcte ; sinon conserver le projet et repartir d’un segment ou relancer. Distinguer arrêt sûr et reprise universelle.

### 8.4 Audio

Prévoir formes d’onde calculées progressivement, resampling, mixage, effets et export par blocs. Tester dérive temporelle, écrêtage, mono/stéréo, changement de fréquence et longues durées. Les mêmes services servent au montage vidéo et à un futur outil audio.

### 8.5 Autres familles

Le socle doit permettre d’ajouter ensuite documents, données tabulaires, 3D, dessin vectoriel et interfaces métier via leurs types et capacités. Cela ne signifie pas développer tous ces backends pendant la première migration. Un outil strictement interactif peut exposer seulement ses fonctions pertinentes à Flow/Mind ; le contrat doit préciser ce périmètre.

## 9. Flow et Mind deviennent des clients complets des projets

### 9.1 Flow

Un nœud cible une opération versionnée, pas seulement un outil. Ajouter des ports de référence : média, projet, séquence, calque, sélection et résultat de tâche selon les premiers usages. Le graphe transmet les références sans dupliquer les fichiers.

Ajouter attente de tâche longue, sortie d’erreur exploitable, progression, annulation propagée, validation des permissions et reprise depuis un point valide. Les retries automatiques sont réservés aux opérations idempotentes, ou munis d’une clé empêchant les doublons. Un échec ne doit pas ajouter deux fois un clip ni publier deux fois un fichier.

Exemple : importer une vidéo → créer un projet → découper → ajouter une musique → exporter → ouvrir le projet dans l’éditeur pour retouches. Le projet obtenu reste modifiable manuellement.

### 9.2 Mind

Avec 3 000 outils et plusieurs opérations par outil, ne pas injecter tous les schémas dans le contexte du modèle. Faire une découverte en deux temps : chercher les capacités pertinentes, puis fournir les schémas des seules opérations retenues et compatibles avec l’appareil.

Donner au modèle les références et informations nécessaires : sélection courante, durée, dimensions, calques utiles. L’accès local au fichier et son envoi au fournisseur IA sont deux autorisations distinctes. Une vignette, un extrait audio ou une transcription constituent aussi du contenu à traiter explicitement.

L’IA propose des commandes métier validées par l’hôte. Les modifications réversibles entrent dans l’historique du projet et sont visualisables. Les écritures destructives, transmissions externes et autres actions sensibles respectent les confirmations requises, avec possibilité de préautorisation précise pour un workflow.

### 9.3 Passage manuel ↔ automatique

Une fenêtre ouverte et Flow ne doivent pas modifier silencieusement la même révision. Choisir un contrôle de version local ou un verrou de projet pour les mutations. Produire une branche/copie lorsque nécessaire. Permettre à l’utilisateur de visualiser ce qu’a changé une série de commandes IA et de revenir en arrière.

## 10. Catalogue, recherche et site pour 3 000 outils

### 10.1 Index léger

Remplacer le document unique de contrats par :

1. manifeste racine signé, version/sequence et références vers les fragments ;
2. métadonnées légères de découverte ;
3. contrats détaillés chargés par outil/famille ;
4. index de paquets et capacités réellement disponibles.

Les fragments sont référencés par empreinte dans le manifeste signé ou signés individuellement. Publier les objets immuables avant de basculer la racine évite de mélanger les versions.

Le registre actuel de 44 outils fait environ 159 Ko ; une extrapolation linéaire donne environ 10,8 Mo à 3 000 outils. Ce n’est pas une mesure d’un futur catalogue, mais dépasse déjà la limite actuelle de téléchargement des apps, 4 Mio. Augmenter seulement cette limite ne règle pas le chargement inutile de tous les contrats.

### 10.2 Recherche

Conserver une recherche locale rapide, charger les index par besoin et limiter le travail par requête. Prévoir filtres par fonction, formats, plateforme, installation, disponibilité hors ligne et capacités matérielles. Virtualiser/paginer les longues listes. La recherche IA réutilise cette découverte au lieu de disposer d’une seconde liste divergente.

### 10.3 Identité et contenu

Ne pas créer un moteur/crate différent pour chaque préréglage ou variante SEO. Définir outil, opération et variante clairement ; préserver identifiants et anciens slugs. Les familles peuvent partager un moteur tout en gardant des produits et contrats distincts si le besoin utilisateur le justifie.

Générer les pages depuis les mêmes métadonnées, avec descriptions vérifiées, langues, capacités et limites réelles. Aucun faux bouton d’ouverture si l’outil n’est pas livré pour la plateforme.

### 10.4 Hébergement

Budgéter le nombre de fichiers, la taille des modules UI/moteurs, les assets SEO, les anciennes versions et le temps de déploiement. La documentation Cloudflare consultée indique notamment 20 000 fichiers statiques par version de Worker gratuit et 25 Mio par fichier. À 3 000 outils, même dix fichiers par outil dépassent le premier plafond avant le site et ses images. [Limites Cloudflare](https://developers.cloudflare.com/workers/platform/limits/).

Prévoir déduplication et répartition raisonnable des assets par hôtes/partitions, sans un Worker par outil. Valider le routage, les quotas et le cache avec un prototype réel. Conserver le budget zéro comme contrainte mesurée ; il ne constitue pas une promesse de trafic, stockage ou IA illimités.

## 11. Packaging et installation

### 11.1 Paquets modulaires

Découper hôte commun, moteur portable, UI facultative, dépendances médias partagées et ressources volumineuses. Chaque module a version, empreinte, signature/provenance et compatibilité. Ne télécharger que les composants nécessaires à l’appareil et à l’usage.

La voie principale cible un hôte inchangé qui ouvre des paquets vérifiés. Les icônes/raccourcis peuvent ouvrir cet hôte sur un outil précis. Préserver une voie d’application autonome lorsque sa distribution est pertinente.

Cette proposition remplace progressivement le collage systématique d’un payload dans chaque exécutable. Elle ne consiste pas à modifier et signer des exécutables sur la machine de l’utilisateur : la stratégie de signature macOS, les bundles et les lanceurs doivent être validés sur les vraies plateformes.

### 11.2 Transactions et mises à jour

Installer sous verrou, vérifier tous les composants, conserver la version précédente jusqu’au commit, puis basculer atomiquement la référence active. Ne pas casser un projet ou une tâche utilisant une ancienne version : la conserver jusqu’à libération, puis la nettoyer.

Gérer dépendances, absence réseau, réparation, espace insuffisant, annulation, compatibilité et rollback. Installer un outil doit installer son contrat exploitable par Flow/Mind ; ce ne doit plus être une étape best effort silencieuse.

### 11.3 Cache et hors ligne

Budgets séparés pour paquets, aperçus et fichiers temporaires ; éviction explicite avec protection des ressources épinglées ou utilisées. Afficher « utilisable hors ligne » seulement si moteur, UI et dépendances requises sont présents.

### 11.4 Installation directe, raccourcis et identité des applications

Exigences ajoutées par l’utilisateur le 22 octobre 2026, suivies dans le dépôt aux étapes 10.10–10.13 :

- **Store directement** : une commande unique par système installe Kortex Store et ses composants nécessaires, même sans installation Kortex préalable.
- **Kortex en ligne de commande** : proposer aussi un parcours simple qui conserve la gestion actuelle par `kortex`, sans obligation d’installer le Store. CLI et Store partagent paquets vérifiés, registre local et mises à jour.
- **Raccourcis bureau** : permettre leur création depuis le Store et la CLI pour ouvrir directement une app installée, avec son nom et son icône. Vérifier leur fonctionnement après mise à jour et leur retrait à la désinstallation.
- **Identité native** : remplacer « Dioxus App » et le logo Dioxus par le nom et l’icône de l’application dans les fenêtres, les paquets et les surfaces système contrôlées par Kortex. Le lecteur partagé doit également refléter l’outil ouvert.

Recetter les deux installations, leur coexistence, les raccourcis et l’identité sur Windows, macOS et Linux ; inclure chemins avec espaces/Unicode et lancement sans terminal. Les commandes exactes seront documentées après implémentation et validation. Ces ajouts sont planifiés, pas encore livrés.

## 12. Sécurité, qualité et exploitation

### 12.1 Corriger les points déjà trouvés

Le chantier réutilise le rapport `kortex-audit-complet.md` : redirections avec secrets, hasard cryptographique, bornes mémoire hôte, métadonnées, rôle administrateur, concurrence/crédits, installation, erreurs et accessibilité. Chaque correction conserve une preuve et un test adapté ; ne pas tout repousser jusqu’à l’arrivée des éditeurs lourds.

### 12.2 Capacités et isolation

Le moteur portable reçoit seulement les capacités accordées. Les extensions natives ont une surface d’IPC minimale et sont signées ; leur isolation OS doit être décrite et testée. Un processus séparé n’est pas, à lui seul, une sandbox de sécurité. Les règles Wasmtime restent un socle, les appels hôte doivent aussi être bornés. [Sécurité Wasmtime](https://docs.wasmtime.dev/security.html).

Gérer clés de signature, rotation, versions révoquées, retour arrière et refus d’un manifeste corrompu. La prise en charge de modules tiers, si décidée plus tard, demandera une politique de provenance/revue distincte ; elle n’est pas implicitement ouverte par ce plan.

### 12.3 Tests

Combiner tests unitaires, corpus indépendants par format, tests de contrats, interleavings/pannes, tests d’intégration médias, recettes OS et parcours web réels. Vérifier que UI, Flow, Mind et CLI exécutent bien les mêmes commandes.

Ajouter des tests de taille de catalogue et de mises à jour à 3 000 outils. Ne pas confondre 3 000 manifestes synthétiques correctement cherchables avec 3 000 produits réellement validés.

### 12.4 CI et observabilité

Construire et tester la fermeture des dépendances touchées. Une modification de contenu ne reconstruit pas tous les moteurs ; une modification d’ABI déclenche une matrice de compatibilité et les reconstructions justifiées. Les outils en données seules ne nécessitent pas chacun une nouvelle compilation Rust.

Caches d’artefacts par empreinte, tests répartis par familles, builds web ciblés en PR, cibles natives appropriées avant publication et vérification d’une CI verte sur le commit signé. Une mise à jour de dépendance médias partagée doit avoir un rayon d’impact calculé.

Mesurer temps de démarrage, découverte, tâches, mémoire, VRAM lorsque disponible, espace disque, écritures D1 et taille des livraisons. Les diagnostics restent sans fichiers, secrets ni prompts ; les rapports locaux détaillés sont transmis uniquement sur choix explicite.

## 13. Plan d’exécution ordonné

Les numéros ci-dessous appartiennent à cette proposition. Ils ne ferment aucune étape de l’ancien PLAN et ne sont pas encore un engagement de calendrier. Chaque lot doit avoir des contrôles proportionnés et une livraison vérifiable.

| Lot | Étapes | Travail | Sortie vérifiable |
|---|---|---|---|
| **0 — Référence et stabilisation** | 0.1–0.4 | Réconcilier l’état courant, fixer la matrice de capacités cibles, corriger les priorités de l’audit, mesurer le socle actuel | Base de comparaison reproductible et correctifs de sécurité testés |
| **1 — Prototypes d’architecture** | 1.1–1.4 | Prouver UI custom, échange de gros médias entre origines, pipeline vidéo minimal, paquet ouvert par hôte commun | Démonstrations web/desktop et ADR fondés sur mesures ; choix des backends et limites |
| **2 — Contrat v2** | 2.1–2.5 | Opérations, types/références, effets/permissions, protocole versionné, adaptateur v1 | Les 44 outils existants restent exécutables ; un outil expose plusieurs opérations |
| **3 — Ressources et projets** | 3.1–3.5 | Gestionnaire local de médias, lecture bornée, format de projet, commandes/historique, sauvegarde/relocalisation | Projet retrouvé après crash avec médias locaux et annuler/rétablir fiable |
| **4 — Tâches et isolation** | 4.1–4.5 | File, budgets, progression, annulation réelle, supervision native et reprise facultative | Une tâche lourde s’arrête et libère ses ressources ; un petit outil reste réactif |
| **5 — Catalogue et distribution** | 5.1–5.6 | Catalogue fragmenté signé, recherche, paquets modulaires, dépendances, transactions, caches | Ajouter un outil sans reconstruire les six apps ; installation hors ligne vérifiée |
| **6 — SDK d’UI** | 6.1–6.4 | Pont de commandes, vues custom Dioxus, composants réutilisables, accessibilité | Une UI dédiée charge à la demande et produit les mêmes commandes que la CLI |
| **7 — Photo pilote** | 7.1–7.5 | Projet non destructif, aperçu par tuiles, calques/masques essentiels, couleurs/métadonnées, export | Même projet ouvert manuellement ou produit par opérations ; comparaison aperçu/export validée |
| **8 — Vidéo/audio pilote** | 8.1–8.6 | Import, temps média, timeline, proxies, mixage, export natif/web selon capacités | Montage représentatif, lecture/seek utilisables, export A/V synchronisé et arrêt sûr |
| **9 — Flow v2** | 9.1–9.5 | Nœuds d’opération, ports de références, tâches longues, permissions, retries/reprise et migration | Workflow créant un projet photo/vidéo puis exportant sans copier les médias à chaque nœud |
| **10 — Mind v2** | 10.1–10.5 | Découverte en deux temps, contexte de projet, commandes validées, consentement contenu et historique | Demande naturelle → changements visibles dans l’éditeur → annuler → export autorisé |
| **11 — Échelle et migration** | 11.1–11.5 | Catalogue synthétique 3 000, index/SEO/assets, CI ciblée, migration progressive et compatibilité | Mesures selon objectifs ci-dessous et publication de pilotes sur les OS pris en charge |
| **12 — Ouverture progressive** | 12.1–12.4 | Étendre par familles, revue de qualité, corpus et mesures des premières releases | Chaque nouvelle famille prouve son backend avant extension massive du catalogue |

**Dépendances :** 0 → 1 → 2 ; 3 et 4 suivent 2 ; 5 suit les choix 1/2 et peut avancer avec 3/4 ; 6 s’appuie sur 2/3 ; 7/8 utilisent 3/4/6 ; 9/10 peuvent commencer avec les opérations simples de 2 puis se valident sur 7/8 ; 11 valide l’ensemble ; 12 augmente progressivement le nombre de produits.

Les prototypes du lot 1 sont jetables ou limités. Ils ne deviennent pas des éditeurs publiés avant les fondations. La première tranche utile à réaliser est **0 + 1 + 2**, puis **3 + 4** ; pas 3 000 nouveaux dossiers d’outils.

## 14. Objectifs de validation proposés

Ce sont des seuils de travail à ajuster après la mesure de référence, pas des performances déjà obtenues. Fixer les machines de référence, navigateurs, jeux de données et conditions réseau avant de comparer.

| Domaine | Mesure / critère |
|---|---|
| Catalogue | 3 000 outils et 30 000 opérations synthétiques ; index initial de découverte visé ≤ 1 Mio transféré, hors détails chargés à la demande |
| Recherche | Cible p95 ≤ 100 ms après index prêt, sur machine de référence ; mesurer aussi l’indexation, le premier chargement et le mobile |
| Installation | Ajouter un outil simple sans reconstruire Hub/Desk/Store/Flow/Mind/Seek ; aucune réinstallation complète de l’hôte hors incompatibilité déclarée |
| Mise à jour | Échec injecté à chaque étape : ancienne version utilisable ou retour arrière explicite, registre cohérent |
| Photo | Jeu de référence incluant 24–50 MP, alpha, EXIF et couleurs ; mémoire bornée par le budget annoncé et aperçu cohérent avec l’export |
| Vidéo | Jeu de référence 1080p long et extrait 4K ; mesurer durée/mémoire, seek, proxies, A/V, export et compatibilité par backend, sans promettre le 4K sur tous appareils |
| Mémoire | Import et lecture de médias de plusieurs Gio sans allocation proportionnelle au fichier entier ; documenter les formats nécessitant un accès particulier |
| Annulation | Cible retour UI ≤ 200 ms ; cible arrêt/libération du travail ≤ 2 s sur backends compatibles, cas non interruptibles explicitement traités |
| Reprise | Projet récupéré après fermeture forcée ; reprise de tâche testée seulement pour les opérations déclarant cette capacité |
| Flow/Mind/UI | Même série de commandes et même révision de projet ; résultats équivalents selon tolérances documentées |
| Réseau | Aucun transfert de contenu local pendant un traitement déclaré local ; transferts IA/connecteurs vérifiés et explicitement autorisés |
| Production | Signature, provenance, quotas d’hébergement, poids par fichier, compte des assets et recette OS validés avant publication |

## 15. Décisions à figer pendant les prototypes

1. **Format du contrat/protocole v2 :** conserver l’adaptateur v1 et choisir l’extension minimale prouvée par les pilotes.
2. **Chargement des UI personnalisées :** isolation, partage du socle Dioxus, durée de vie et pont de commandes.
3. **Propriété des médias sur le web :** quelle origine détient le projet, qui fournit les plages/aperçus et comment les permissions sont vérifiées.
4. **Backends vidéo/audio :** codecs web requis, comportement de secours et backend natif distribué.
5. **Installation :** hôte commun principal, paquets vérifiés et compatibilité avec les applications autonomes existantes.
6. **Modèle d’état :** commandes, versions, historique et concurrence entre UI et automatisation.
7. **Périmètre de la première photo/vidéo :** fonctions essentielles et formats réellement validables, avant une promesse d’éditeur professionnel complet.
8. **Budget de distribution :** plan d’assets et dépendances compatible avec les services gratuits actuels, validé sur un inventaire réaliste.

## 16. Résultat attendu

Un petit calculateur continue à utiliser une interface générée légère. Un éditeur photo ou vidéo possède sa propre interface et travaille sur un projet local durable. Flow et Mind utilisent les opérations de ces mêmes moteurs ; ils peuvent produire, modifier et ouvrir les mêmes projets. Le catalogue et la distribution grandissent indépendamment du chargement initial des apps.

Le résultat à viser est une plateforme capable d’accueillir des familles très différentes avec des capacités explicites, pas une promesse que toute fonction imaginable fonctionne identiquement sur chaque navigateur et chaque appareil.
