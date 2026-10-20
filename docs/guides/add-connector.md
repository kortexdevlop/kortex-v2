# Ajouter un connecteur Flow

Un connecteur est un fichier : `apps/flow/connectors/<nom>/connector.toml`, puis une ligne dans `apps/flow/src/connectors.rs` (`SOURCES`). Aucun code d'exécution : les actions sont des requêtes HTTP à gabarits (voir ADR 0026).

```toml
schema = 1
[connector]
id = "demo"                  # minuscules, chiffres, - et _
[connector.name]             # fr ET en obligatoires
fr = "Démo"
en = "Demo"
[connector.help]             # affiché dans le panneau de propriétés (où trouver le jeton)
fr = "Enregistrez votre jeton comme secret DEMO_TOKEN."
en = "Save your token as the secret DEMO_TOKEN."

[[actions]]
id = "send"
method = "POST"              # GET, POST, PUT, PATCH ou DELETE
url = "https://api.example.org/v1/items/{item|url}"
headers = ["authorization: Bearer {secret:DEMO_TOKEN}", "content-type: application/json"]
body = '{"text": {text|json}}'
[actions.name]
fr = "Envoyer"
en = "Send"
[[actions.params]]
id = "item"
required = true
[actions.params.label]
fr = "Élément"
en = "Item"
```
Règles : adresse HTTPS d'un nom public (le nœud HTTP applique la même politique à l'exécution), en-têtes parmi `accept, accept-language, content-type, authorization, x-api-key, notion-version`, tout `{param}` doit être déclaré, filtres `json` (chaîne JSON échappée), `url` (pourcent-encodage) ou aucun. Les valeurs reçues d'un autre nœud sont des données : elles ne peuvent jamais nommer un secret.

Vérifier : `cargo test -p kortex-flow --test connectors` (ajouter un test d'expansion pour la nouvelle action) et `cargo test -p kortex-workflow --test http`.
