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
  android|android-arm64|android-x86_64)
    arches=()
    case "${PLATFORM}" in
      android-arm64) arches=(aarch64-linux-android) ;;
      android-x86_64) arches=(x86_64-linux-android) ;;
      android) arches=(aarch64-linux-android x86_64-linux-android) ;;
    esac
    found=0
    for arch in "${arches[@]}"; do
      jni=""
      case "${arch}" in
        aarch64-linux-android) jni=arm64-v8a ;;
        x86_64-linux-android) jni=x86_64 ;;
      esac
      so="${ROOT}/rust/target/${arch}/release/libarqma_wallet_flutter_ffi.so"
      if [[ -f "${so}" ]]; then
        mkdir -p "${STAGE}/jniLibs/${jni}"
        cp "${so}" "${STAGE}/jniLibs/${jni}/"
        found=1
      fi
    done
    if [[ "${found}" -eq 0 ]]; then
      echo "no Android FFI .so for ${PLATFORM}" >&2
      exit 1
    fi
    ;;
  ios)
    dev="${ROOT}/rust/target/aarch64-apple-ios/release/libarqma_wallet_flutter_ffi.dylib"
    sim="${ROOT}/rust/target/aarch64-apple-ios-sim/release/libarqma_wallet_flutter_ffi.dylib"
    if [[ -f "${dev}" ]]; then
      mkdir -p "${STAGE}/device"
      cp "${dev}" "${STAGE}/device/"
    fi
    if [[ -f "${sim}" ]]; then
      mkdir -p "${STAGE}/simulator"
      cp "${sim}" "${STAGE}/simulator/"
    fi
    if [[ ! -f "${dev}" && ! -f "${sim}" ]]; then
      echo "no iOS FFI dylib built" >&2
      exit 1
    fi
    ;;
  *)
    echo "unknown platform: ${PLATFORM}" >&2
    exit 2
    ;;
esac
OUT="${ROOT}/dist/arqma-wallet-ffi-${PLATFORM}-${VER}.zip"
(cd "${ROOT}/dist" && zip -qr "$(basename "${OUT}")" "${PLATFORM}")
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "artifact=${OUT}" >> "${GITHUB_OUTPUT}"
fi
ls -la "${OUT}"
