#!/usr/bin/env bash
# generate-docs.sh — Build and run the doc generator, then optionally commit.
#
# Usage:
#   bash scripts/generate-docs.sh [--commit]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT=false

for arg in "$@"; do
  [[ "$arg" == "--commit" ]] && COMMIT=true
done

echo "=== TipJar Doc Generator ==="
cargo run --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p tipjar-doc-generator -- \
  --input  "$REPO_ROOT/contracts/tipjar/src/lib.rs" \
  --out-dir "$REPO_ROOT/docs/api" \
  --contract TipJarContract

if $COMMIT; then
  git -C "$REPO_ROOT" add docs/api/
  git -C "$REPO_ROOT" diff --cached --quiet \
    || git -C "$REPO_ROOT" commit -m "docs: regenerate API docs [skip ci]"
  echo "Committed updated docs."
fi
