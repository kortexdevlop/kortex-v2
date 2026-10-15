#!/usr/bin/env bash
# Local quality gate: same checks as CI. Any failure stops the script (non-zero exit).
set -euo pipefail
pnpm install --frozen-lockfile --silent   # same as CI: the lockfile must match every package.json
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
before=$(cat design/dist/kortex.css registry/index.json | shasum)
cargo xtask tokens
cargo xtask registry
after=$(cat design/dist/kortex.css registry/index.json | shasum)
[ "$before" = "$after" ] || { echo "✗ design/dist ou registry était périmé : régénéré, relance le gate"; exit 1; }
cargo xtask check
pnpm --filter @kortex/site test
pnpm --filter @kortex/shared test
pnpm --filter @kortex/api typecheck
pnpm --filter @kortex/api test
echo "✓ gate OK"
