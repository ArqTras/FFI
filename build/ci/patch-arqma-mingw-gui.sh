#!/usr/bin/env bash
# Delegates to Node so CMake `${ARCH_ID}` and regex stay reliable on all shells.
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UP="${1:-}"
if [[ -z "$UP" ]]; then
  echo "usage: $0 <arqma-upstream-root>" >&2
  exit 1
fi
exec node "$ROOT/build/ci/patch-arqma-mingw-gui.js" "$UP"
