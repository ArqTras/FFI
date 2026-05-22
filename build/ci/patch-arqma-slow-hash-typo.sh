#!/usr/bin/env bash
# Fix upstream typo in slow-hash.c (reuslt -> result) for Android/depends builds.
set -eu
UP="${1:?upstream dir}"
F="${UP}/src/crypto/slow-hash.c"
if [[ ! -f "${F}" ]]; then
  exit 0
fi
if grep -q 'reuslt' "${F}"; then
  if [[ "$(uname -s)" == Darwin ]]; then
    sed -i '' 's/reuslt/result/g' "${F}"
  else
    sed -i 's/reuslt/result/g' "${F}"
  fi
  echo "[patch-arqma-slow-hash-typo] fixed reuslt in ${F}"
fi
