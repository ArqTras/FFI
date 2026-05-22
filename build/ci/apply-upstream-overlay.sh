#!/usr/bin/env bash
# Copy FFI-specific contrib/depends overrides into a cloned arqtras/arqma tree.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UP="${ARQMA_WALLET2_UPSTREAM_DIR:-$ROOT/rust/arqma-rpc-upstream}"
OVERLAY="$ROOT/upstream-overlay"
if [[ ! -d "$OVERLAY" ]]; then
  exit 0
fi
if command -v rsync >/dev/null 2>&1; then
  rsync -a "$OVERLAY/" "$UP/"
else
  cp -a "$OVERLAY/." "$UP/"
fi
echo "[apply-upstream-overlay] applied to $UP"
