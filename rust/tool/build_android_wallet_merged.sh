#!/usr/bin/env bash
# Build libwallet_merged.a for Android (contrib/depends + CMake).
# HOST examples: aarch64-linux-android (phones), x86_64-linux-android (emulator).
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UPSTREAM="${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}"
DEPENDS_HOST="${ARQMA_ANDROID_DEPENDS_HOST:-aarch64-linux-android}"
set -euo pipefail
# Fast CRLF on make inputs only (skip when building on WSL native after fix_depends_for_android).
if [[ "${ARQMA_SKIP_DEPENDS_CRLF:-0}" != "1" ]]; then
  bash "$(cd "$(dirname "$0")" && pwd)/fix_depends_for_android.sh" "${UPSTREAM}"
fi
J="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"

bash "${ROOT}/build/ci/patch-arqma-epee-floor.sh" "${UPSTREAM}" 2>/dev/null || true
bash "${ROOT}/build/ci/patch-arqma-depends-fetch-typo.sh" "${UPSTREAM}" 2>/dev/null || true
bash "${ROOT}/build/ci/patch-arqma-android-sdk-ndk.sh" "${UPSTREAM}" 2>/dev/null || true

export ARQMA_USE_SDK_ANDROID_NDK="${ARQMA_USE_SDK_ANDROID_NDK:-1}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [[ "${ARQMA_USE_SDK_ANDROID_NDK}" == "1" && -z "${ANDROID_NDK_HOME}" && -n "${ANDROID_HOME:-}" ]]; then
  ANDROID_NDK_HOME="$(ls -d "${ANDROID_HOME}"/ndk/* 2>/dev/null | sort -V | tail -1)"
  export ANDROID_NDK_HOME
fi
if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
  export ANDROID_NDK="${ANDROID_NDK_HOME}"
fi

if [[ "${ARQMA_SKIP_ANDROID_DEPENDS:-0}" != "1" ]]; then
  echo "==> contrib/depends (HOST=${DEPENDS_HOST}, SDK_NDK=${ARQMA_USE_SDK_ANDROID_NDK})"
  mkdir -p "${UPSTREAM}/contrib/depends/built" "${UPSTREAM}/contrib/depends/sources"
  # Clean boost only when requested (resume after success should not rebuild boost).
  if [[ "${ARQMA_CLEAN_BOOST:-0}" == "1" ]]; then
    rm -rf "${UPSTREAM}/contrib/depends/work/build/${DEPENDS_HOST}/boost" 2>/dev/null || true
    find "${UPSTREAM}/contrib/depends/work/staging/${DEPENDS_HOST}" -maxdepth 1 -type d -name 'boost*' \
      -exec rm -rf {} + 2>/dev/null || true
    rm -f "${UPSTREAM}/contrib/depends/built/${DEPENDS_HOST}/boost/"*.tar.gz* 2>/dev/null || true
  fi
  # boost b2 is fragile under high make -j while sources are still extracting.
  make -C "${UPSTREAM}/contrib/depends" "HOST=${DEPENDS_HOST}" boost -j1
  make -C "${UPSTREAM}/contrib/depends" "HOST=${DEPENDS_HOST}" -j"${J}"
fi

TOOLCHAIN="${UPSTREAM}/contrib/depends/${DEPENDS_HOST}/share/toolchain.cmake"
if [[ ! -f "${TOOLCHAIN}" ]]; then
  echo "Missing depends toolchain: ${TOOLCHAIN}" >&2
  echo "Set ARQMA_SKIP_ANDROID_DEPENDS=1 only if depends is already built." >&2
  exit 1
fi

bash "${ROOT}/build/ci/patch-arqma-android-wallet-cmake.sh" "${UPSTREAM}" 2>/dev/null || true

BUILD_DIR="${UPSTREAM}/build-android-depends-${DEPENDS_HOST}"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

echo "==> CMake Android (HOST=${DEPENDS_HOST})"
cmake -S "${UPSTREAM}" -B "${BUILD_DIR}" \
  -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
  -DBUILD_GUI_DEPS=ON \
  -DSTATIC=ON \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTS=OFF \
  -DBUILD_DOCUMENTATION=OFF \
  -DBUILD_DEBUG_UTILITIES=OFF \
  -DUSE_READLINE=OFF

TARGETS=(epee easylogging randomx lmdb cryptonote_format_utils_basic wallet_merged)
for t in "${TARGETS[@]}"; do
  echo "==> ${t}"
  cmake --build "${BUILD_DIR}" --target "${t}" -j"${J}"
done

bash "${ROOT}/build/ci/fold-wallet-merged-archive.sh" "${BUILD_DIR}"

merged="${BUILD_DIR}/src/wallet/libwallet_merged.a"
if [[ ! -f "${merged}" ]]; then
  merged="$(find "${BUILD_DIR}" -name 'libwallet_merged.a' -size +1M -print -quit)"
fi
if [[ -z "${merged}" || ! -f "${merged}" ]]; then
  echo "libwallet_merged.a not found under ${BUILD_DIR}" >&2
  exit 1
fi
echo "Android wallet_merged (${DEPENDS_HOST}): ${merged}"
