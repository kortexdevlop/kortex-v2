# 0002 — Cloudflare plan gratuit uniquement, calcul minimal côté serveur
**Statut** : accepté (2026-10-15)
**Contexte** : budget zéro ; 10 ms CPU/req, 100k req/jour, 10 000 neurons IA/jour pour tout le compte.
**Décision** : sites en Static Assets (sans invocation de Worker) ; seul `api.kortex.site` exécute du code ; D1 pour les données, pas de KV pour écritures utilisateur ; PBKDF2 WebCrypto + passkeys ; IA : crédits affichés en % par utilisateur (calcul en neurons, seau rechargé) plafonnés par le budget global journalier. R2 non utilisé (moyen de paiement requis).
**Alternatives** : plan payant (hors budget).
**Conséquences** : IA volontairement frugale (modèles 3B/8B, cache) ; message « IA en pause » quand le budget global est épuisé.
