#!/usr/bin/env bash
# Build arqma-wallet-flutter-ffi for iOS device + simulator (requires Xcode + wallet_merged for iOS).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "${ROOT}/rust"

# rustup toolchain (iOS std) must win over Homebrew rustc on PATH.
export PATH="${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

if ! rustup target list --installed --toolchain 1.92.0-aarch64-apple-darwin 2>/dev/null | grep -q 'aarch64-apple-ios'; then
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim --toolchain 1.92.0-aarch64-apple-darwin
fi

# Upstream wallet_merged must exist for the iOS triple (see rust/docs/NATIVE_WALLET2.md).
export ARQMA_WALLET2_UPSTREAM_DIR="${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}"

if [[ "${ARQMA_SKIP_IOS_WALLET_MERGED:-0}" != "1" ]]; then
  bash "${ROOT}/rust/tool/build_ios_wallet_merged.sh"
fi
DEVICE_LIB_DIR="${ARQMA_WALLET2_LIB_DIR:-${ARQMA_WALLET2_UPSTREAM_DIR}/build-ios-depends-device/src/wallet}"
if [[ ! -f "${DEVICE_LIB_DIR}/libwallet_merged.a" ]]; then
  DEVICE_LIB_DIR="$(dirname "$(find "${ARQMA_WALLET2_UPSTREAM_DIR}/build-ios-device" -name 'libwallet_merged.a' -print -quit 2>/dev/null || true)")"
fi
if [[ ! -f "${DEVICE_LIB_DIR}/libwallet_merged.a" ]]; then
  echo "Missing iOS libwallet_merged.a; set ARQMA_WALLET2_LIB_DIR or run build_ios_wallet_merged.sh" >&2
  exit 1
fi

export ARQMA_WALLET_FFI_STATIC_HYBRID=1
export ARQMA_WALLET_FFI_USE_DEPENDS=1
export ARQMA_WALLET_FFI_DEPENDS_LIB_DIR="${ARQMA_WALLET2_UPSTREAM_DIR}/contrib/depends/aarch64-apple-ios/lib"

echo "Building arqma-wallet-flutter-ffi for aarch64-apple-ios (device)..."
ARQMA_WALLET2_LIB_DIR="${DEVICE_LIB_DIR}" \
  cargo build -p arqma-wallet-flutter-ffi --release --target aarch64-apple-ios

if [[ "${BUILD_IOS_SIM:-0}" == "1" ]]; then
  BUILD_IOS_SIM=1 bash "${ROOT}/rust/tool/build_ios_wallet_merged.sh"
  SIM_LIB_DIR="${ARQMA_WALLET2_UPSTREAM_DIR}/build-ios-sim/lib-arm64"
  if [[ ! -f "${SIM_LIB_DIR}/libwallet_merged.a" ]]; then
    SIM_LIB_DIR="$(dirname "$(find "${ARQMA_WALLET2_UPSTREAM_DIR}/build-ios-sim" -name 'libwallet_merged.a' -print -quit)")"
  fi
  echo "Building arqma-wallet-flutter-ffi for aarch64-apple-ios-sim..."
  ARQMA_WALLET2_LIB_DIR="${SIM_LIB_DIR}" \
    cargo build -p arqma-wallet-flutter-ffi --release --target aarch64-apple-ios-sim
fi

echo "Artifacts:"
ls -la "${ROOT}/rust/target/aarch64-apple-ios/release/libarqma_wallet_flutter_ffi.dylib" 2>/dev/null || true
ls -la "${ROOT}/rust/target/aarch64-apple-ios-sim/release/libarqma_wallet_flutter_ffi.dylib" 2>/dev/null || true