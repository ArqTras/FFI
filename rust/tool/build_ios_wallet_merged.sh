#!/usr/bin/env bash
# Build libwallet_merged.a for iOS device (contrib/depends + CMake STATIC).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UPSTREAM="${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}"
DEPENDS_HOST="${ARQMA_IOS_DEPENDS_HOST:-aarch64-apple-ios}"
export PATH="/usr/bin:/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"
J="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"

bash "${ROOT}/build/ci/patch-arqma-epee-floor.sh" "${UPSTREAM}" 2>/dev/null || true
bash "${ROOT}/build/ci/patch-zeromq-ios-host.sh" "${UPSTREAM}"

if [[ "${ARQMA_SKIP_IOS_DEPENDS:-0}" != "1" ]]; then
  echo "==> contrib/depends (HOST=${DEPENDS_HOST}) — cold build can take 30–90+ minutes"
  mkdir -p "${UPSTREAM}/contrib/depends/built" "${UPSTREAM}/contrib/depends/sources"
  make -C "${UPSTREAM}/contrib/depends" "HOST=${DEPENDS_HOST}" -j"${J}"
  bash "${ROOT}/build/ci/build-icu-static-into-depends.sh" "${UPSTREAM}" "${DEPENDS_HOST}"
fi

TOOLCHAIN="${UPSTREAM}/contrib/depends/${DEPENDS_HOST}/share/toolchain.cmake"
if [[ ! -f "${TOOLCHAIN}" ]]; then
  echo "Missing depends toolchain: ${TOOLCHAIN}" >&2
  exit 1
fi

BUILD_DIR="${UPSTREAM}/build-ios-depends-device"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

echo "==> CMake iOS (depends toolchain)"
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

merged="${BUILD_DIR}/src/wallet/libwallet_merged.a"
if [[ ! -f "${merged}" ]] || [[ "$(wc -c < "${merged}" | tr -d ' ')" -lt 1048576 ]]; then
  echo "==> Repacking libwallet_merged.a"
  python3 - "${BUILD_DIR}" <<'PY'
import os, shlex, subprocess, sys
build = sys.argv[1]
link = os.path.join(build, "src/wallet/CMakeFiles/wallet_merged.dir/link.txt")
line = open(link).read().splitlines()[0]
parts = shlex.split(line.replace("/opt/homebrew/bin/ccache ", "", 1))
objs = [p for p in parts if p.endswith(".o")]
wallet = os.path.join(build, "src/wallet")
out = os.path.join(wallet, "libwallet_merged.a")
subprocess.run(["/usr/bin/ar", "qc", out] + objs, cwd=wallet, check=True)
subprocess.run(["/usr/bin/ranlib", out], check=True)
print(f"Repacked {out} ({os.path.getsize(out)} bytes)")
PY
fi

if [[ ! -f "${merged}" ]] || [[ "$(wc -c < "${merged}" | tr -d ' ')" -lt 1048576 ]]; then
  merged="$(find "${BUILD_DIR}" -name 'libwallet_merged.a' -size +1M -print -quit)"
fi
if [[ -z "${merged}" || ! -f "${merged}" ]]; then
  echo "libwallet_merged.a not found under ${BUILD_DIR}" >&2
  exit 1
fi
echo "Device wallet_merged: ${merged}"
