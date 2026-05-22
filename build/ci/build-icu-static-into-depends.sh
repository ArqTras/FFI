#!/usr/bin/env bash
# Build ICU4C as static PIC archives and install into contrib/depends/<HOST>/lib so the
# wallet Flutter FFI can fold Boost.Locale ICU users without runtime libicu*.so.N.
#
# Idempotent: skips if libicuuc.a, libicui18n.a, and libicudata.a already exist.
#
# Usage:
#   bash build/ci/build-icu-static-into-depends.sh <arqma-upstream-root> <depends-host-triplet>
# Example:
#   bash build/ci/build-icu-static-into-depends.sh rust/arqma-rpc-upstream x86_64-unknown-linux-gnu
#
# Env:
#   ICU_VERSION_TAG  — default release-74-2 (matches icu4c-74_2-src.tgz)
#   ICU_SRC_TGZ      — override tarball name under the GitHub release (default icu4c-74_2-src.tgz)
set -euo pipefail

UP="$(cd "${1:?usage: $0 <upstream-root> <depends-host>}" && pwd)"
HOST="${2:?usage: $0 <upstream-root> <depends-host>}"
PREFIX="${UP}/contrib/depends/${HOST}"
LIBDIR="${PREFIX}/lib"

mkdir -p "${LIBDIR}"

if [[ -f "${LIBDIR}/libicuuc.a" && -f "${LIBDIR}/libicui18n.a" && -f "${LIBDIR}/libicudata.a" ]]; then
  echo "[build-icu-static-into-depends] ICU static libs already present in ${LIBDIR}"
  exit 0
fi

ICU_VERSION_TAG="${ICU_VERSION_TAG:-release-74-2}"
ICU_SRC_TGZ="${ICU_SRC_TGZ:-icu4c-74_2-src.tgz}"
URL="https://github.com/unicode-org/icu/releases/download/${ICU_VERSION_TAG}/${ICU_SRC_TGZ}"

WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/arqma-icu-build.XXXXXX")"
trap 'rm -rf "${WORKDIR}"' EXIT

echo "[build-icu-static-into-depends] downloading ${URL}"
curl -fsSL -o "${WORKDIR}/${ICU_SRC_TGZ}" "${URL}"
tar -C "${WORKDIR}" -xzf "${WORKDIR}/${ICU_SRC_TGZ}"

ICU_SRC_ROOT="$(find "${WORKDIR}" -maxdepth 1 -type d -name 'icu' | head -n1)"
if [[ -z "${ICU_SRC_ROOT}" || ! -d "${ICU_SRC_ROOT}/source" ]]; then
  echo "error: expected icu/source under extracted tarball" >&2
  exit 1
fi

cd "${ICU_SRC_ROOT}/source"

COMMON_OPTS=(
  --prefix="${PREFIX}"
  --disable-shared
  --enable-static
  --with-data-packaging=static
  --disable-tests
  --disable-samples
  --disable-extras
  --disable-layoutex
)

case "${HOST}" in
  *linux*)
    # PIC required when folding ICU .a into a Linux cdylib.
    export CFLAGS="-fPIC ${CFLAGS:-}"
    export CXXFLAGS="-fPIC ${CXXFLAGS:-}"
    ./configure "${COMMON_OPTS[@]}"
    ;;
  *apple-darwin*)
    export CFLAGS="-fPIC ${CFLAGS:-}"
    export CXXFLAGS="-fPIC ${CXXFLAGS:-}"
    ./runConfigureICU MacOSX/GCC "${COMMON_OPTS[@]}"
    ;;
  *apple-ios)
    IOS_SDK="$(xcrun --sdk iphoneos --show-sdk-path)"
    NATIVE_PREFIX="${WORKDIR}/icu-native-install"
    NATIVE_BUILD="${WORKDIR}/icu-native-build"
    mkdir -p "${NATIVE_BUILD}"
    (
      cd "${NATIVE_BUILD}"
      "${ICU_SRC_ROOT}/source/configure" \
        --prefix="${NATIVE_PREFIX}" \
        --disable-shared --enable-static \
        --with-data-packaging=static \
        --disable-tests --disable-samples --disable-extras --disable-layoutex
      make -j"$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
      make install
    )
    export CC="$(xcrun --sdk iphoneos --find clang)"
    export CXX="$(xcrun --sdk iphoneos --find clang++)"
    export CFLAGS="-fPIC -arch arm64 -isysroot${IOS_SDK} -miphoneos-version-min=13.0"
    export CXXFLAGS="${CFLAGS}"
    export LDFLAGS="-arch arm64 -isysroot${IOS_SDK} -miphoneos-version-min=13.0"
    ./configure --host=aarch64-apple-ios --disable-dyload --disable-tools \
      --with-cross-build="${NATIVE_PREFIX}" "${COMMON_OPTS[@]}"
    ;;
  *)
    echo "error: unsupported depends host for ICU build: ${HOST}" >&2
    exit 1
    ;;
esac

make -j"$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
make install

if [[ ! -f "${LIBDIR}/libicuuc.a" || ! -f "${LIBDIR}/libicui18n.a" || ! -f "${LIBDIR}/libicudata.a" ]]; then
  echo "error: ICU install did not produce expected archives under ${LIBDIR}" >&2
  exit 1
fi

echo "[build-icu-static-into-depends] OK -> ${LIBDIR}/libicu{uc,i18n,data}.a"
