## Encoder et décoder des URL
Transformez un texte en version sûre pour une URL (espaces en %20, accents en %C3%A9…) ou décodez une URL encodée. Le comportement suit encodeURIComponent et encodeURI de JavaScript. Tout reste sur votre appareil.

## Comment ça marche
1. Collez le texte ou l'URL.
2. Choisissez Encoder ou Décoder (et la portée).
3. Cliquez sur Lancer puis copiez le résultat.

## FAQ
### Quelle différence entre Composant et URL complète ?
Composant encode tout sauf les lettres, chiffres et - _ . ! ~ * ' ( ) : à utiliser pour une valeur de paramètre. URL complète conserve aussi les séparateurs : / ? & = # etc.
### Pourquoi des espaces deviennent-elles « + » ?
Dans les formulaires HTML, « + » représente une espace. Activez l'option correspondante (mode Pro) pour le décoder.
### Les accents sont-ils gérés ?
Oui, en UTF-8 : é devient %C3%A9.
