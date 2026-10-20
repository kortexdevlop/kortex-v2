## À propos

Le rapport repose sur la luminance relative sRGB : (L la plus claire + 0,05) / (L la plus sombre + 0,05). Les décisions utilisent la valeur non arrondie. Le test porte uniquement sur cette paire de couleurs opaques, pas sur l’accessibilité complète d’une page.

## Comment utiliser cet outil

1. Choisissez la couleur du texte et celle du fond.
2. Lancez la mesure.
3. Consultez séparément les seuils pour texte normal et grand texte.

## Questions fréquentes

### Quels sont les seuils AA ?

4,5:1 pour le texte normal et 3:1 pour le grand texte selon WCAG 1.4.3.

### Que signifie grand texte ?

Au moins 18 points, ou 14 points en gras selon la définition WCAG. La taille effective et la graisse doivent être vérifiées dans votre interface.

### Les images de fond et la transparence sont-elles prises en compte ?

Non. Utilisez les couleurs composites réellement affichées ; cet outil refuse les couleurs avec alpha.

Référence : [W3C WCAG, Contrast Minimum](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum).
