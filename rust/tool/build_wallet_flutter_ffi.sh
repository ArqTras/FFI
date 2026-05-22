#!/usr/bin/env bash
# Build the Flutter desktop wallet FFI library (native wallet2, no arqma-wallet-rpc subprocess).
# Requires Arqma upstream + libwallet_merged per rust/docs/NATIVE_WALLET2.md.
# Full chain (CMake wallet_merged + this crate) on Linux/macOS: tool/build_native_wallet_flutter_ffi_unix.sh
# Windows: tool/build_native_wallet_flutter_ffi_windows.ps1
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [[ $# -ge 1 ]]; then
  cargo build -p arqma-wallet-flutter-ffi --release --target "$1"
else
  cargo build -p arqma-wallet-flutter-ffi --release
fi
echo "Done. Look under target/<profile>/ for libarqma_wallet_flutter_ffi (cdylib)."
