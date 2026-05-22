#!/usr/bin/env bash
# Packages commonly required to build Arqma contrib/depends on Debian/Ubuntu (GitHub Actions ubuntu-latest).
# Safe to run repeatedly; no-ops if apt-get is missing.
set -eu
if ! command -v apt-get >/dev/null 2>&1; then
  echo "[install-arqma-depends-linux] no apt-get — skipping"
  exit 0
fi
if [[ "$(id -u)" -eq 0 ]]; then
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libtool \
    automake \
    autoconf \
    bison \
    flex \
    gperf \
    python3 \
    ca-certificates \
    curl \
    git \
    cmake \
    ninja-build \
    ccache \
    patch \
    zip \
    xz-utils
else
  sudo apt-get update -qq
  DEBIAN_FRONTEND=noninteractive sudo apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libtool \
    automake \
    autoconf \
    bison \
    flex \
    gperf \
    python3 \
    ca-certificates \
    curl \
    git \
    cmake \
    ninja-build \
    ccache \
    patch \
    zip \
    xz-utils
fi

echo "[install-arqma-depends-linux] OK"
