# Règles d’exécution de Kortex

Version consolidée du 16 octobre 2026, après retrait des prototypes non conformes. Ce document précise comment appliquer le cahier des charges de MASTER_PROMPT.md ; il ne réduit aucune fonction demandée. PLAN.md décrit l’état et l’ordre des travaux, pas de nouvelles règles.

## 1. Autorité et conflits

1.1 Les instructions explicites les plus récentes de l’utilisateur priment. Ensuite s’appliquent ces règles consolidées et le périmètre fonctionnel de MASTER_PROMPT.md.
1.2 AGENTS.md et CLAUDE.md sont des résumés identiques. PLAN.md, PROGRESS.md et les ADR n’autorisent aucune dérogation aux règles ou au cahier des charges.
1.3 Une contradiction découverte doit être signalée immédiatement et corrigée dans les documents concernés. Ne pas inventer une exception pour avancer.
1.4 Un ADR explique un choix compatible avec les exigences ; une dérogation exige une instruction explicite de l’utilisateur. Ne pas présenter un choix unilatéral comme une décision validée.

## 2. Périmètre actuel

2.1 Le catalogue est gelé à 38 outils, plus les ajouts autorisés explicitement par l’utilisateur : `image.compress` (2026-10-20), `code.jwt-decoder`, `text.diff` et `calc.bmi` (2026-10-21), soit 42 outils (recette ADR 0034). La cible historique 150+ et les autres nouveaux outils restent suspendus.
2.2 Corriger et finir l’existant, puis développer les applications prévues, dans l’ordre des dépendances de PLAN.md.
2.3 Aucun outil supplémentaire, service payant ou dépense. Cloudflare gratuit uniquement. Aucun fichier utilisateur traité sur le serveur.
2.4 Les fonctions absentes, prototypes, compilations non vérifiées et cibles non testées restent explicitement non validés.

## 3. Frontières des langages et des dossiers

3.1 Un outil appartient à tools/<univers>/<nom>/ : tool.toml, Cargo.toml, moteur pur Rust, UI Dioxus auto ou custom, glyph, locales FR/EN, content FR/EN, tests, changelog.
3.2 Toute application installable appartient à apps/<nom>/ : app.toml validé, Cargo.toml, logique Rust, UI Dioxus et les mêmes sources compilées pour web et desktop, avec ses locales/contenus/tests/visuels nécessaires.
3.3 Une application Astro/TypeScript séparée, même fonctionnelle, n’est pas un substitut à cette application Rust/Dioxus. Ne pas la publier comme un jalon de l’app.
3.4 web/site est la vitrine Astro : pages SEO, catalogue, pages légales, marketing et intégration des bundles. Son TypeScript est limité aux besoins du site : navigation, palette intégrée, Stage et interface de compte existante. Les apps Rust utiliseront les clients communs, sans dupliquer leur logique métier en TypeScript.
3.5 services contient les Workers TypeScript strict prévus par le prompt : auth, données, quotas, proxy. Rust n’est pas imposé aux Workers ni au site Astro.
3.6 TOML/JSON/Markdown/CSS, bindings wasm JavaScript générés et adaptateurs plateforme minimaux restent nécessaires. Ils ne doivent pas porter une seconde implémentation métier d’une app.
3.7 Ajouter un outil ou une app alimente le registre et les générateurs ; ne pas ajouter de branche par identifiant dans le site, la CLI ou le packaging.
3.8 Une interface web ou un binaire desktop d'outil `ui = "auto"` n'est pas compilé par outil : le lecteur partagé (`kortex-player`) est compilé une fois par cible et le paquet de l'outil (`engine.wasm`, contrat, locales, glyph) est assemblé dedans (ADR 0035, instruction de l'utilisateur du 2026-10-21). Une interface personnalisée exigerait un ADR et une décision explicite.

## 4. Frontières d’exécution

4.1 Le moteur d’un outil est indépendant de l’UI et n’importe aucun autre outil. Les hôtes orchestrent les moteurs par les contrats, les types de ports et le bus.
4.2 Tout accès runtime au système/navigateur/réseau/stockage/processus passe par kortex-platform. Les moteurs utilisent le contexte et les abstractions prévues, jamais directement std::fs, web_sys ou une commande système.
4.3 Les scripts de build, xtask et les fixtures de tests peuvent manipuler leurs propres artefacts. Cette exception ne permet pas de générer du code runtime qui contourne kortex-platform.
4.4 Les calculs lourds sont isolés dans un Worker web ou un thread/processus natif. Annulation, délai, erreurs et progression sont des comportements vérifiés, pas des commentaires dans un générateur.
4.5 Les apps partagent kortex-ui, les enveloppes, le bus typé, auth/sync/IA et la plateforme. Créer une crate quand elle a un usage réel, pas une coquille pour cocher une case.
4.6 Les outils web sont isolés sur l’origine run ; messages avec origine, fenêtre source et targetOrigin explicites. Aucun cookie de session API transmis aux outils.
4.7 Sur desktop, Desk/Flow/Mind découvrent les outils réellement installés. Un catalogue web complet ne remplace pas le registre local.
4.8 Les gros fichiers restent locaux ; streaming et limites mémoire doivent être validés avant de promettre les limites maximales des contrats.

## 5. Contrats, dépendances et UI

