# 0008 — Exécuteur Web Worker par invocation

## Contexte
Les moteurs CPU exécutés directement par `AutoTool` bloquent le navigateur. Le spike de Phase 0 a validé un moteur Rust dans un Worker, y compris l'arrêt d'un calcul long.

## Décision
Le contrat active `tool.web_worker = true` pour les moteurs lourds. `xtask build` compile alors un wasm headless séparé de Dioxus et un module Worker, dans un dossier nommé par son contenu. `kortex-platform::worker` prend en charge la plateforme ; les moteurs restent purs et inchangés.

Chaque invocation possède son Worker : réponse, erreur, délai maximal ou annulation le terminent. Une nouvelle invocation repart de zéro, ce qui évite les résultats tardifs d'une ancienne exécution. Les valeurs et erreurs traversent un protocole Serde typé ; les octets des fichiers deviennent des `Uint8Array` transférés dans les deux sens. Le passage mémoire wasm ↔ JavaScript implique encore une copie, sans encodage JSON/base64 des fichiers.

Les codes d'erreur sont rattachés aux clés statiques des locales par le wrapper généré. Les erreurs de chargement, protocole et délai sont traduites dans l'UI commune. La progression reste indéterminée : aucun faux pourcentage n'est annoncé. Le streaming de progression pourra étendre ce protocole.

## Alternatives
Un pool conserverait les moteurs chauds mais compliquerait l'annulation des moteurs synchrones. Un message « cancel » ne peut pas interrompre une boucle Rust qui occupe déjà le Worker. L'arrêt immédiat par `terminate()` est donc retenu pour cette première intégration.

## Conséquences et validation
Image, PDF et hash activent le Worker ; les autres outils gardent le chemin léger. Le desktop utilise son runner natif existant. Le démarrage et la compilation du module sont inclus dans le délai du contrat. L'annulation perd l'état temporaire du moteur.

Test navigateur réel : conversion PNG → JPEG, annulation sur PNG 3 000 × 3 000, erreur d'entrée manquante traduite, puis nouvelle conversion réussie. Les charges PDF extrêmes, la consommation mémoire et le cache hors ligne restent à vérifier dans la recette des pilotes.


## Recette complémentaire — 16 octobre 2026
Le test Chromium reproductible `scripts/tests/worker-smoke.mjs` valide PDF, hash (64 Mio), transfert des buffers, UI réactive, erreurs, annulation/délai et reprise. Le build produit désormais un service worker par outil, enregistré par `kortex-platform`, avec l'inventaire fermé des assets publics et un cache versionné par contenu. Aucun fichier utilisateur ni réponse API n'est mis en cache. Exécution hash et rechargement validés sans réseau. Les caches d'autres outils sont préservés. Les builds existants doivent être reconstruits ; mémoire totale et streaming restent à mesurer/implémenter.
