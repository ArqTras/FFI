#!/usr/bin/env bash
# libzmq 4.3.5: autoconf rejects host_os=ios; treat like Darwin for aarch64-apple-ios depends.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
UP="${1:-${ARQMA_WALLET2_UPSTREAM_DIR:-${ROOT}/rust/arqma-rpc-upstream}}"
PATCH_SRC="${ROOT}/build/ci/patches/zeromq-ios-host-os.patch"
PATCH_NAME="ios-host-os.patch"
DEPENDS_PATCH="${UP}/contrib/depends/patches/zeromq/${PATCH_NAME}"
ZMQ_MK="${UP}/contrib/depends/packages/zeromq.mk"

[[ -f "${PATCH_SRC}" ]] || { echo "missing ${PATCH_SRC}" >&2; exit 1; }
mkdir -p "$(dirname "${DEPENDS_PATCH}")"
cp -f "${PATCH_SRC}" "${DEPENDS_PATCH}"

if ! grep -q 'ios-host-os.patch' "${ZMQ_MK}"; then
  perl -i -pe 's|^(\$\(package\)_sha256_hash=.*)$/$1\n$(package)_patches=ios-host-os.patch/' "${ZMQ_MK}"
  perl -0777 -i -pe 's|define \$\(package\)_preprocess_cmds\n  cp -f \$\(BASEDIR\)/config.guess \$\(BASEDIR\)/config.sub config\nendef|define $(package)_preprocess_cmds\n  cp -f $(BASEDIR)/config.guess $(BASEDIR)/config.sub config && \\\n  patch -p1 < $$($(package)_patch_dir)/ios-host-os.patch\nendef|' "${ZMQ_MK}"
fi

echo "[patch-zeromq-ios-host] ${DEPENDS_PATCH}"
