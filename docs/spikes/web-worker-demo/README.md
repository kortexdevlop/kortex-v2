# Spike : moteur Rust dans un Web Worker

Ce dossier est une preuve reproductible de la Phase 0(c), pas l'exécuteur final. Il compile **le moteur existant** `image.convert` dans un wasm autonome, chargé par un Web Worker. Aucun traitement d'image ne passe par le serveur local ; celui-ci ne sert que les fichiers statiques du test.

## Reproduire

Depuis la racine du dépôt :

```sh
CARGO_TARGET_DIR="$PWD/.kortex/target-worker" cargo build --release --target wasm32-unknown-unknown --offline --manifest-path docs/spikes/web-worker-demo/Cargo.toml
wasm-bindgen --target web --out-dir docs/spikes/web-worker-demo/pkg .kortex/target-worker/wasm32-unknown-unknown/release/web_worker_demo.wasm
python3 -m http.server 4331 --bind 127.0.0.1 --directory docs/spikes/web-worker-demo
```

Ouvrir `http://127.0.0.1:4331/`, choisir un PNG local et lancer la conversion. La signature JPEG doit être `ff d8 ff`. Le bouton de stress prouve que le compteur de l'interface avance pendant un calcul Rust long ; « Cancel worker » termine le Worker et en redémarre un neuf.

## Résultat du 16 octobre 2026

- Build `wasm32-unknown-unknown` et génération `wasm-bindgen` réussis hors ligne.
- Dans le navigateur : `Worker ready, off main thread` ; PNG de test 8 × 8 converti en JPEG de 632 octets, signature `ff d8 ff`.
- Pendant le calcul long, le compteur de l'interface est passé de 36 à 80 ; annulation demandée avant la fin, Worker terminé et recréé. L'interface est restée utilisable.
- Transfert `ArrayBuffer` dans les deux sens, sans copie JS volontaire. L'appel `wasm-bindgen` peut encore copier les octets en mémoire wasm : mesurer ce coût avec de gros fichiers avant l'intégration.

## Intégration à faire

L'exécuteur générique est désormais intégré pour les contrats `web_worker = true` (ADR 0008). Ce démonstrateur conserve la preuve initiale indépendante. Restent la progression détaillée, les limites de mémoire sur gros fichiers, la recette PDF/hash, CSP et cache PWA.
