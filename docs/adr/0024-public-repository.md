# 0024 — Dépôt kortex-v2 public, licence propriétaire
**Statut** : accepté (2026-10-19), choix explicite de l'utilisateur (« 2 » : passer le dépôt en public). Modifie l'exigence « dépôt privé » (MASTER_PROMPT §Distribution, PLAN 1.6) ; ne modifie ni le budget nul ni la licence.
**Contexte** : sur un dépôt privé, les minutes GitHub Actions sont limitées (macOS ×10, Windows ×2) et les runs ci/deploy du commit 3122cff n'ont démarré aucun job (« paiements récents échoués ou plafond de dépenses à relever »). La release des trois systèmes était donc impossible. Sur un dépôt public les minutes sont gratuites.
**Décision** :
- `kortexdevlop/kortex-v2` est public. La licence reste « tous droits réservés, aucune licence accordée » (fichier LICENSE) : le code est lisible, pas réutilisable.
- Audit préalable de l'historique (72 commits, tous auteurs `noreply`) : aucune clé privée, aucun jeton, aucun `.env`/`.dev.vars`. Seul un identifiant de base D1 apparaît (identifiant, pas un secret).
- Les secrets restent dans les secrets GitHub et Wrangler, jamais dans Git. Toute fuite éventuelle impose rotation, pas seulement suppression.
**Conséquences** : le dépôt ne peut pas être « redevenu privé » pour effacer ce qui a été lu ; les workflows doivent rester sûrs pour un dépôt public (pas de `pull_request_target` avec secrets, secrets non exposés aux PR de forks).
**Alternatives** : régler la facturation du dépôt privé (écartée par l'utilisateur) ; dépôt public séparé pour les seuls builds (non retenu).