5.1 Les ports d’une app d’interface peuvent être vides ; les outils de traitement exigent une sortie. Métadonnées, ports déclarés et permissions gardent les mêmes types et validations. Les contrats sont l’unique source des métadonnées, permissions, ports, modes et préréglages. Outils et apps utilisent des schémas Rust partagés et validés.
5.2 Dépendances essentielles seulement, justifiées dans docs/deps.md avant installation ; ADR si choix structurant. Aucun import d’un outil par un autre.
5.3 Couleurs, rayons et ombres proviennent des tokens. Interdiction de valeurs visuelles en dur dans le produit. Géométrie métier, coordonnées et tailles de données ne sont pas des couleurs/rayons/ombres.
5.4 UI commune web/desktop, FR/EN, Simple/Medium/Pro selon contrat, clair/sombre, clavier/mobile, erreurs traduites et états chargement/vide/succès/annulation accessibles.
5.5 Pas d’unwrap()/panic! dans les moteurs. Pas de code mort ni TODO sans étape de plan correspondante.
5.6 Les budgets de taille historiques sont indicatifs depuis la consigne utilisateur ; seule une taille inadmissible doit être signalée et traitée. Cela ne dispense pas de contrôler mémoire et temps d’exécution.

## 6. Données et sécurité

6.1 Aucun secret dans Git. Utiliser secrets Worker/CI et stockage local protégé. Les jetons desktop vont dans le trousseau système ; un fichier 0600 n’est pas le livrable final demandé.
6.2 Pas d’Argon2/bcrypt dans les Workers, pas de KV pour les écritures utilisateur. D1 et autres services uniquement dans les possibilités gratuites réellement disponibles.
6.3 Auth mot de passe : dérivation client et vérification serveur selon ADR 0005, cohérentes entre Rust et clients existants ; mesurer CPU réel avant validation de charge. Passkeys avec origine/RP/UV/challenges contrôlés.
6.4 Aucun contenu de fichier envoyé à l’IA sans consentement explicite par fichier. Les permissions sensibles, réseau et écritures doivent être confirmées dans les parcours qui les exigent.
6.5 Synchronisation de métadonnées uniquement, jamais des fichiers. Secrets exclus des exports non protégés, journaux et sync. Respecter les rétentions et droits RGPD.
6.6 Dépôt source `kortex-v2` public depuis le 2026-10-19 (décision explicite de l’utilisateur, ADR 0024), licence propriétaire « tous droits réservés ». Les GitHub Releases ne contiennent que binaires, archives de bundles web compilés, index et catalogue signés, et notes (aucune source). Aucun secret dans Git ; une fuite impose la rotation. Le wasm public reste inspectable par nature.

## 7. Compilation et validation

7.1 Une présence de source, un test, une compilation wasm, une compilation native, une recette et une publication sont des preuves distinctes.
7.2 Regrouper les changements et les contrôles indispensables. Ne pas recompiler systématiquement 38 outils inchangés après une modification du site.
7.3 Compiler les sources Rust modifiées et leurs dépendants pour les cibles concernées avant validation. Une modification commune runtime/UI/bus peut invalider tous les bundles : le pipeline doit le détecter.
7.4 Un reçu source/artefact vérifiable peut éviter une compilation identique. Un artefact sans reçu ou une compilation historique ne prouve pas sa fraîcheur actuelle.
7.5 Une app n’est complète/installable qu’après compilations web et desktop, contrats/checks, tests pertinents et recettes requises. Une première compilation ne clôt pas toutes ses fonctions.
7.6 Avant commit, appliquer le gate du dépôt. Sans commit, effectuer les contrôles pertinents au lot et préciser leur portée. La CI doit compiler les bundles modifiés et contrôler aussi les apps.
7.7 Les builds desktop ne concernent que les identifiants modifiés ; ils s’enchaînent après une CI verte sur `main` ou manuellement (décision de l’utilisateur du 2026-10-20, ADR 0034), jamais sur chaque push de branche ou de PR. Rester dans le budget gratuit et contrôler les artefacts publics.

## 8. Publication et opérations

8.1 La consigne persistante autorise la publication de chaque lot terminé après ses contrôles utiles. Pas de nouvelle demande d’autorisation pour une publication déjà autorisée dans ce périmètre.
8.2 CI/CD est la voie nominale ; la publication locale reproductible est le secours autorisé par l’utilisateur. Elle ne remplace pas la finalisation de la CI.
8.3 Aucune suppression irréversible de repo/Worker/DNS/base, force-push ou dépense sans instruction explicite et inventaire préalable. Une demande de suppression ciblée autorise ce retrait, pas la destruction des ressources partagées.
8.4 Ne pas publier une fonction locale comme opérationnelle en production. Garder IA et apps non validées indisponibles. Les téléchargements desktop sont ouverts depuis le 2026-10-19 par décision explicite de l’utilisateur ; leurs limites restent affichées et suivies (release.md, PLAN 10.x).
8.5 Conserver un lien entre sources, artefacts, contrôles et version publiée. Tester un parcours réel utile en fin de lot ; ne pas multiplier les recettes identiques sans changement ou échec nouveau.

## 9. Suivi et langues

9.1 Échanges utilisateur en français. Identifiants, code, commits et commentaires techniques en anglais. Produit FR et EN dès le départ.
9.2 Avant chaque lot : lire le plan, choisir ses identifiants N.M et annoncer exactement, avant modification, la couche, les fichiers/cibles, les changements prévus, les vérifications indispensables et ce qui sera publié. Consigne persistante explicite du 17 octobre 2026 ; une formule générale ne suffit pas. Signaler immédiatement un écart trouvé ; corriger avant de poursuivre dans cette direction.
9.3 Mettre à jour PLAN.md avec preuves et validations, puis PROGRESS.md à chaque session. ROADMAP renvoie au plan ; les journaux sont historiques et ne remplacent pas l’état courant.
9.4 Fin de session : résultat, contrôles et limites, fichier/URL à lire ou tester, prochaine étape. Ne pas annoncer une conformité parfaite ou un projet achevé sans preuves correspondantes.
