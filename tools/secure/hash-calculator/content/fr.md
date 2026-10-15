## Calculer l'empreinte d'un fichier ou d'un texte
Un hash est une empreinte numérique : si un seul octet change, l'empreinte change complètement. Cet outil calcule MD5, SHA-1, SHA-256 et SHA-512 directement sur votre appareil. Votre fichier n'est jamais envoyé.

## Comment ça marche
1. Déposez un fichier (ou saisissez un texte).
2. Choisissez l'algorithme (SHA-256 est recommandé).
3. Copiez l'empreinte, ou collez l'empreinte attendue pour vérifier un téléchargement.

## FAQ
### Mon fichier est-il envoyé sur Internet ?
Non. Le calcul est fait localement, sans aucune requête réseau.
### Quel algorithme choisir ?
SHA-256 pour vérifier l'intégrité d'un fichier. MD5 et SHA-1 ne sont plus sûrs contre un attaquant, mais restent courants pour de simples sommes de contrôle.
### Comment vérifier un téléchargement ?
En mode Pro, collez le hash publié par l'éditeur dans « Comparer avec » : l'outil indique si les deux correspondent.
