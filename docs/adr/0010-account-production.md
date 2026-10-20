# 0010 — Compte en production et rétention

## Contexte
Le compte mot de passe fonctionne localement ; la publication de chaque lot est autorisée. L'API de production nécessite un binding D1 propre, une configuration de cookie sûre et une disponibilité publique qui survive aux prochains builds.

## Décision
La configuration production de kortex-api porte explicitement son binding D1, son domaine, sa liste d'origines et le cookie __Host-kortex_session. L'IA n'a pas de binding de production. brand/features.toml pilote l'ouverture du compte ; les variables de build peuvent la surcharger. Le déploiement applique les migrations, publie l'API et vérifie sa santé avant l'interface.

Les clés anti-abus sont hachées. Une tâche quotidienne supprime sessions expirées, clés de limitation âgées de plus d'un jour et agrégats âgés de plus de 90 jours ; les index limitent le coût des balayages. Les corps JSON sont bornés en octets avant décodage et parsing.

## Conséquences
Aucune collecte de fichiers ni de mot de passe en clair. Les sessions restent sur l'origine API. La politique de confidentialité reflète les données réellement stockées et les limites de rétention, y compris les sauvegardes D1. Passkeys et Turnstile sont des étapes suivantes et ne sont pas présentés comme disponibles.

La recette de production utilise uniquement un compte QA unique, supprimé à la fin, avec nettoyage de secours après un échec. Elle a aussi révélé puis corrigé une règle CSS qui outrepassait les attributs hidden des panneaux de compte.
