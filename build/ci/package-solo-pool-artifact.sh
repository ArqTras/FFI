#!/usr/bin/env bash
# Zip arqma_flutter_solo_pool for GitHub Release (desktop platforms only).
# Layout matches wallet FFI zips: {platform}/<binary> + solo-pool-manifest.json
set -euo pipefail
PLATFORM="${1:?platform e.g. linux-x86_64}"
VER="${2:?version e.g. 1.0.0}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="${ROOT}/dist/.solo-pool-stage/${PLATFORM}"
mkdir -p "${STAGE}"
shopt -s nullglob
case "${PLATFORM}" in
  linux-x86_64)
    BIN="${ROOT}/rust/target/release/arqma_flutter_solo_pool"
    BIN_NAME="arqma_flutter_solo_pool"
    ;;
  macos-arm64)
    BIN="${ROOT}/rust/target/release/arqma_flutter_solo_pool"
    BIN_NAME="arqma_flutter_solo_pool"
    ;;
  windows-x86_64-gnu)
    BIN="${ROOT}/rust/target/x86_64-pc-windows-gnu/release/arqma_flutter_solo_pool.exe"
    BIN_NAME="arqma_flutter_solo_pool.exe"
    ;;
  *)
    echo "unknown solo-pool platform: ${PLATFORM}" >&2
    exit 2
    ;;
esac
if [[ ! -f "${BIN}" ]]; then
  echo "solo pool binary not found: ${BIN}" >&2
  exit 1
fi
cp "${BIN}" "${STAGE}/${BIN_NAME}"
chmod +x "${STAGE}/${BIN_NAME}" 2>/dev/null || true
cat > "${STAGE}/solo-pool-manifest.json" <<EOF
{
  "product": "arqma-wallet-solo-pool",
  "version": "${VER}",
  "platform": "${PLATFORM}",
  "binary": "${BIN_NAME}"
}
EOF
OUT="${ROOT}/dist/arqma-wallet-solo-pool-${PLATFORM}-${VER}.zip"
rm -f "${OUT}"
(
  cd "${ROOT}/dist/.solo-pool-stage"
  zip -qr "${OUT}" "${PLATFORM}"
)
echo "artifact=${OUT}"
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "artifact=${OUT}" >> "${GITHUB_OUTPUT}"
fi
ls -la "${OUT}"
