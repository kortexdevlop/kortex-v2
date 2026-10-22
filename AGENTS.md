# Kortex — règles du projet

Autorité : instructions utilisateur récentes, puis `docs/RULES.md` et périmètre fonctionnel de `docs/MASTER_PROMPT.md` (à lire intégralement au début d’une phase). Plan : `docs/PLAN.md`. Journal : `docs/PROGRESS.md`. ADR : décisions compatibles, jamais dérogations unilatérales.

## Périmètre
- Catalogue gelé à 38 outils + ajouts autorisés explicitement par l’utilisateur : `image.compress` (2026-10-20), `code.jwt-decoder`, `text.diff`, `calc.bmi` (2026-10-21) = 42. Corriger l’existant ; aucun autre outil sans instruction explicite.
- Budget zéro, Cloudflare gratuit, licence propriétaire « tous droits réservés ». Dépôt `kortex-v2` public depuis le 2026-10-19 (décision explicite de l’utilisateur : minutes CI gratuites) ; aucun secret dans Git.
- Aucune action irréversible hors autorisation explicite et inventaire préalable.

## Architecture
- Outils : `tools/<univers>/<nom>/`, contrat `tool.toml`, moteur Rust pur, UI Dioxus commune web/desktop, glyph/locales/content/tests. Interface web et binaire desktop assemblés depuis les lecteurs partagés `kortex-player` + paquet `.kxp` (ADR 0035), jamais compilés par outil.
- Apps installables : `apps/<nom>/`, `app.toml` validé et mêmes sources Rust/Dioxus compilées web/desktop. Une app Astro/TS distincte est interdite comme substitut.
- Site Astro/TS : vitrine, SEO et intégration (navigation, palette site, Stage, compte existant). Workers : TypeScript strict conformément au prompt.
- Accès runtime plateforme uniquement via `kortex-platform` ; scripts de build et fixtures peuvent gérer leurs artefacts, jamais générer un contournement runtime.
- Aucun import d’un outil par un autre. Contrats/registre/bus pour la coopération. Pas d’unwrap/panic dans les moteurs.
- Couleurs/rayons/ombres via tokens ; dépendances justifiées dans `docs/deps.md` avant installation.
- Pas de fichiers utilisateur côté serveur ; consentement précis avant contenu vers IA. Métadonnées seules pour Sync, jetons desktop dans le trousseau.

## Exécution du plan
- Avant toute modification de chaque lot, annoncer exactement les étapes N.M, fichiers/couches concernés, changements prévus, contrôles indispensables et publication attendue ; ne pas se limiter à « je continue ».
- Choisir les étapes N.M avant chaque lot. Signaler immédiatement tout écart ; ne pas changer d’architecture pour gagner du temps.
- Contrôles utiles groupés ; compiler le Rust modifié et ses dépendants pour les cibles concernées. Réutiliser seulement les artefacts dont le reçu valide les sources/sorties.
- Distinguer présence, tests, compilation web, compilation native, recette et publication. Ne pas déclarer une app achevée avant preuves web + desktop.
- Publier chaque lot terminé après contrôles : autorisation persistante de l’utilisateur. CI voie nominale, secours local reproductible autorisé. Pas de fonction fictivement ouverte.
- Gate complet avant commit. Aucune recompilation générale inutile après changement Astro seul.

## Communication et mémoire
- Utilisateur : français. Code/identifiants/commits/commentaires techniques : anglais. Produit FR + EN.
- Mettre à jour PLAN.md et PROGRESS.md à chaque session ; AGENTS.md et CLAUDE.md restent identiques.
- Fin : résumé ≤5 lignes, résultat/limites, fichier ou URL à tester et prochaine étape.
