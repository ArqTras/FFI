#!/usr/bin/env bash
# MinGW g++ 16+ (MSYS2): unqualified floor() in epee fails without <cmath> (abstract_http_client.cpp).
# Idempotent; safe on all platforms.
set -eu
UP="${1:-}"
if [[ -z "$UP" ]]; then
  echo "usage: $0 <arqma-upstream-root>" >&2
  exit 1
fi
F="$UP/contrib/epee/src/abstract_http_client.cpp"
[[ -f "$F" ]] || exit 0
if grep -q "Arqma-GUI-MM: patch epee floor" "$F"; then
  exit 0
fi
perl -0777 -i -pe '
  if (!/Arqma-GUI-MM: patch epee floor/) {
    s/(#include "misc_log_ex\.h"\r?\n)/$1#include <cmath> \/\/ Arqma-GUI-MM: patch epee floor (MinGW GCC 16+)\n/s;
    s/\(int\)floor\(/\(int\)std::floor(/g;
  }
' "$F"
echo "[patch-arqma-epee-floor] patched ${F#$UP/}"
