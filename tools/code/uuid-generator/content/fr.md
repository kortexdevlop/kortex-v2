## Générer des UUID
Créez des identifiants universels uniques : v4 (entièrement aléatoires) ou v7 (commencent par l'horodatage, donc triables et bien adaptés aux bases de données). Générés avec le générateur aléatoire cryptographique de votre appareil, sans envoi en ligne.

## Comment ça marche
1. Choisissez la version et la quantité.
2. Ajustez la casse et les tirets si besoin.
3. Cliquez sur Lancer et copiez la liste.

## FAQ
### Quelle différence entre v4 et v7 ?
v4 est entièrement aléatoire. v7 contient l'heure de création en tête : les UUID générés plus tard sont triés après, ce qui accélère les index de base de données.
### Deux UUID peuvent-ils être identiques ?
En pratique non : avec 122 bits aléatoires, la probabilité est négligeable.
### Peut-on s'en servir comme secret ?
Non, un UUID est un identifiant, pas un mot de passe : utilisez le générateur de mots de passe.
