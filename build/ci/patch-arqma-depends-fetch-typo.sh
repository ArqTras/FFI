#!/usr/bin/env bash
# contrib/depends/funcs.mk: fetch_file_inner was misspelled fetch_file_inne (breaks android_ndk fetch).
set -euo pipefail
UPSTREAM="${1:?upstream dir}"
FUNCS="${UPSTREAM}/contrib/depends/funcs.mk"
if [[ ! -f "${FUNCS}" ]]; then
  exit 0
fi
if grep -qE '^define fetch_file_inne+$' "${FUNCS}"; then
  sed -i -E 's/^define fetch_file_inne+$/define fetch_file_inner/' "${FUNCS}"
  echo "patched fetch_file_inner typo in ${FUNCS}"
fi
