#!/usr/bin/env bash
# Build Arqma libwallet_merged + arqma-wallet-flutter-ffi (no arqma-wallet-rpc subprocess).
# Run on Linux or macOS from anywhere; requires CMake, toolchain deps per rust/docs/NATIVE_WALLET2.md.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUST_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$RUST_ROOT"
UP="${ARQMA_WALLET2_UPSTREAM_DIR:-$RUST_ROOT/arqma-rpc-upstream}"
export ARQMA_WALLET2_UPSTREAM_DIR="$UP"

if [[ ! -f "$UP/src/wallet/api/wallet2_api.h" ]]; then
  echo "Missing upstream; run from repo root: bash build/ci/clone-arqma.sh" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin)
    bash "$REPO_ROOT/build/ci/build-arqma-macos.sh"
    ;;
  Linux)
    bash "$REPO_ROOT/build/ci/build-arqma-linux.sh"
    ;;
  *)
    echo "Use Windows: rust\\tool\\build_native_wallet_flutter_ffi_windows.ps1" >&2
    exit 2
    ;;
esac

export ARQMA_WALLET_FFI_STATIC_HYBRID="${ARQMA_WALLET_FFI_STATIC_HYBRID:-1}"
export ARQMA_WALLET_FFI_USE_DEPENDS="${ARQMA_WALLET_FFI_USE_DEPENDS:-1}"
cargo build -p arqma-wallet-flutter-ffi --release
echo "OK: native wallet FFI library under $RUST_ROOT/target/release/"
