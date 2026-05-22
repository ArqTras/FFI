#!/usr/bin/env bash
# libzmq 4.3.5: autoconf rejects host_os=ios; treat like Darwin for aarch64-apple-ios depends.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
CFG_AC_SRC="${ROOT}/build/ci/patch-zeromq-configure-ac-ios.sh"
CFG_IOS_SRC="${ROOT}/build/ci/patch-zeromq-configure-ios.sh"
DEPENDS_ZMQ="${UP}/contrib/depends/patches/zeromq"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

HOST_HINT="${ARQMA_IOS_DEPENDS_HOST:-${ARQMA_ANDROID_DEPENDS_HOST:-}}"
if [[ "${HOST_HINT}" != *apple-ios* && "${1:-}" != *apple-ios* ]]; then
  echo "[patch-zeromq-ios-host] skip (not an iOS depends host)"
  exit 0
fi

[[ -f "${CFG_AC_SRC}" ]] || { echo "missing ${CFG_AC_SRC}" >&2; exit 1; }
[[ -f "${CFG_IOS_SRC}" ]] || { echo "missing ${CFG_IOS_SRC}" >&2; exit 1; }
mkdir -p "${DEPENDS_ZMQ}"
cp -f "${CFG_AC_SRC}" "${DEPENDS_ZMQ}/patch-zeromq-configure-ac-ios.sh"
cp -f "${CFG_IOS_SRC}" "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
chmod +x "${DEPENDS_ZMQ}/patch-zeromq-configure-ac-ios.sh" "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
for f in "${DEPENDS_ZMQ}"/*.sh; do
  [[ -f "${f}" ]] || continue
  sed -i 's/\r$//' "${f}" 2>/dev/null || sed -i '' 's/\r$//' "${f}" 2>/dev/null || true
done
rm -f "${DEPENDS_ZMQ}/ios-host-os.patch" 2>/dev/null || true

python3 - "$ZMQ_MK" <<'PY'
import pathlib, re, sys

p = pathlib.Path(sys.argv[1])
t = p.read_text()

new_block = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
    "  bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ac-ios.sh && \\\n"
    "  bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ios.sh configure\n"
    "endef"
)

if (
    "bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ac-ios.sh" in t
    and "patch-zeromq-configure-ios.sh configure" in t
    and "ios-host-os.patch" not in t
):
    print("[patch-zeromq-ios-host] zeromq.mk already patched")
    sys.exit(0)

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

p.write_text(t)
print("[patch-zeromq-ios-host] updated", p)
PY

echo "[patch-zeromq-ios-host] ${DEPENDS_ZMQ}"
