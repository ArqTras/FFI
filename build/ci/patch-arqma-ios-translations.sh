#!/usr/bin/env bash
# iOS wallet_merged CMake fixes: skip embedded translations; shim FindThreads for cross-compile.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
TRANS_MARKER="ARQMA_SKIP_EMBEDDED_TRANSLATIONS"
THREADS_MARKER="ARQMA_IOS_THREADS_SHIM"
BOOST_THREADS_MARKER="ARQMA_IOS_BOOST_THREADS"
CM="${UP}/CMakeLists.txt"
COMMON="${UP}/src/common/CMakeLists.txt"
ELOG="${UP}/external/easylogging++/CMakeLists.txt"
STUB="${UP}/translations/translation_files.stub.h"

[[ -f "${CM}" ]] || { echo "missing ${CM}" >&2; exit 1; }

trans_done=0
threads_done=0
boost_done=0
grep -q "${TRANS_MARKER}" "${CM}" && trans_done=1
grep -q "${THREADS_MARKER}" "${CM}" && threads_done=1
grep -q "${BOOST_THREADS_MARKER}" "${CM}" && boost_done=1
if [[ "${trans_done}" -eq 1 && "${threads_done}" -eq 1 && "${boost_done}" -eq 1 ]]; then
  echo "[patch-arqma-ios-translations] already patched"
  exit 0
fi

if [[ "${trans_done}" -eq 0 ]]; then
  [[ -f "${STUB}" ]] || {
    echo "missing ${STUB} (apply upstream-overlay first)" >&2
    exit 1
  }
fi

python3 - "${CM}" "${COMMON}" "${ELOG}" "${TRANS_MARKER}" "${THREADS_MARKER}" "${BOOST_THREADS_MARKER}" "${trans_done}" "${threads_done}" "${boost_done}" <<'PY'
import pathlib
import re
import sys

cm_path, common_path, elog_path, trans_marker, threads_marker, boost_marker, trans_done, threads_done, boost_done = sys.argv[1:10]
trans_done = trans_done == "1"
threads_done = threads_done == "1"
boost_done = boost_done == "1"
cm = pathlib.Path(cm_path)
text = cm.read_text()

if not trans_done:
    option = f'option({trans_marker} "Skip Qt/lrelease embedded translations (mobile FFI)" OFF)\n'
    if option not in text:
        text = text.replace(
            "include(ExternalProject)\n",
            option + "include(ExternalProject)\n",
            1,
        )

    old_ep = """ExternalProject_Add(generate_translations_header
  SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/translations"
  BINARY_DIR "${CMAKE_CURRENT_BINARY_DIR}/translations"
  STAMP_DIR ${LRELEASE_PATH}
  CMAKE_ARGS -DLRELEASE_PATH=${LRELEASE_PATH}
  INSTALL_COMMAND ${CMAKE_COMMAND} -E echo "")"""

    new_ep = f"""if(NOT {trans_marker})
ExternalProject_Add(generate_translations_header
  SOURCE_DIR "${{CMAKE_CURRENT_SOURCE_DIR}}/translations"
  BINARY_DIR "${{CMAKE_CURRENT_BINARY_DIR}}/translations"
  STAMP_DIR ${{LRELEASE_PATH}}
  CMAKE_ARGS -DLRELEASE_PATH=${{LRELEASE_PATH}}
  INSTALL_COMMAND ${{CMAKE_COMMAND}} -E echo "")
endif()
if({trans_marker})
  file(MAKE_DIRECTORY "${{CMAKE_CURRENT_BINARY_DIR}}/translations")
  configure_file(
    "${{CMAKE_CURRENT_SOURCE_DIR}}/translations/translation_files.stub.h"
    "${{CMAKE_CURRENT_BINARY_DIR}}/translations/translation_files.h"
    COPYONLY)
endif()"""

    if old_ep not in text:
        raise SystemExit("CMakeLists.txt: generate_translations_header block not found")
    text = text.replace(old_ep, new_ep, 1)

    common = pathlib.Path(common_path).read_text()
    pat = re.compile(
        r"(arqma_add_library\(common\n(?:.*\n)*?)(\s+DEPENDS generate_translations_header\))",
        re.MULTILINE,
    )
    m = pat.search(common)
    if not m:
        raise SystemExit("src/common/CMakeLists.txt: common+DEPENDS block not found")
    replacement = (
        "if(NOT ARQMA_SKIP_EMBEDDED_TRANSLATIONS)\n"
        + m.group(1)
        + m.group(2)
        + "\nelse()\n"
        + m.group(1).rstrip("\n")
        + ")\nendif()"
    )
    pathlib.Path(common_path).write_text(pat.sub(replacement, common, count=1))
    print("[patch-arqma-ios-translations] translations stub")

def ios_threads_shim_block(marker: str) -> str:
    return f"""# {marker}
if(IOS)
  set(CMAKE_THREAD_LIBS_INIT "" CACHE STRING "" FORCE)
  set(Threads_FOUND TRUE CACHE BOOL "" FORCE)
  if(NOT TARGET Threads::Threads)
    add_library(Threads::Threads INTERFACE IMPORTED GLOBAL)
  endif()
endif()"""

if not threads_done:
    shim = ios_threads_shim_block(threads_marker)
    old_threads = """set(THREADS_PREFER_PTHREAD_FLAG ON)
find_package(Threads)"""
    new_threads = f"""set(THREADS_PREFER_PTHREAD_FLAG ON)
{shim}
if(NOT IOS)
  find_package(Threads)
endif()"""
    if old_threads in text:
        text = text.replace(old_threads, new_threads, 1)
        print("[patch-arqma-ios-translations] Threads shim (early)")
    elif threads_marker not in text:
        # Already partially patched or upstream drift: insert shim before OpenSSL.
        needle = "find_package(OpenSSL REQUIRED)"
        if needle not in text:
            raise SystemExit("CMakeLists.txt: cannot locate Threads/OpenSSL block")
        text = text.replace(needle, shim + "\n" + needle, 1)
        print("[patch-arqma-ios-translations] Threads shim (inserted)")

if not boost_done:
    old_boost = """set(Boost_version_min 1.66)
find_package(Boost ${Boost_version_min} QUIET REQUIRED)"""
    new_boost = f"""set(Boost_version_min 1.66)
{ios_threads_shim_block(boost_marker)}
find_package(Boost ${{Boost_version_min}} QUIET REQUIRED)"""
    if old_boost not in text:
        raise SystemExit("CMakeLists.txt: find_package(Boost) block not found")
    text = text.replace(old_boost, new_boost, 1)
    print("[patch-arqma-ios-translations] Threads shim (before Boost)")

cm.write_text(text)

elog = pathlib.Path(elog_path)
if elog.is_file() and threads_marker not in elog.read_text():
    et = elog.read_text()
    old = "find_package(Threads)"
    new = f"""# {threads_marker}
if(IOS)
  set(CMAKE_THREAD_LIBS_INIT "" CACHE STRING "" FORCE)
  set(Threads_FOUND TRUE CACHE BOOL "" FORCE)
  if(NOT TARGET Threads::Threads)
    add_library(Threads::Threads INTERFACE IMPORTED GLOBAL)
  endif()
else()
  find_package(Threads)
endif()"""
    if old not in et:
        raise SystemExit("easylogging++/CMakeLists.txt: find_package(Threads) not found")
    elog.write_text(et.replace(old, new, 1))
    print("[patch-arqma-ios-translations] Threads shim in easylogging++")
PY

echo "[patch-arqma-ios-translations] ok"
