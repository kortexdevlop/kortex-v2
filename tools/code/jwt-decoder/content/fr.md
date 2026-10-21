## Lisez un jeton JWT sans l'envoyer
Collez un jeton de connexion (JWT) pour voir ce qu'il contient : l'algorithme, les informations qu'il porte, sa date d'émission et sa date d'expiration. Le décodage se fait sur votre appareil : le jeton ne quitte jamais votre ordinateur ou votre téléphone, ce qui compte puisqu'un jeton donne souvent accès à un compte.

## Comment ça marche
1. Collez le jeton (avec ou sans le mot « Bearer » devant).
2. Choisissez en mode Medium de mettre le JSON en forme ou non.
3. Lancez le décodage et lisez l'en-tête, le contenu et les dates.

## FAQ
### Mon jeton est-il envoyé quelque part ?
Non. Le décodage est local, sans aucune requête réseau.
### L'outil vérifie-t-il la signature ?
Non. Vérifier une signature demande la clé secrète ou publique du service : l'outil sert à lire un jeton, pas à prouver qu'il est authentique.
### Pourquoi un jeton « chiffré » ne s'affiche-t-il pas ?
Un jeton chiffré (JWE) cache son contenu : il faut la clé pour le lire, ici ou ailleurs.
### Que signifient les dates ?
« Émis le » correspond à `iat` et « Expire le » à `exp`, affichées en heure UTC.
