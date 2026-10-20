## À propos

Saisissez une couleur opaque en HEX court ou long, RGB 0–255, HSL ou OKLCH. Les conversions suivent les espaces définis par CSS Color 4. Une couleur OKLCH hors sRGB est signalée ; ses sorties sont limitées au gamut par écrêtage des composantes.

## Comment utiliser cet outil

1. Choisissez le format d’entrée.
2. Saisissez par exemple #f00, rgb(255 0 0), hsl(0 100% 50%) ou oklch(0.627955 0.257683 29.23).
3. Copiez une sortie et vérifiez l’indicateur de gamut.

## Questions fréquentes

### La transparence est-elle acceptée ?

Non. Cet outil travaille sur des couleurs opaques ; les syntaxes alpha sont refusées.

### Que signifie hors gamut ?

La couleur dépasse sRGB. Les composantes RGB sont écrêtées, ce qui peut modifier sa teinte ; le résultat est signalé.

### Quels formats sont acceptés ?

HEX à 3 ou 6 chiffres, RGB numériques 0–255, HSL en degrés et pourcentages, OKLCH avec L entre 0 et 1 ou en pourcentage.

Référence : [W3C CSS Color 4](https://www.w3.org/TR/css-color/).
