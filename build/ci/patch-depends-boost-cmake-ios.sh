#!/usr/bin/env bash
# boost_thread-config.cmake calls find_dependency(Threads); skip on iOS (no -pthread).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
HOST="${2:-aarch64-apple-ios}"
MARKER="ARQMA_IOS_BOOST_THREADS"
PREFIX="${UP}/contrib/depends/${HOST}"
CMAKE_DIR="${PREFIX}/lib/cmake"

[[ -d "${CMAKE_DIR}" ]] || {
  echo "[patch-depends-boost-cmake-ios] skip (no ${CMAKE_DIR})"
  exit 0
}

python3 - "${CMAKE_DIR}" "${MARKER}" <<'PY'
import glob
import pathlib
import sys

cmake_dir, marker = sys.argv[1:3]
patched = 0
for path in sorted(glob.glob(str(pathlib.Path(cmake_dir) / "boost_thread-*/boost_thread-config.cmake"))):
    p = pathlib.Path(path)
    text = p.read_text()
    if marker in text:
        continue
    old = "find_dependency(Threads)"
    if old not in text:
        old = "find_dependency(Threads REQUIRED)"
    if old not in text:
        print(f"[patch-depends-boost-cmake-ios] skip {p.name} (no find_dependency(Threads))")
        continue
    new = f"""# {marker}
if(NOT IOS)
  {old}
else()
  if(NOT TARGET Threads::Threads)
    add_library(Threads::Threads INTERFACE IMPORTED GLOBAL)
  endif()
endif()"""
    p.write_text(text.replace(old, new, 1))
    patched += 1
    print(f"[patch-depends-boost-cmake-ios] {p}")
if patched == 0:
    raise SystemExit("boost_thread-config.cmake not patched")
PY

echo "[patch-depends-boost-cmake-ios] ok"
