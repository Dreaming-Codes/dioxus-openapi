#!/usr/bin/env bash
# Publish dioxus-openapi to crates.io, then push the git tag.
# Prerequisites:
#   cargo login          # or CARGO_REGISTRY_TOKEN
#   gh auth login        # or git remote + SSH key for GitHub
set -euo pipefail
cd "$(dirname "$0")"

echo "==> Publishing dioxus-openapi-macros 0.1.0"
(cd macros && cargo publish "$@")

echo "==> Waiting for crates.io index (macros)"
sleep 15

echo "==> Publishing dioxus-openapi 0.1.0"
cargo publish "$@"

echo "==> Tagging v0.1.0"
git tag -a v0.1.0 -m "v0.1.0" 2>/dev/null || true
echo "Done. Push with: git push -u origin main --tags"
