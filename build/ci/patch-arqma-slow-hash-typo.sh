#!/usr/bin/env bash
# Fix upstream typo in slow-hash.c (reuslt -> result) for Android/depends builds.
set -eu
UP="${1:?upstream dir}"
F="${UP}/src/crypto/slow-hash.c"
if [[ ! -f "${F}" ]]; then
  exit 0
fi
if grep -q 'reuslt' "${F}"; then
  sed -i 's/reuslt/result/g' "${F}"
  echo "[patch-arqma-slow-hash-typo] fixed reuslt in ${F}"
fi
