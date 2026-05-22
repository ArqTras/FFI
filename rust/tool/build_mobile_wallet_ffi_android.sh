#!/usr/bin/env bash
# Build arqma-wallet-flutter-ffi for Android (arm64 phones + x86_64 emulator by default).
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
for _sh in "${ROOT}/rust/tool/"*.sh "${ROOT}/build/ci/"*.sh; do
  [[ -f "${_sh}" ]] && sed -i 's/\r$//' "${_sh}" 2>/dev/null || true
done
set -euo pipefail
cd "${ROOT}/rust"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/rust/target}"

export PATH="${HOME}/.cargo/bin:${PATH}"
export ARQMA_WALLET2_UPSTREAM_DIR="${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}"

# Phones + emulator unless narrowed.
TARGETS=(aarch64-linux-android)
BUILD_X86=1
if [[ "${BUILD_ANDROID_X86_64:-1}" == "0" ]]; then
  BUILD_X86=0
fi
if [[ "${BUILD_ANDROID_X86_64:-1}" == "1" ]]; then
  TARGETS+=(x86_64-linux-android)
fi
if [[ "${BUILD_ANDROID_ARMV7:-0}" == "1" ]]; then
  TARGETS+=(armv7-linux-androideabi)
fi

for t in "${TARGETS[@]}"; do
  if ! rustup target list --installed | grep -q "${t}"; then
    rustup target add "${t}"
  fi
done

depends_host_for_triple() {
  case "$1" in
    aarch64-linux-android) echo aarch64-linux-android ;;
    x86_64-linux-android) echo x86_64-linux-android ;;
    armv7-linux-androideabi) echo armv7-linux-androideabi ;;
    *) echo "unknown triple: $1" >&2; return 1 ;;
  esac
}

wallet_merged_dir_for_host() {
  local host="$1"
  local dir="${ARQMA_WALLET2_UPSTREAM_DIR}/build-android-depends-${host}/src/wallet"
  if [[ -f "${dir}/libwallet_merged.a" ]]; then
    echo "${dir}"
    return 0
  fi
  dir="$(dirname "$(find "${ARQMA_WALLET2_UPSTREAM_DIR}/build-android-depends-${host}" -name 'libwallet_merged.a' -print -quit 2>/dev/null || true)")"
  if [[ -n "${dir}" && -f "${dir}/libwallet_merged.a" ]]; then
    echo "${dir}"
    return 0
  fi
  return 1
}

if [[ "${ARQMA_SKIP_ANDROID_WALLET_MERGED:-0}" != "1" ]]; then
  hosts_built=()
  for t in "${TARGETS[@]}"; do
    host="$(depends_host_for_triple "${t}")"
    skip=0
    for h in "${hosts_built[@]:-}"; do
      [[ "${h}" == "${host}" ]] && skip=1 && break
    done
    if [[ "${skip}" == "1" ]]; then
      continue
    fi
    hosts_built+=("${host}")
    ARQMA_SKIP_DEPENDS_CRLF=1 ARQMA_ANDROID_DEPENDS_HOST="${host}" \
      bash "${ROOT}/rust/tool/build_android_wallet_merged.sh"
  done
fi

NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [[ -z "${NDK_HOME}" && -n "${ANDROID_HOME:-}" ]]; then
  NDK_HOME="$(ls -d "${ANDROID_HOME}"/ndk/* 2>/dev/null | sort -V | tail -1)"
fi
if [[ -n "${NDK_HOME}" ]]; then
  export NDK_HOME
  shopt -s nullglob
  for prebuilt in "${NDK_HOME}"/toolchains/llvm/prebuilt/*; do
    export CC_aarch64_linux_android="${prebuilt}/bin/aarch64-linux-android21-clang"
    export CXX_aarch64_linux_android="${prebuilt}/bin/aarch64-linux-android21-clang++"
    export CC_x86_64_linux_android="${prebuilt}/bin/x86_64-linux-android21-clang"
    export CXX_x86_64_linux_android="${prebuilt}/bin/x86_64-linux-android21-clang++"
    export CC_armv7_linux_androideabi="${prebuilt}/bin/armv7a-linux-androideabi21-clang"
    export CXX_armv7_linux_androideabi="${prebuilt}/bin/armv7a-linux-androideabi21-clang++"
    break
  done
  shopt -u nullglob
fi

export ARQMA_WALLET_FFI_STATIC_HYBRID=1
export ARQMA_WALLET_FFI_USE_DEPENDS=1

build_ffi() {
  local triple="$1"
  local host
  host="$(depends_host_for_triple "${triple}")"
  local lib_di
  lib_dir="$(wallet_merged_dir_for_host "${host}")" || {
    echo "Missing libwallet_merged.a for ${host}; run build_android_wallet_merged.sh" >&2
    exit 1
  }
  echo "Building arqma-wallet-flutter-ffi for ${triple} (wallet_merged=${lib_dir})..."
  ARQMA_WALLET2_LIB_DIR="${lib_dir}" \
  ARQMA_WALLET_FFI_DEPENDS_LIB_DIR="${ARQMA_WALLET2_UPSTREAM_DIR}/contrib/depends/${host}/lib" \
    cargo build -p arqma-wallet-flutter-ffi --release --target "${triple}"
}

for t in "${TARGETS[@]}"; do
  build_ffi "${t}"
done

echo "Artifacts:"
for t in "${TARGETS[@]}"; do
  ls -la "${ROOT}/rust/target/${t}/release/libarqma_wallet_flutter_ffi.so" 2>/dev/null || true
done

echo "Copy into Flutter:"
echo "  bash flutter-android/arqma_wallet_android/tool/copy_android_wallet_ffi.sh"
