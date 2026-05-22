#!/usr/bin/env bash
# Apply upstream CMake/FFI patches via Node (reliable regex). Requires patch-arqma-mingw-gui.js.
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UP="${1:-}"
if [[ -z "$UP" ]]; then
  echo "usage: $0 <arqma-upstream-root>" >&2
  exit 1
fi
SCRIPT="$ROOT/build/ci/patch-arqma-mingw-gui.js"
if [[ ! -f "$SCRIPT" ]]; then
  echo "::error::missing $SCRIPT" >&2
  exit 1
fi
NODE="${ARQMA_PATCH_NODE:-}"
if [[ -z "$NODE" ]]; then
  if command -v node >/dev/null 2>&1; then
    NODE=node
  elif [[ -n "${ARQMA_CI_WINDOWS_NODE_DIR:-}" && -f "${ARQMA_CI_WINDOWS_NODE_DIR}/node.exe" ]]; then
    NODE="${ARQMA_CI_WINDOWS_NODE_DIR}/node.exe"
  else
    echo "::error::node not found (install Node or set ARQMA_PATCH_NODE)" >&2
    exit 1
  fi
fi
exec "$NODE" "$SCRIPT" "$UP"
