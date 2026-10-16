# Publier une release (CLI, Store, outils desktop)

## Vue d'ensemble
`release.yml` construit sur **Linux, macOS (Apple Silicon, + Intel en option) et Windows** : la CLI `kortex`, le Store (`apps.store`) et les applications desktop des outils, puis une tâche `publish` regroupe les paquets, écrit `index.json`, le **signe** (Ed25519) et, si le secret `RELEASES_TOKEN` existe, crée la release dans le dépôt public `kortex-releases`.

Coût en minutes (dépôt privé : macOS ×10, Windows ×2) : une validation « 1 outil » ≈ 250 min ; une release complète est plus lourde. Le workflow ne tourne **jamais sur push** : uniquement sur tag `release/AAAA-MM-JJ` ou à la main (Actions → release → Run workflow).

Validée sur GitHub Actions (Linux, macOS arm64, Windows) le 2026-10-16. Pour ne relancer qu'un système : Run workflow → `only`.

## Prérequis (une fois)
1. Clé de signature : `cargo xtask keygen --out ~/.kortex-signing-key` (clé publique → `crates/kortex-pkg/release.pub`, à committer ; **ne jamais committer la clé privée**).
2. Secret GitHub `KORTEX_SIGNING_KEY` = contenu du fichier de clé privée.
3. Pour publier : secret `RELEASES_TOKEN` = jeton à portée limitée (contenu : écriture sur `kortexdevlop/kortex-releases` uniquement).

## En local (tout sauf la publication)
```bash
cargo xtask tokens
cargo xtask build-cli && cargo xtask build-app apps.store && cargo xtask build-desktop secure.password-generator
cargo xtask package cli && cargo xtask package apps.store && cargo xtask package secure.password-generator
cargo xtask release-index --key-file ~/.kortex-signing-key          # dist/release/index.json(.sig) + cli-latest.txt
cd dist/release && python3 -m http.server 4350 &                     # serveur local de test
KORTEX_HOME=/tmp/kh KORTEX_INDEX=http://127.0.0.1:4350/index.json sh installers/install.sh
```

## Côté utilisateur
```bash
curl -fsSL https://kortex.site/install.sh | sh        # macOS / Linux
irm https://kortex.site/install.ps1 | iex             # Windows (PowerShell)
kortex available · kortex install <outil> · kortex update --all · kortex update --self · kortex doctor
```

## Hébergement de l'index
`web/site/scripts/sync.mjs` copie `installers/install.{sh,ps1}` vers `kortex.site/` et, si `dist/release/index.json` existe, l'index signé et `cli-latest.txt` vers `kortex.site/registry/`. La release suivante remplace l'index : c'est ce qui « publie » les nouvelles versions.

## À savoir
- Une release reconstruit tout (une seule version par outil dans un index). Les releases « groupées » avec fusion des paquets inchangés viendront plus tard.
- Pas encore de numéro de séquence dans l'index (protection contre le retour à un ancien index signé).
- Windows : lanceur `.cmd` dans le menu Démarrer (un vrai raccourci `.lnk` demande COM).
