# 0004 — Bus hôte ↔ outil par postMessage, outils isolés dans une iframe d'une autre origine
**Statut** : accepté (2026-10-15)
**Contexte** : le site, plus tard Desk/Flow/Mind, hébergent des outils. Un outil qui plante ne doit pas casser l'hôte, et ne doit accéder à aucune donnée de session (cookies d'`api.` jamais envoyés à `run.`).
**Décision** : chaque outil est servi depuis `run.kortex.site/<id>/` et chargé dans une `<iframe>`. Messages JSON simples `{ type: "kortex:*" }` :
- outil → hôte : `kortex:ready` ;
- hôte → outil : `kortex:init { theme }` et `kortex:files { files: File[] }` (les `File` passent par clonage structuré, sans copie manuelle).
Sécurité : côté outil, on n'accepte que `e.source === window.parent` ET `e.origin` égal à l'origine de l'hôte (`location.ancestorOrigins[0]`, sinon `document.referrer`) ; côté hôte, seulement la fenêtre de SON iframe et l'origine `run.` ; `targetOrigin` toujours explicite (jamais `*`).
**Alternatives** : montage direct du wasm dans la page hôte (plus rapide mais pas d'isolation), `BroadcastChannel` (pas de contrôle de la source).
**Conséquences** : le protocole sera étendu (résultats, liaisons Desk) dans `kortex-bus` ; tout nouveau type de message est documenté ici.


**Précision 2026-10-16** : l’iframe du Stage utilise `referrerpolicy="origin"`, car le `no-referrer` initial rendait document.referrer vide lorsque ancestorOrigins était absent. Seule l’origine est transmise, jamais le chemin ni les paramètres. Le test de secours sans ancestorOrigins rejette les fenêtres et origines forgées. En Chromium de production, referrer d’origine et propagation du thème sont vérifiés ; recette Firefox native restante.
