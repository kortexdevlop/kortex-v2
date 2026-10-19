## Présentation
Kortex Flow est un éditeur visuel d’automatisation : vous posez des outils Kortex sur un canevas comme des cartes, vous les reliez, et le résultat de chacun alimente le suivant. Un workflow est un petit fichier JSON versionné que vous pouvez exporter et importer.
## Utilisation
Choisissez un outil dans la palette : il se place à droite du nœud sélectionné et se relie à lui automatiquement, ce qui permet de construire une chaîne en quelques clics. Pour relier autrement, glissez d’une sortie (à droite d’une carte) vers une entrée (à gauche d’une autre) ; Flow vérifie les types et refuse les boucles. Glissez une carte pour la déplacer, le fond pour naviguer, la molette pour zoomer ; Suppr supprime, Ctrl+Z et Ctrl+Y annulent et rétablissent. Les propriétés du nœud sélectionné (texte, paramètres, nouvelles tentatives) sont dans le panneau de droite. Exécutez : chaque carte indique en direct si elle attend, tourne, réussit ou échoue, et son résultat se lit dans le panneau.
## Questions fréquentes
### Mes données quittent-elles mon appareil ?
Non. Chaque outil s’exécute localement : dans le navigateur par son moteur embarqué, sur le bureau par l’outil installé. Le workflow ne conserve que sa structure, les positions et vos textes dans le stockage de votre appareil.
### Quels outils puis-je utiliser ?
Les 38 outils sont proposés. Pour un outil qui demande des fichiers (PDF, images, calcul d’empreinte), ajoutez un nœud Fichier ou Lot de fichiers : le fichier est choisi sur votre appareil, reste en mémoire pour l’exécution et n’est jamais enregistré dans le workflow ni envoyé. Les nœuds intégrés Condition, Filtre de lignes, Fusion et Délai permettent de faire des branches : seule la branche prise s’exécute, l’autre apparaît comme ignorée.
