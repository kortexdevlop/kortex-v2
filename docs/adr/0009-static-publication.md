# 0009 — Première publication statique et fonctions disponibles

## Contexte
Le 16 octobre 2026, l'utilisateur a arrêté l'ajout d'outils et demandé une mise en ligne à chaque lot. Les 38 outils et le site sont compilés ; API/D1 et distribution publique restent locaux. L'index desktop présent dans dist est un artefact de test, impropre à une publication.

## Décision
Publier deux Workers Static Assets, sans script de calcul serveur, sur les domaines définis dans `brand/domains.toml`. Les pages compte/téléchargement affichent leur indisponibilité en production. Leur activation future est explicite via variables de build ; le développement local conserve les parcours complets. Aucun artefact de test desktop n'est publié par défaut.

Une préparation versionnée contrôle l'inventaire, les limites du plan gratuit et les fichiers non publics. Wrangler publie les artefacts déjà compilés depuis la machine locale, ce qui évite un premier rebuild CI de tous les moteurs et préserve le budget GitHub privé. La commande est réutilisable en CI avec les mêmes variables d'environnement, sans secrets dans le dépôt.

## Conséquences
Aucune base D1 ni passerelle IA n'est créée pour cette première publication. Les fonctions futures ne sont pas promises comme opérationnelles. Les en-têtes du site utilisent les empreintes des scripts intégrés ; le domaine isolé Dioxus nécessite l'évaluation JavaScript dynamique pour son API document. Les nouvelles publications reprennent cette préparation et une recette de production ciblée.
