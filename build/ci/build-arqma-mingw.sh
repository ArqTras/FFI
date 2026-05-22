#!/usr/bin/env bash
# Build libwallet_merged.a with MinGW (run inside MSYS2 MINGW64 shell). CMake output: <upstream>/build-mingw/...
# Delegates to build-arqma-wallet-ffi-deps.sh (minimal CMake targets for FFI only).
set -eu
# GitHub Actions often runs this script with pipefail; `where node` can fail while stderr is empty — do not fail the step.
set +o pipefail 2>/dev/null || true
# MSYS2 bash uses a minimal PATH. CI sets ARQMA_CI_WINDOWS_NODE_DIR (pwsh) from setup-node; prefer that.
if [ -n "${ARQMA_CI_WINDOWS_NODE_DIR:-}" ] && command -v cygpath >/dev/null 2>&1; then
  export PATH="$(cygpath -u "$ARQMA_CI_WINDOWS_NODE_DIR"):$PATH"
fi
# Local / fallback: locate node.exe via cmd when still missing.
if ! command -v node >/dev/null 2>&1 && command -v cygpath >/dev/null 2>&1; then
  _win_node=""
  _win_node=$(cmd.exe /d /s /c "where node" 2>/dev/null | tr -d '\r' | head -n 1) || true
  if [ -n "${_win_node}" ]; then
    export PATH="$(dirname "$(cygpath -u "${_win_node}")"):$PATH"
  fi
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec env ARQMA_WALLET_FFI_PLATFORM=mingw bash "$ROOT/build/ci/build-arqma-wallet-ffi-deps.sh"
