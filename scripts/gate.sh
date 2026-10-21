#!/usr/bin/env bash
# Local quality gate: same checks as CI. Any failure stops the script (non-zero exit).
set -euo pipefail
# Tests must not depend on what is installed or cached on this machine (a cached catalogue or
# installed tools change what the apps list): they get an empty Kortex home, like a clean CI runner.
KORTEX_HOME="$(mktemp -d)"
export KORTEX_HOME
pnpm install --frozen-lockfile --silent   # same as CI: the lockfile must match every package.json
rm -f design/dist/kortex.css   # like a clean checkout: it must be (re)generated, never assumed
cargo xtask tokens            # embedded by the Store and desktop apps
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
before=$(cat design/dist/kortex.css registry/contracts.json | shasum)
cargo xtask tokens
cargo xtask registry
after=$(cat design/dist/kortex.css registry/contracts.json | shasum)
[ "$before" = "$after" ] || { echo "✗ design/dist ou registry était périmé : régénéré, relance le gate"; exit 1; }
cargo xtask check
pnpm --filter @kortex/site test
pnpm --filter @kortex/shared test
pnpm --filter @kortex/api typecheck
pnpm --filter @kortex/api test
echo "✓ gate OK"
