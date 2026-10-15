#!/usr/bin/env bash
# Local quality gate: same checks as CI. Any failure stops the script (non-zero exit).
set -euo pipefail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
before=$(cat design/dist/kortex.css registry/index.json | shasum)
cargo xtask tokens
cargo xtask registry
after=$(cat design/dist/kortex.css registry/index.json | shasum)
[ "$before" = "$after" ] || { echo "✗ design/dist ou registry était périmé : régénéré, relance le gate"; exit 1; }
cargo xtask check
echo "✓ gate OK"
