#!/usr/bin/env bash
# Zip FFI binaries for GitHub Release (platform label + version).
set -euo pipefail
PLATFORM="${1:?platform e.g. linux-x86_64}"
VER="${2:?version e.g. 1.0.0}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="${ROOT}/dist/${PLATFORM}"
mkdir -p "${STAGE}"
shopt -s nullglob
case "${PLATFORM}" in
  linux-x86_64)
    cp "${ROOT}/rust/target/release"/libarqma_wallet_flutter_ffi.so "${STAGE}/"
    ;;
  macos-arm64)
    cp "${ROOT}/rust/target/release"/libarqma_wallet_flutter_ffi.dylib "${STAGE}/"
    ;;
  windows-x86_64-gnu)
    cp "${ROOT}/rust/target/x86_64-pc-windows-gnu/release"/arqma_wallet_flutter_ffi.dll "${STAGE}/"
    ;;
  android)
    for arch in aarch64-linux-android x86_64-linux-android; do
      jni=""
      case "${arch}" in
        aarch64-linux-android) jni=arm64-v8a ;;
        x86_64-linux-android) jni=x86_64 ;;
      esac
      so="${ROOT}/rust/target/${arch}/release/libarqma_wallet_flutter_ffi.so"
      if [[ -f "${so}" ]]; then
        mkdir -p "${STAGE}/jniLibs/${jni}"
        cp "${so}" "${STAGE}/jniLibs/${jni}/"
      fi
    done
    ;;
  ios)
    dev="${ROOT}/rust/target/aarch64-apple-ios/release/libarqma_wallet_flutter_ffi.dylib"
    sim="${ROOT}/rust/target/aarch64-apple-ios-sim/release/libarqma_wallet_flutter_ffi.dylib"
    [[ -f "${dev}" ]] && mkdir -p "${STAGE}/device" && cp "${dev}" "${STAGE}/device/"
    [[ -f "${sim}" ]] && mkdir -p "${STAGE}/simulator" && cp "${sim}" "${STAGE}/simulator/"
    ;;
  *)
    echo "unknown platform: ${PLATFORM}" >&2
    exit 2
    ;;
esac
OUT="${ROOT}/dist/arqma-wallet-ffi-${PLATFORM}-${VER}.zip"
(cd "${ROOT}/dist" && zip -qr "$(basename "${OUT}")" "${PLATFORM}")
echo "artifact=${OUT}" >> "${GITHUB_OUTPUT}"
ls -la "${OUT}"
