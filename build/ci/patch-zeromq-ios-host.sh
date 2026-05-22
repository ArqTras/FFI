#!/usr/bin/env bash
# libzmq 4.3.5 / aarch64-apple-ios: pre-generated `configure` must accept host_os=ios.
#
# Mobile branch: ios-host-os.patch edits configure.ac (local depends build). That makes
# libzmq's Makefile rerun aclocal/automake — fine on a dev Mac with autotools installed.
# Mobile release CI sets ARQMA_SKIP_IOS_DEPENDS=1 and never builds depends zeromq.
#
# FFI release CI builds full depends on macOS runners without automake — patch only
# `configure` (see patch-zeromq-configure-ios.sh), not configure.ac.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
CFG_IOS_SRC="${ROOT}/build/ci/patch-zeromq-configure-ios.sh"
DEPENDS_ZMQ="${UP}/contrib/depends/patches/zeromq"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

HOST_HINT="${ARQMA_IOS_DEPENDS_HOST:-${ARQMA_ANDROID_DEPENDS_HOST:-}}"
if [[ "${HOST_HINT}" != *apple-ios* && "${1:-}" != *apple-ios* ]]; then
  echo "[patch-zeromq-ios-host] skip (not an iOS depends host)"
  exit 0
fi

[[ -f "${CFG_IOS_SRC}" ]] || { echo "missing ${CFG_IOS_SRC}" >&2; exit 1; }
mkdir -p "${DEPENDS_ZMQ}"
cp -f "${CFG_IOS_SRC}" "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
chmod +x "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
sed -i 's/\r$//' "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh" 2>/dev/null \
  || sed -i '' 's/\r$//' "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh" 2>/dev/null || true
rm -f "${DEPENDS_ZMQ}/patch-zeromq-configure-ac-ios.sh" "${DEPENDS_ZMQ}/ios-host-os.patch" 2>/dev/null || true

python3 - "$ZMQ_MK" <<'PY'
import pathlib, re, sys

p = pathlib.Path(sys.argv[1])
t = p.read_text()

new_block = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
    "  bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ios.sh configure\n"
    "endef"
)

needs_preprocess_fix = (
    "patch-zeromq-configure-ac-ios.sh" in t
    or "ios-host-os.patch" in t
    or "bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ios.sh configure" not in t
)
needs_build_fix = "MAINTAINER_MODE=0" not in t
if not needs_preprocess_fix and not needs_build_fix:
    print("[patch-zeromq-ios-host] zeromq.mk already patched")
    sys.exit(0)

if needs_preprocess_fix:
    t, n = re.subn(
        r"define \$\(package\)_preprocess_cmds\n.*?\nendef",
        new_block,
        t,
        count=1,
        flags=re.DOTALL,
    )
    if n != 1:
        raise SystemExit("zeromq.mk: preprocess_cmds block not found")
    t = re.sub(r"\$\(package\)_patches=ios-host-os\.patch\n", "", t)

if needs_build_fix:
    t, n = re.subn(
        r"define \$\(package\)_build_cmds\n  \$\(MAKE\) (\$\(\(package\)_build_opts\) )?src/libzmq\.la\nendef",
        "define $(package)_build_cmds\n"
        "  $(MAKE) MAINTAINER_MODE=0 $($(package)_build_opts) src/libzmq.la\n"
        "endef",
        t,
        count=1,
    )
    if n != 1:
        raise SystemExit("zeromq.mk: build_cmds block not found")

p.write_text(t)
print("[patch-zeromq-ios-host] updated", p)
PY

echo "[patch-zeromq-ios-host] ${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
