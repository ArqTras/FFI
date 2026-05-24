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

# macOS/iOS CMake may already fold epee/easylogging/randomx via libtool POST_BUILD on wallet_merged.
# `nm` alone is unreliable (mangled names / thin archives). Member names from `ar t` are definitive.
wallet_merged_archive_listing() {
  local ar="$1"
  local listing=""
  for ar_cmd in llvm-ar ar; do
    if command -v "${ar_cmd}" >/dev/null 2>&1; then
      listing="$("${ar_cmd}" t "${ar}" 2>/dev/null)" && break
    fi
  done
  printf '%s' "${listing}"
}

wallet_merged_already_folded() {
  local ar="$1"
  local listing
  listing="$(wallet_merged_archive_listing "${ar}")"
  if [[ -z "${listing}" ]]; then
    wallet_merged_contains_epee_nm "${ar}"
    return $?
  fi
  # Any epee object member ⇒ aux libs were folded (CMake libtool or a prior run of this script).
  if echo "${listing}" | grep -qE '(^|/)hex\.cpp\.o$|(^|/)wipeable_string\.cpp\.o$|(^|/)easylogging\+\+\\.cc\\.o$'; then
    return 0
  fi
  return 1
}

wallet_merged_contains_epee_nm() {
  local ar="$1"
  local out=""
  for cmd in "llvm-nm -g" "nm -g"; do
    set +e
    out="$(${cmd} "${ar}" 2>/dev/null)"
    set -e
    if [[ -n "${out}" ]] && echo "${out}" | grep -qE 'epee.*to_hex|_ZN4epee6to_hex'; then
      return 0
    fi
  done
  return 1
}

if wallet_merged_already_folded "${WALLET_A}"; then
  echo "[fold-wallet-merged] OK (aux objects already in archive): ${WALLET_A}"
  exit 0
fi

# GNU libtool (macOS) supports `libtool -static` to merge `.a` files. MSYS2's `libtool` is a different
# program and rejects `-static`; use python extract/repack on MinGW instead.
libtool_supports_static_fat_archive() {
  case "$(uname -s 2>/dev/null)" in
    MINGW* | MSYS* | CYGWIN*) return 1 ;;
  esac
  if ! command -v libtool >/dev/null 2>&1; then
    return 1
  fi
  libtool --help 2>/dev/null | grep -qE '[[:space:]]-static[[:space:]]'
}

if libtool_supports_static_fat_archive; then
  echo "[fold-wallet-merged] libtool -static -> ${WALLET_A}"
  libtool -static -o "${WALLET_A}.fat" "${WALLET_A}" "${EPEE}" "${EASY}" "${RX}"
  mv -f "${WALLET_A}.fat" "${WALLET_A}"
  echo "[fold-wallet-merged] $(wc -c < "${WALLET_A}" | tr -d ' ') bytes"
  exit 0
fi

resolve_python() {
  for py in python3 python; do
    if command -v "${py}" >/dev/null 2>&1; then
      echo "${py}"
      return 0
    fi
  done
  return 1
}

PY="$(resolve_python)" || {
  echo "[fold-wallet-merged] need libtool or python for extract/repack (install mingw-w64-x86_64-libtool / python)" >&2
  exit 127
}

echo "[fold-wallet-merged] ${PY} extract/repack -> ${WALLET_A}"
"${PY}" - "${WALLET_A}" "${EPEE}" "${EASY}" "${RX}" <<'PY'
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
