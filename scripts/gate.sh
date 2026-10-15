#!/usr/bin/env bash
# Local quality gate: same checks as CI. Any failure stops the script (non-zero exit).
set -euo pipefail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask tokens
cargo xtask registry
cargo xtask check
git diff --exit-code design/dist registry >/dev/null || { echo "✗ design/dist ou registry non committé (régénéré par ce script)"; exit 1; }
echo "✓ gate OK"
