#!/usr/bin/env bash
# Emit rust_toolchain for GitHub Actions from rust-toolchain.toml.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RT="$ROOT/rust-toolchain.toml"
if [[ ! -f "$RT" ]]; then
  echo "::error::missing $RT"
  exit 1
fi
t=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$RT" | head -1)
if [[ -z "${t}" ]]; then
  echo "::error::could not parse channel= from $RT"
  exit 1
fi
{
  echo "rust_toolchain=${t}"
} >> "${GITHUB_OUTPUT}"
echo "Pinned Rust toolchain=${t}"
