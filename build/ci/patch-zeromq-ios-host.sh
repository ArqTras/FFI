#!/usr/bin/env bash
# libzmq 4.3.5: autoconf rejects host_os=ios; treat like Darwin for aarch64-apple-ios depends.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
SCRIPT_SRC="${ROOT}/build/ci/patch-zeromq-configure-ac-ios.sh"
SCRIPT_DEST="${UP}/contrib/depends/patches/zeromq/patch-zeromq-configure-ac-ios.sh"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

HOST_HINT="${ARQMA_IOS_DEPENDS_HOST:-${ARQMA_ANDROID_DEPENDS_HOST:-}}"
if [[ "${HOST_HINT}" != *apple-ios* && "${1:-}" != *apple-ios* ]]; then
  echo "[patch-zeromq-ios-host] skip (not an iOS depends host)"
  exit 0
fi

[[ -f "${SCRIPT_SRC}" ]] || { echo "missing ${SCRIPT_SRC}" >&2; exit 1; }
mkdir -p "$(dirname "${SCRIPT_DEST}")"
cp -f "${SCRIPT_SRC}" "${SCRIPT_DEST}"
chmod +x "${SCRIPT_DEST}"
sed -i 's/\r$//' "${SCRIPT_DEST}" 2>/dev/null || sed -i '' 's/\r$//' "${SCRIPT_DEST}" 2>/dev/null || true

if grep -q 'patch-zeromq-configure-ac-ios.sh' "${ZMQ_MK}" 2>/dev/null; then
  echo "[patch-zeromq-ios-host] already in zeromq.mk"
  exit 0
fi

python3 - "$ZMQ_MK" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
t = p.read_text()
old = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config\n"
    "endef"
)
new = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
    "  bash $($(package)_patch_dir)/patch-zeromq-configure-ac-ios.sh\n"
    "endef"
)
if "patch-zeromq-configure-ac-ios.sh" in t:
    sys.exit(0)
if old not in t:
    old2 = (
        "define $(package)_preprocess_cmds\n"
        "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
        "  patch -p1 < $($(package)_patch_dir)/ios-host-os.patch\n"
        "endef"
    )
    if old2 in t:
        t = t.replace(old2, new, 1)
    else:
        raise SystemExit("zeromq.mk: preprocess_cmds block not found")
else:
    t = t.replace(old, new, 1)
# Drop broken unified-diff patch entry if present.
t = t.replace("$(package)_patches=ios-host-os.patch\n", "")
p.write_text(t)
print("[patch-zeromq-ios-host] updated", p)
PY

echo "[patch-zeromq-ios-host] ${SCRIPT_DEST}"
