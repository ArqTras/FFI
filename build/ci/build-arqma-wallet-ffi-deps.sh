#!/usr/bin/env bash
# Build only the CMake targets needed for arqma-wallet2-api / arqma-wallet-flutter-ffi:
# static archives that rustc links (see rust/arqma-wallet2-api/build.rs), plus wallet_merged.
# This does NOT run a blanket `cmake --build` of the whole Arqma tree (no arqmad, no rpc, etc.).
#
# Modes:
#   * Default: host CMake in ARQMA_CMAKE_BUILD_DIR (default …/build/ci-native-release).
#   * ARQMA_WALLET_FFI_USE_DEPENDS=1 (linux|macos only): `make -C contrib/depends HOST=…` then CMake with
#     contrib/depends/<HOST>/share/toolchain.cmake and STATIC=ON (same idea as upstream `make depends`).
#
# Upstream: arqtras/arqma (fork) — clone with build/ci/clone-arqma.sh first (ARQMA_UPSTREAM_REF, default pospow).
#
# Usage:
#   bash build/ci/build-arqma-wallet-ffi-deps.sh linux|macos|mingw
#   ARQMA_WALLET_FFI_PLATFORM=linux bash build/ci/build-arqma-wallet-ffi-deps.sh
#
# Env:
#   ARQMA_WALLET2_UPSTREAM_DIR   — Arqma core root (default: <repo>/rust/arqma-rpc-upstream)
#   ARQMA_CMAKE_BUILD_DIR        — Linux/macOS CMake build dir (default: ci-native-release or ci-depends-release)
#   ARQMA_MINGW_BUILD_DIR        — MinGW build dir (default: <upstream>/build-mingw)
#   ARQMA_WALLET_FFI_USE_DEPENDS — set to 1 on Linux/macOS to build vendored static deps via contrib/depends
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UP="${ARQMA_WALLET2_UPSTREAM_DIR:-$ROOT/rust/arqma-rpc-upstream}"

PLATFORM="${ARQMA_WALLET_FFI_PLATFORM:-${1:-}}"
case "$PLATFORM" in
  linux|macos|mingw) ;;
  "")
    echo "error: set ARQMA_WALLET_FFI_PLATFORM or pass linux|macos|mingw" >&2
    exit 1
    ;;
  *)
    echo "error: unknown platform: $PLATFORM (use linux, macos, or mingw)" >&2
    exit 1
    ;;
esac

bash "$ROOT/build/ci/patch-arqma-epee-floor.sh" "$UP"
if [[ "$PLATFORM" == mingw ]]; then
  bash "$ROOT/build/ci/patch-arqma-mingw-gui.sh" "$UP"
fi

use_depends=false
if [[ "${ARQMA_WALLET_FFI_USE_DEPENDS:-}" =~ ^(1|true|yes|YES|TRUE)$ ]] && [[ "$PLATFORM" != mingw ]]; then
  use_depends=true
fi

# Trim configure-time work: full project is still configured, but we skip doc/debug extras.
CMAKE_EXTRA=(
  -D CMAKE_BUILD_TYPE=Release
  -D BUILD_GUI_DEPS=ON
  -D BUILD_TESTS=OFF
  -D BUILD_DOCUMENTATION=OFF
  -D BUILD_DEBUG_UTILITIES=OFF
)

# Keep in sync with rust/arqma-wallet2-api/build.rs (rustc-link-search for epee, easylogging, randomx, lmdb, cryptonote_basic).
WALLET_FFI_TARGETS=(epee easylogging randomx lmdb cryptonote_format_utils_basic wallet_merged)

if [[ "$PLATFORM" == macos ]]; then
  J="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
else
  J="$(nproc 2>/dev/null || echo 4)"
fi

