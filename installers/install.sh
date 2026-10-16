#!/bin/sh
# Kortex installer:  curl -fsSL https://kortex.site/install.sh | sh
#
# Installs the `kortex` command into ~/.kortex/bin (or $KORTEX_HOME/bin). It downloads the CLI
# package named in `cli-latest.txt` (same directory as the signed index), verifies its SHA-256 and
# extracts exactly one file. No sudo, nothing outside $KORTEX_HOME.
#
# Trust model: this script and cli-latest.txt come from kortex.site over TLS. Every tool you install
# afterwards is verified by `kortex` itself against the Ed25519 signature of the release index
# (public key compiled into the binary). Read this script before running it if you prefer.
set -eu

INDEX="${KORTEX_INDEX:-https://kortex.site/registry/index.json}"
HOME_DIR="${KORTEX_HOME:-$HOME/.kortex}"
BIN_DIR="$HOME_DIR/bin"

say() { printf '%s\n' "$*"; }
die() { printf 'erreur : %s\n' "$*" >&2; exit 1; }

case "$INDEX" in
  https://*) ;;
  http://localhost*|http://127.0.0.1*) ;;
  /*|file://*) ;;
  *) die "KORTEX_INDEX doit être en https:// (ou localhost pour le développement)" ;;
esac

# --- platform -> release target ---------------------------------------------------------------
os=$(uname -s); arch=$(uname -m)
case "$os-$arch" in
  Linux-x86_64)               target=x86_64-unknown-linux-gnu ;;
  Darwin-arm64|Darwin-aarch64) target=aarch64-apple-darwin ;;
  Darwin-x86_64)              target=x86_64-apple-darwin ;;
  *) die "plateforme non supportée : $os $arch (Windows : utilise install.ps1)" ;;
esac

# --- tools -------------------------------------------------------------------------------------
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL --proto '=https,http' "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q "$1" -O "$2"; }
else
  die "curl ou wget est requis"
fi
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "sha256sum ou shasum est requis"
fi
command -v tar >/dev/null 2>&1 || die "tar est requis"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/kortex-install.XXXXXX")
trap 'rm -rf "$tmp"' EXIT INT TERM

base="${INDEX%/*}"
say "Kortex — installation pour $target"
fetch "$base/cli-latest.txt" "$tmp/cli-latest.txt" || die "impossible de télécharger $base/cli-latest.txt"

# Lines: "<target> <sha256> <url>" ; url may be relative to the index directory.
line=$(grep "^$target " "$tmp/cli-latest.txt" || true)
[ -n "$line" ] || die "pas de version de kortex pour $target dans cli-latest.txt"
want=$(printf '%s' "$line" | cut -d' ' -f2)
url=$(printf '%s' "$line" | cut -d' ' -f3)
case "$url" in
  *://*|/*) ;;
  *) url="$base/$url" ;;
esac
case "$want" in
  ????????????????????????????????????????????????????????????????) ;;
  *) die "empreinte invalide dans cli-latest.txt" ;;
esac

say "Téléchargement…"
fetch "$url" "$tmp/kortex.tar.gz" || die "téléchargement impossible : $url"
got=$(sha256 "$tmp/kortex.tar.gz")
[ "$got" = "$want" ] || die "empreinte SHA-256 différente de celle attendue : paquet refusé ($got)"

# Extract the single expected file to stdout, never trusting names inside the archive.
mkdir -p "$BIN_DIR"
tar -xzOf "$tmp/kortex.tar.gz" kortex > "$tmp/kortex" 2>/dev/null || die "archive invalide (fichier 'kortex' absent)"
[ -s "$tmp/kortex" ] || die "binaire vide"
chmod 755 "$tmp/kortex"
mv "$tmp/kortex" "$BIN_DIR/kortex"

say "✓ kortex installé dans $BIN_DIR/kortex"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say ""
     say "Ajoute-le à ton PATH :"
     say "  echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.profile   # ou ~/.zshrc"
     say "puis ouvre un nouveau terminal." ;;
esac
say ""
say "Essaie :  kortex available     (outils disponibles)"
say "          kortex install secure.password-generator"
say "Vérification :  kortex doctor"
