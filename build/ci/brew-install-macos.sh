#!/usr/bin/env bash
# Install macOS native dependencies from Brewfile (fail on missing formulae; no silent brew errors).
set -euxo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
brew update
brew bundle install --file="$ROOT/build/ci/Brewfile"
