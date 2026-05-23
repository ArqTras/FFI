#!/bin/sh
# Build the Boost.Build engine (b2) with the *macOS build* toolchain only.
# Must not inherit iOS cross vars (SDKROOT/CC/IPHONEOS_DEPLOYMENT_TARGET) from CI.
set -e
build_cc="$1"
build_cxx="$2"
extra_path="${3:-}"
unset CC CXX CFLAGS CXXFLAGS LDFLAGS CPPFLAGS IPHONEOS_DEPLOYMENT_TARGET
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export SDKROOT
export CC="$build_cc"
export CXX="$build_cxx"
export PATH="/usr/bin:/bin:/usr/sbin:/sbin${extra_path:+:${extra_path}}"
exec ./build.sh clang
