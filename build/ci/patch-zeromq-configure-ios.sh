#!/usr/bin/env bash
# Patch pre-generated libzmq configure script for host_os=ios (configure.ac patch alone is not enough).
set -eu
CFG="${1:-configure}"
[[ -f "${CFG}" ]] || { echo "missing ${CFG}" >&2; exit 1; }
if grep -qF '*ios*)' "${CFG}"; then
  exit 0
fi
perl -i -pe '
  if (/^\s+\*darwin\*\)/ && !$seen++) {
    $_ = "    *ios*)\n        CPPFLAGS=\"-D_DARWIN_C_SOURCE \$CPPFLAGS\"\n        libzmq_pedantic=\"no\"\n\nprintf \"%s\\n\" \"#define ZMQ_HAVE_OSX 1\" >>confdefs.h\n\n        ;;\n" . $_;
  }
' "${CFG}"
# Patching `configure` makes it newer than configure.ac; libzmq Makefile may then rerun aclocal.
touch -r "${CFG}" configure.ac 2>/dev/null || true
[[ -f aclocal.m4 ]] && touch -r "${CFG}" aclocal.m4 2>/dev/null || true
echo "[patch-zeromq-configure-ios] ${CFG}"