if $use_depends; then
  case "$PLATFORM" in
    linux)
      bash "$ROOT/build/ci/install-arqma-depends-linux.sh"
      DEPENDS_HOST="x86_64-unknown-linux-gnu"
      ;;
    macos)
      case "$(uname -m)" in
        arm64) DEPENDS_HOST="aarch64-apple-darwin" ;;
        x86_64) DEPENDS_HOST="x86_64-apple-darwin" ;;
        *)
          echo "error: unsupported macOS machine: $(uname -m)" >&2
          exit 1
          ;;
      esac
      ;;
  esac

  if [[ "$PLATFORM" == macos ]]; then
    if [[ -z "${SDKROOT:-}" ]]; then
      SDKROOT="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
      export SDKROOT
    fi
    if [[ -z "${SDKROOT:-}" ]]; then
      echo "error: macOS SDKROOT is empty; install Xcode / Command Line Tools" >&2
      exit 1
    fi
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
    echo "[build-arqma-wallet-ffi-deps] using SDKROOT=${SDKROOT} MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET}"
  fi

  BUILD_DIR="${ARQMA_CMAKE_BUILD_DIR:-$UP/build/ci-depends-release}"
  mkdir -p "$UP/contrib/depends/built" "$UP/contrib/depends/sources"

  echo "[build-arqma-wallet-ffi-deps] contrib/depends (HOST=$DEPENDS_HOST) — this can take a long time on a cold cache"
  make -C "$UP/contrib/depends" "HOST=$DEPENDS_HOST" -j"$J"

  # Static ICU into the same depends lib/ tree (not part of upstream packages.mk) so the Flutter FFI
  # cdylib can link Boost.Locale without runtime libicu*.so.
  bash "$ROOT/build/ci/build-icu-static-into-depends.sh" "$UP" "$DEPENDS_HOST"

  TOOLCHAIN="$UP/contrib/depends/$DEPENDS_HOST/share/toolchain.cmake"
  test -f "$TOOLCHAIN" || {
    echo "error: missing toolchain file: $TOOLCHAIN" >&2
    exit 1
  }

  mkdir -p "$BUILD_DIR"
  echo "[build-arqma-wallet-ffi-deps] cmake (depends toolchain + STATIC=ON) -> $BUILD_DIR"
  cmake -S "$UP" -B "$BUILD_DIR" \
    -DCMAKE_TOOLCHAIN_FILE="$TOOLCHAIN" \
    "${CMAKE_EXTRA[@]}" \
    -D STATIC=ON

  cmake --build "$BUILD_DIR" --target "${WALLET_FFI_TARGETS[@]}" -j"$J"

  test -f "$BUILD_DIR/src/wallet/libwallet_merged.a"
  echo "[build-arqma-wallet-ffi-deps] OK ($PLATFORM, depends): $BUILD_DIR/src/wallet/libwallet_merged.a"
  exit 0
fi

if [[ "$PLATFORM" == mingw ]]; then
  BUILD_DIR="${ARQMA_MINGW_BUILD_DIR:-$UP/build-mingw}"
else
  BUILD_DIR="${ARQMA_CMAKE_BUILD_DIR:-$UP/build/ci-native-release}"
fi
mkdir -p "$BUILD_DIR"

if [[ "$PLATFORM" == mingw ]]; then
  cmake -S "$UP" -B "$BUILD_DIR" \
    -G "MinGW Makefiles" \
    "${CMAKE_EXTRA[@]}" \
    -D CMAKE_SYSTEM_PROCESSOR=x86_64 \
    -D ARCH_ID=x86_64 \
    -D ARCH=native
else
  cmake -S "$UP" -B "$BUILD_DIR" "${CMAKE_EXTRA[@]}"
fi

cmake --build "$BUILD_DIR" --target "${WALLET_FFI_TARGETS[@]}" -j"$J"

test -f "$BUILD_DIR/src/wallet/libwallet_merged.a"

echo "[build-arqma-wallet-ffi-deps] OK ($PLATFORM): $BUILD_DIR/src/wallet/libwallet_merged.a"
