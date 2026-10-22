#!/usr/bin/env bash
# Adds or updates ONE tool everywhere in about a minute, without CI and without compiling anything
# but its engine (ADR 0035): the desktop players are reused (release `players`), the payload of the
# tool is pasted into them, the signed index is extended and only the `run` and `site` hosts are
# deployed. Usage: scripts/add-tool.sh <universe.name> [--no-commit]
# Needs: gh (logged in), wrangler (logged in), the signing key (~/.kortex-signing-key).
set -euo pipefail
ID="${1:?usage: scripts/add-tool.sh <universe.name> [--no-commit]}"
cd "$(dirname "$0")/.."
REPO="kortexdevlop/kortex-v2"
KEY="${KORTEX_KEY_FILE:-$HOME/.kortex-signing-key}"
TARGETS="x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc x86_64-unknown-linux-gnu"
T0=$(date +%s)
step() { printf '[%3ss] %s\n' "$(( $(date +%s) - T0 ))" "$*"; }
[ -f "$KEY" ] || { echo "clé de signature absente : $KEY" >&2; exit 1; }
export KORTEX_RELEASE_DIR="$PWD/dist/release"
DIR="tools/${ID%%.*}/${ID#*.}"
PKG="tool-${ID%%.*}-${ID#*.}"
if [ "${2:-}" != "--no-commit" ]; then
  [ "$(git rev-parse --abbrev-ref HEAD)" = "main" ] || { echo "à lancer depuis la branche main (ou avec --no-commit)" >&2; exit 1; }
  trap 'echo "échec : corriger puis relancer scripts/add-tool.sh $ID (rejouable, le commit déjà poussé est réutilisé)" >&2' ERR
fi

step "1/8 gate de l'outil (version, registre, garde-fous, tests, moteurs)"
if git cat-file -e "HEAD:$DIR/tool.toml" 2>/dev/null && ! git diff --quiet HEAD -- "$DIR"; then
  [ "$(git show "HEAD:$DIR/tool.toml" | grep -m1 '^version')" != "$(grep -m1 '^version' "$DIR/tool.toml")" ] \
    || { echo "outil modifié : monter 'version' dans $DIR/tool.toml" >&2; exit 1; }
fi
cargo xtask registry >/dev/null && cargo xtask check "$ID" | tail -1
cargo test -q -p "$PKG"
cargo xtask engines-check "$ID" | tail -1

step "2/8 moteur wasm"
cargo xtask build-engine "$ID" | tail -1

step "3/8 lecteurs déjà compilés (release 'players')"
for t in $TARGETS; do
  ext=""; case "$t" in *windows*) ext=".exe";; esac
  dest=".kortex/players/$t/kortex-player$ext"
  if [ ! -f "$dest" ]; then
    mkdir -p ".kortex/players/$t"
    gh release download players --repo "$REPO" --pattern "kortex-player-$t$ext" --output "$dest" --clobber
    chmod +x "$dest"
  fi
done

step "4/8 assemblage et paquets des 4 plateformes"
rm -rf dist/release && mkdir -p dist/release
for t in $TARGETS; do
  cargo xtask build-desktop "$ID" --target "$t" --players-dir .kortex/players | tail -1
  cargo xtask package "$ID" --target "$t" | tail -1
done

step "5/8 web (fichiers de l'outil)"
cargo xtask build "$ID" | tail -1
cargo xtask package-web "$ID" | tail -1

if [ "${2:-}" = "--no-commit" ]; then
  step "terminé (--no-commit) : build local seulement, rien commité ni publié"
  exit 0
fi
if [ "${2:-}" != "--no-commit" ]; then
  step "6/8 sources de l'outil dans git (sans CI)"
  git add -A "tools/${ID%%.*}/${ID#*.}" registry CLAUDE.md AGENTS.md docs Cargo.lock 2>/dev/null || true
  git diff --cached --quiet || git commit -q -m "feat(tools): add $ID [skip ci]

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
  git push -q origin HEAD:main
fi

step "7/8 index signé et release GitHub"
mkdir -p .kortex/previous
curl -fsSL https://kortex.site/registry/index.json -o .kortex/previous/index.json
SEQ=$(python3 -c 'import json; print(json.load(open(".kortex/previous/index.json"))["sequence"] + 1)')
TAG="feed-$SEQ"
cargo xtask release-index --base-url "https://github.com/$REPO/releases/download/$TAG" --channel stable \
  --source-commit "$(git rev-parse HEAD)" --previous .kortex/previous/index.json --key-file "$KEY" | tail -2
shopt -s nullglob
gh release create "$TAG" dist/release/*.json dist/release/*.sig dist/release/*.txt dist/release/*.tar.gz --repo "$REPO" --target "$(git rev-parse HEAD)" --title "$TAG" \
  --notes "Fast lane: $ID (players reused, nothing compiled but its engine)." >/dev/null

step "8/8 déploiement des hôtes run et site"
rm -rf dist/apps
cargo xtask restore-web --index dist/release/index.json | tail -1
cargo xtask tokens >/dev/null
DX_CLI_TELEMETRY=false cargo xtask build-player-web | tail -1
cargo xtask icons >/dev/null
pnpm site:build >/dev/null
node scripts/prepare-publish.mjs >/dev/null
wrangler_bin="$(command -v wrangler || true)"
[ -n "$wrangler_bin" ] || PATH="$(dirname "$(npm exec --yes --package=wrangler@4.151.0 -- which wrangler)"):$PATH"
node scripts/publish.mjs --static-only --only=run,site | tail -3
if [ "${2:-}" != "--no-commit" ] && ! git diff --quiet -- web/site/public/seek-index.json; then
  git add web/site/public/seek-index.json
  git commit -q -m "chore: refresh the search index for $ID [skip ci]

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
  git push -q origin HEAD:main
fi
step "terminé : $ID est en ligne (index séquence $SEQ)"
