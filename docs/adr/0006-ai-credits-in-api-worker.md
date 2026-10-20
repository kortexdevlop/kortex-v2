# 0006 — Passerelle IA dans le Worker `api`, crédits en pourcentage
> Mise à jour 2026-10-18 : le modèle est désormais Gemma 4 (ADR 0023) ; les seaux, budgets et règles de confidentialité ci-dessous restent valables. Les prix par modèle et l'exemple Llama sont historiques.

**Statut** : accepté (2026-10-15)
**Contexte** : Workers AI gratuit = 10 000 neurons/jour pour TOUT le compte (reset 00:00 UTC), erreur au-delà (pas de dépassement possible sans plan payant). L'utilisateur veut des crédits affichés en %, calculés d'après les neurons réellement consommés, avec une limite « généreuse mais pas trop ».
**Décision** :
- Routes `GET /v1/ai/credits` et `POST /v1/ai/chat` dans `services/api` (même D1, mêmes sessions) plutôt qu'un second Worker : une ressource de moins sur le plan gratuit, pas de duplication de l'auth. Le modèle est appelé via le binding `AI`.
- **Seau par utilisateur** : capacité 1 200 neurons, +400 toutes les 3 h (recharge complète en 9 h). Avec Llama 3.2 3B (≈ 30 neurons/1 000 tokens générés) cela représente de l'ordre de 80 réponses courtes d'avance. **Budget global** : 9 000 neurons/jour (marge de 10 % sous le quota). Réglables par variables (`AI_USER_CAP`, `AI_USER_REFILL_3H`, `AI_GLOBAL_DAILY`).
- Réservation avant l'appel (estimation prudente : 3 caractères/token en entrée, `max_tokens` en sortie), puis règlement sur l'usage réel renvoyé par le modèle ; remboursement complet si l'appel échoue. Compteur global : upsert conditionnel atomique (le premier appel du jour respecte aussi le plafond) ; seau utilisateur : mise à jour optimiste avec reprises.
- L'interface ne reçoit que `percent`, `status` (`ok|low|empty|paused`) et `resets_in_seconds` : jamais de comptes de neurons. Quand le budget global est épuisé : « IA en pause, revient à HH:MM » (`ai_paused`, `Retry-After`).
- Confidentialité : `consent: true` obligatoire à chaque requête ; aucune table ne stocke prompts ni réponses (le registre `ai_ledger` agrège jour/modèle/neurons).
- Prix par modèle dans `ai-models.ts` (source : page tarifs Workers AI, 2026-10-15) à revérifier à chaque changement de modèle.
**Alternatives** : Durable Objects pour les compteurs (atomicité parfaite mais ressource supplémentaire), KV (très peu d'écritures gratuites), crédits en « nombre de requêtes » (rejeté par l'utilisateur).
**Conséquences** : en cas de course extrême, le compteur global peut être dépassé de quelques neurons ; le provider reste l'arbitre final et une erreur de quota le fait basculer en pause pour la journée.
