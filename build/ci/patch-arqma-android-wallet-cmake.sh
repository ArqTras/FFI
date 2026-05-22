#!/usr/bin/env bash
# Ensure Android wallet_merged merge uses GNU ar (llvm-ar does not support MRI OPEN/ADDLIB).
set -euo pipefail
UP="${1:?upstream dir}"
CM="$UP/src/wallet/CMakeLists.txt"
if grep -q 'ARQMA_WALLET_MERGE_AR' "$CM" 2>/dev/null; then
  exit 0
fi
python3 - "$CM" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
t = p.read_text()
needle = '      elseif(WIN32 AND NOT MINGW)'
if needle not in t:
    sys.exit('CMakeLists pattern not found')
insert = '''      elseif(ANDROID)
        find_program(ARQMA_WALLET_MERGE_AR NAMES aarch64-linux-gnu-ar ar REQUIRED)
        add_custom_command(TARGET wallet_merged POST_BUILD
          COMMAND ${CMAKE_COMMAND}
            -DCMAKE_AR=${ARQMA_WALLET_MERGE_AR}
            -DWALLET_MERGED=$<TARGET_FILE:wallet_merged>
            -DLIBEPEE=$<TARGET_FILE:epee>
            -DLIBEASYLOGGING=$<TARGET_FILE:easylogging>
            -DLIBRANDOMX=$<TARGET_FILE:randomx>
            -P "${CMAKE_CURRENT_SOURCE_DIR}/wallet_merge_gnu_ar.cmake"
          COMMENT "Fold epee/easylogging/randomx into libwallet_merged.a (Android GNU ar MRI)"
          VERBATIM)
'''
p.write_text(t.replace(needle, insert + needle, 1))
print('patched', p)
PY
