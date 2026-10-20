# 0003 — Dépôt privé `kortex-v2` + dépôt public `kortex-releases` (binaires seuls)
**Statut** : remplacé par [ADR 0024](0024-public-repository.md) (2026-10-19). `kortex-v2` est public et les binaires sont publiés dans ses GitHub Releases ; `kortex-releases` n’est plus utilisé. Décision initiale acceptée le 2026-10-15.
**Décision** : code dans `kortexdevlop/kortex-v2` (privé, licence propriétaire) ; binaires, index signé Ed25519 et notes dans `kortexdevlop/kortex-releases` (public, aucun code). L'ancien dépôt `kortex` est conservé tel quel. CI économe (Linux, paths, releases groupées).
**Conséquences** : le code client wasm/JS reste inspectable ; logique sensible uniquement côté serveur.
