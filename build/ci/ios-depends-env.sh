#!/usr/bin/env bash
# Environment for contrib/depends when HOST is aarch64-apple-ios.
# Do not export SDKROOT/CC/CXX globally — depends sets host tools via ios.mk;
# native_b2 uses contrib/depends/scripts/native-b2-build.sh with macosx SDK.
set -euo pipefail
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:${HOME}/.cargo/bin"
unset SDKROOT CC CXX CFLAGS CXXFLAGS LDFLAGS CPPFLAGS
