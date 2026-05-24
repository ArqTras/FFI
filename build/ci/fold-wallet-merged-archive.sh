#!/usr/bin/env bash
# Fold epee / easylogging / randomx into libwallet_merged.a (GNU ar MRI fails on LLVM/thin .a in CI).
set -euo pipefail
BUILD_DIR="${1:?cmake build directory}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WALLET_A="${BUILD_DIR}/src/wallet/libwallet_merged.a"

find_lib() {
  local name="$1"
  local p
  p="$(find "${BUILD_DIR}" -name "${name}" -type f 2>/dev/null | head -1)"
  if [[ -n "${p}" && -f "${p}" ]]; then
    echo "${p}"
    return 0
  fi
  return 1
}

EPEE="$(find_lib libepee.a)" || true
EASY="$(find_lib libeasylogging.a)" || true
RX="$(find_lib librandomx.a)" || true

if [[ ! -f "${WALLET_A}" ]]; then
  echo "[fold-wallet-merged] missing ${WALLET_A}" >&2
  exit 1
fi
if [[ -z "${EPEE}" || -z "${EASY}" || -z "${RX}" ]]; then
  echo "[fold-wallet-merged] skip (aux libs not built yet under ${BUILD_DIR})" >&2
  exit 0
fi

# Do not skip fold based on file size alone — `wallet_merged` can be >1MB but still miss epee
# (breaks Android dlopen: undefined `epee::to_hex`). Fold whenever aux `.a` files exist.

if command -v libtool >/dev/null 2>&1; then
  echo "[fold-wallet-merged] libtool -static -> ${WALLET_A}"
  libtool -static -o "${WALLET_A}.fat" "${WALLET_A}" "${EPEE}" "${EASY}" "${RX}"
  mv -f "${WALLET_A}.fat" "${WALLET_A}"
  echo "[fold-wallet-merged] $(wc -c < "${WALLET_A}" | tr -d ' ') bytes"
  exit 0
fi

echo "[fold-wallet-merged] python extract/repack -> ${WALLET_A}"
python3 - "${WALLET_A}" "${EPEE}" "${EASY}" "${RX}" <<'PY'
import glob
import os
import shutil
import subprocess
import sys
import tempfile

archives = sys.argv[1:]

def extract(ar_path: str, dest: str) -> None:
    for cmd in (["ar", "x"], ["llvm-ar", "x"]):
        try:
            subprocess.run(cmd + [ar_path], cwd=dest, check=True, capture_output=True)
            return
        except (subprocess.CalledProcessError, FileNotFoundError):
            continue
    raise RuntimeError(f"cannot extract objects from {ar_path}")

tmpdir = tempfile.mkdtemp(prefix="wallet_merged_fold_")
objs: list[str] = []
try:
    for ar in archives:
        sub = os.path.join(tmpdir, os.path.basename(ar))
        os.makedirs(sub, exist_ok=True)
        extract(ar, sub)
        objs.extend(glob.glob(os.path.join(sub, "*.o")))
    if not objs:
        raise RuntimeError("no object files extracted")
    out = archives[0]
    subprocess.run(["ar", "qc", out, *objs], check=True)
    subprocess.run(["ranlib", out], check=True)
    print(f"Repacked {out} ({os.path.getsize(out)} bytes)")
finally:
    shutil.rmtree(tmpdir, ignore_errors=True)
PY

if [[ "$(wc -c < "${WALLET_A}" | tr -d ' ')" -lt 1048576 ]]; then
  echo "[fold-wallet-merged] repack produced small archive; check ${BUILD_DIR}" >&2
  exit 1
fi
echo "[fold-wallet-merged] OK: ${WALLET_A}"
