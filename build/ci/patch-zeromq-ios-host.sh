#!/usr/bin/env bash
# libzmq 4.3.5: autoconf rejects host_os=ios; treat like Darwin for aarch64-apple-ios depends.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
PATCH_SRC="${ROOT}/build/ci/patches/zeromq-ios-host-os.patch"
CFG_IOS_SRC="${ROOT}/build/ci/patch-zeromq-configure-ios.sh"
DEPENDS_ZMQ="${UP}/contrib/depends/patches/zeromq"
PATCH_NAME="ios-host-os.patch"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

HOST_HINT="${ARQMA_IOS_DEPENDS_HOST:-${ARQMA_ANDROID_DEPENDS_HOST:-}}"
if [[ "${HOST_HINT}" != *apple-ios* && "${1:-}" != *apple-ios* ]]; then
  echo "[patch-zeromq-ios-host] skip (not an iOS depends host)"
  exit 0
fi

[[ -f "${PATCH_SRC}" ]] || { echo "missing ${PATCH_SRC}" >&2; exit 1; }
[[ -f "${CFG_IOS_SRC}" ]] || { echo "missing ${CFG_IOS_SRC}" >&2; exit 1; }
mkdir -p "${DEPENDS_ZMQ}"
cp -f "${PATCH_SRC}" "${DEPENDS_ZMQ}/${PATCH_NAME}"
cp -f "${CFG_IOS_SRC}" "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
chmod +x "${DEPENDS_ZMQ}/patch-zeromq-configure-ios.sh"
for f in "${DEPENDS_ZMQ}"/*.sh "${DEPENDS_ZMQ}/${PATCH_NAME}"; do
  [[ -f "${f}" ]] || continue
  sed -i 's/\r$//' "${f}" 2>/dev/null || sed -i '' 's/\r$//' "${f}" 2>/dev/null || true
done
rm -f "${DEPENDS_ZMQ}/patch-zeromq-configure-ac-ios.sh" 2>/dev/null || true

python3 - "$ZMQ_MK" <<'PY'
import pathlib, re, sys

p = pathlib.Path(sys.argv[1])
t = p.read_text()

new_block = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
    "  patch -p1 < $($(package)_patch_dir)/ios-host-os.patch && \\\n"
    "  bash $(BASEDIR)/patches/zeromq/patch-zeromq-configure-ios.sh configure\n"
    "endef"
)

if "patch-zeromq-configure-ios.sh configure" in t and "ios-host-os.patch" in t:
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

if "$(package)_patches=ios-host-os.patch" not in t:
    t = t.replace(
        "$(package)_sha256_hash=6653ef5910f17954861fe72332e68b03ca6e4d9c7160eb3a8de5a5a913bfab43\n",
        "$(package)_sha256_hash=6653ef5910f17954861fe72332e68b03ca6e4d9c7160eb3a8de5a5a913bfab43\n"
        "$(package)_patches=ios-host-os.patch\n",
        1,
    )

p.write_text(t)
print("[patch-zeromq-ios-host] updated", p)
PY

echo "[patch-zeromq-ios-host] ${DEPENDS_ZMQ}/${PATCH_NAME}"
