#!/usr/bin/env bash
# iOS wallet_merged: skip ExternalProject generate_translations_header (host lrelease + iOS link/OOM).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
MARKER="ARQMA_SKIP_EMBEDDED_TRANSLATIONS"
CM="${UP}/CMakeLists.txt"
COMMON="${UP}/src/common/CMakeLists.txt"
STUB="${UP}/translations/translation_files.stub.h"

[[ -f "${CM}" ]] || { echo "missing ${CM}" >&2; exit 1; }

if grep -q "${MARKER}" "${CM}"; then
  echo "[patch-arqma-ios-translations] already patched"
  exit 0
fi

[[ -f "${STUB}" ]] || {
  echo "missing ${STUB} (apply upstream-overlay first)" >&2
  exit 1
}

python3 - "${CM}" "${COMMON}" "${MARKER}" <<'PY'
import pathlib
import re
import sys

cm_path, common_path, marker = sys.argv[1:4]
cm = pathlib.Path(cm_path)
text = cm.read_text()

option = f'option({marker} "Skip Qt/lrelease embedded translations (mobile FFI)" OFF)\n'
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

new_ep = f"""if(NOT {marker})
ExternalProject_Add(generate_translations_header
  SOURCE_DIR "${{CMAKE_CURRENT_SOURCE_DIR}}/translations"
  BINARY_DIR "${{CMAKE_CURRENT_BINARY_DIR}}/translations"
  STAMP_DIR ${{LRELEASE_PATH}}
  CMAKE_ARGS -DLRELEASE_PATH=${{LRELEASE_PATH}}
  INSTALL_COMMAND ${{CMAKE_COMMAND}} -E echo "")
endif()
if({marker})
  file(MAKE_DIRECTORY "${{CMAKE_CURRENT_BINARY_DIR}}/translations")
  configure_file(
    "${{CMAKE_CURRENT_SOURCE_DIR}}/translations/translation_files.stub.h"
    "${{CMAKE_CURRENT_BINARY_DIR}}/translations/translation_files.h"
    COPYONLY)
endif()"""

if old_ep not in text:
    raise SystemExit("CMakeLists.txt: generate_translations_header block not found")
text = text.replace(old_ep, new_ep, 1)
cm.write_text(text)

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
common = pat.sub(replacement, common, count=1)
pathlib.Path(common_path).write_text(common)
print("[patch-arqma-ios-translations] patched CMakeLists.txt and src/common/CMakeLists.txt")
PY

echo "[patch-arqma-ios-translations] ok"
