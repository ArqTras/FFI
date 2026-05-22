#!/usr/bin/env bash
# Run inside extracted libzmq source (contrib/depends preprocess). Treat host_os=ios like Darwin.
set -eu
CFG=configure.ac
if grep -q '*ios*' "${CFG}" 2>/dev/null; then
  exit 0
fi
python3 - "${CFG}" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
t = p.read_text()
needle = "    *darwin*)"
block = """    *ios*)
        CPPFLAGS="-D_DARWIN_C_SOURCE $CPPFLAGS"
        libzmq_pedantic="no"
        AC_DEFINE(ZMQ_HAVE_OSX, 1, [Have DarwinOSX OS])
        ;;
"""
if needle not in t:
    raise SystemExit(f"{p}: *darwin* block not found")
p.write_text(t.replace(needle, block + needle, 1))
print(f"patched {p} for ios host_os")
PY
