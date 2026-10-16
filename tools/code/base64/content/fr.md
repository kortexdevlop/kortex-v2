## Encoder et décoder du Base64
Convertissez du texte en Base64 ou décodez une chaîne Base64 (même sans remplissage ni avec des retours à la ligne). Les variantes standard et URL-safe sont gérées. Tout reste sur votre appareil.

## Comment ça marche
1. Collez le texte ou la chaîne Base64.
2. Choisissez Encoder ou Décoder.
3. Cliquez sur Lancer et copiez le résultat.

## FAQ
### Le Base64 est-il du chiffrement ?
Non : c'est un simple codage réversible par n'importe qui. Ne l'utilisez pas pour protéger un secret.
### Quand utiliser la variante URL-safe ?
Dans une URL, un nom de fichier ou un jeton JWT : elle remplace + et / par - et _.
### Les accents fonctionnent-ils ?
Oui, le texte est encodé en UTF-8 avant la conversion.
