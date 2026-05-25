#!/usr/bin/env bash
# Build `arqma_flutter_solo_pool` for Flutter desktop bundles (pure Rust — no wallet_merged / wallet2 FFI).
#
# Usage (from repo root):
#   bash rust/tool/build_flutter_solo_pool.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

CARGO_ARGS=(build -p arqma-flutter-solo-pool --release --bin arqma_flutter_solo_pool)
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-thin}"
    CARGO_ARGS+=(--target x86_64-pc-windows-gnu)
    ;;
esac

cargo "${CARGO_ARGS[@]}"

install_one() {
  local src="$1"
  if [[ ! -f "${src}" ]]; then
    return 1
  fi
  echo "Built $(basename "${src}") <- ${src}"
  return 0
}

for cand in \
  "${ROOT}/target/release/arqma_flutter_solo_pool" \
  "${ROOT}/target/x86_64-pc-windows-gnu/release/arqma_flutter_solo_pool.exe"; do
  if install_one "${cand}"; then
    exit 0
  fi
done

echo "::error::arqma_flutter_solo_pool not found under rust/target after build" >&2
exit 1
