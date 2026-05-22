#!/usr/bin/env bash
# libzmq 4.3.5: autoconf rejects host_os=ios; treat like Darwin for aarch64-apple-ios depends.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
PATCH_SRC="${ROOT}/build/ci/patches/zeromq-ios-host-os.patch"
PATCH_NAME="ios-host-os.patch"
DEPENDS_PATCH="${UP}/contrib/depends/patches/zeromq/${PATCH_NAME}"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

HOST_HINT="${ARQMA_IOS_DEPENDS_HOST:-${ARQMA_ANDROID_DEPENDS_HOST:-}}"
if [[ "${HOST_HINT}" != *apple-ios* && "${1:-}" != *apple-ios* ]]; then
  echo "[patch-zeromq-ios-host] skip (not an iOS depends host)"
  exit 0
fi

[[ -f "${PATCH_SRC}" ]] || { echo "missing ${PATCH_SRC}" >&2; exit 1; }
mkdir -p "$(dirname "${DEPENDS_PATCH}")"
cp -f "${PATCH_SRC}" "${DEPENDS_PATCH}"

if grep -q 'ios-host-os.patch' "${ZMQ_MK}" 2>/dev/null; then
  echo "[patch-zeromq-ios-host] already in zeromq.mk"
  exit 0
fi

python3 - "$ZMQ_MK" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
t = p.read_text()
marker = "$(package)_sha256_hash="
idx = t.find(marker)
if idx < 0:
    raise SystemExit("zeromq.mk: sha256_hash line not found")
end = t.find("\n", idx)
t = t[: end + 1] + "$(package)_patches=ios-host-os.patch\n" + t[end + 1 :]
old = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config\n"
    "endef"
)
new = (
    "define $(package)_preprocess_cmds\n"
    "  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n"
    "  patch -p1 < $($(package)_patch_dir)/ios-host-os.patch\n"
    "endef"
)
if old not in t:
    raise SystemExit("zeromq.mk: preprocess_cmds block not found")
p.write_text(t.replace(old, new, 1))
print("[patch-zeromq-ios-host] updated", p)
PY

echo "[patch-zeromq-ios-host] ${DEPENDS_PATCH}"
