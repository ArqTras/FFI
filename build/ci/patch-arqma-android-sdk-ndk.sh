#!/usr/bin/env bash
# Use Android SDK NDK (r21+) instead of contrib/depends android-ndk-r17b (clang segfault on Ubuntu 24).
set -euo pipefail
UPSTREAM="${1:?upstream dir}"
HOSTS_MK="${UPSTREAM}/contrib/depends/hosts/android.mk"
PACKAGES_MK="${UPSTREAM}/contrib/depends/packages/packages.mk"
_patch_android_hosts() {
if grep -q '_android_ndk_exe' "${HOSTS_MK}" 2>/dev/null; then
  return 0
fi

cat > "${HOSTS_MK}" <<'EOF'
ANDROID_API=21

# Patched: ARQMA_USE_SDK_ANDROID_NDK — LLVM from Android SDK (set ANDROID_NDK_HOME).
ifneq ($(ARQMA_USE_SDK_ANDROID_NDK),)
_android_ndk_prebuilt := $(firstword $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/linux-x86_64) $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/*))
_android_ndk_exe :=
ifeq ($(findstring windows,$(_android_ndk_prebuilt)),windows)
_android_ndk_exe := .exe
endif
ifeq ($(host_arch),aarch64)
android_CC := $(_android_ndk_prebuilt)/bin/aarch64-linux-android21-clang
android_CXX := $(_android_ndk_prebuilt)/bin/aarch64-linux-android21-clang++
endif
ifeq ($(host_arch),arm)
android_CC := $(_android_ndk_prebuilt)/bin/armv7a-linux-androideabi21-clang
android_CXX := $(_android_ndk_prebuilt)/bin/armv7a-linux-androideabi21-clang++
endif
ifeq ($(host_arch),x86_64)
android_CC := $(_android_ndk_prebuilt)/bin/x86_64-linux-android21-clang
android_CXX := $(_android_ndk_prebuilt)/bin/x86_64-linux-android21-clang++
endif
android_AR := $(_android_ndk_prebuilt)/bin/llvm-ar$(_android_ndk_exe)
android_STRIP := $(_android_ndk_prebuilt)/bin/llvm-strip$(_android_ndk_exe)
android_NM := $(_android_ndk_prebuilt)/bin/llvm-nm$(_android_ndk_exe)
android_RANLIB := :
else
ifeq ($(host_arch),arm)
host_toolchain=arm-linux-androideabi-
endif
android_CC=$(host_toolchain)clang
android_CXX=$(host_toolchain)clang++
android_RANLIB=:
endif

android_CFLAGS=-pipe
android_CXXFLAGS=$(android_CFLAGS)
android_ARFLAGS=crsD

android_release_CFLAGS=-O2
android_release_CXXFLAGS=$(android_release_CFLAGS)

android_debug_CFLAGS=-g -O0
android_debug_CXXFLAGS=$(android_debug_CFLAGS)

ifneq ($(ARQMA_USE_SDK_ANDROID_NDK),)
android_native_toolchain=
else
android_native_toolchain=android_ndk
endif

android_cmake_system=Android
EOF
}

_patch_openssl_ndk() {
  local openssl_mk="${UPSTREAM}/contrib/depends/packages/openssl.mk"
  [[ -f "${openssl_mk}" ]] || return 0
  if grep -q 'ARQMA_OPENSSL_BUILD_CMDS' "${openssl_mk}" 2>/dev/null; then
    return 0
  fi
  sed -i \
    's|ANDROID_NDK_ROOT="$(host_prefix)/native"|ANDROID_NDK_ROOT="$(ANDROID_NDK_HOME)"|g' \
    "${openssl_mk}"
  sed -i \
    's|PATH="$(host_prefix)/native/bin"|PATH="$(firstword $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/linux-x86_64) $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/*))/bin"|g' \
    "${openssl_mk}"
  if ! grep -q 'CC=clang CXX=clang++' "${openssl_mk}"; then
    sed -i \
      's|^\$(package)_config_env_android=ANDROID_NDK_ROOT|\$(package)_config_env_android=CC=clang CXX=clang++ ANDROID_API=21 ANDROID_NDK_ROOT|' \
      "${openssl_mk}"
  fi
  sed -i '9a# ARQMA_SDK_OPENSSL_NDK' "${openssl_mk}" 2>/dev/null || true
  if ! grep -q 'ARQMA_OPENSSL_CONFIG_CMDS' "${openssl_mk}" 2>/dev/null; then
    cat >>"${openssl_mk}" <<'EOF'

# ARQMA_OPENSSL_CONFIG_CMDS / ARQMA_OPENSSL_BUILD_CMDS — keep NDK wrappers on PATH for Configure + make.
ifneq ($(ARQMA_USE_SDK_ANDROID_NDK),)
_android_openssl_ndk_bin := $(firstword $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/linux-x86_64) $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/*))/bin
define $(package)_config_cmds
  PATH="$(_android_openssl_ndk_bin):$$PATH:/usr/bin:/bin" CC="$$($(package)_cc)" CXX="$$($(package)_cxx)" ./Configure $($(package)_config_opts) ARFLAGS=$($(package)_arflags)
endef
define $(package)_build_cmds
  PATH="$(_android_openssl_ndk_bin):$$PATH:/usr/bin:/bin" $(MAKE) build_libs
endef
define $(package)_stage_cmds
  PATH="$(_android_openssl_ndk_bin):$$PATH:/usr/bin:/bin" $(MAKE) DESTDIR=$($(package)_staging_dir) install_sw
endef
endif
EOF
  fi
}

_patch_android_hosts
if grep -q '^android_native_packages := android_ndk' "${PACKAGES_MK}"; then
  sed -i 's/^android_native_packages := android_ndk/android_native_packages :=/' "${PACKAGES_MK}"
fi
_patch_openssl_ndk

echo "patched Android depends to use SDK NDK when ARQMA_USE_SDK_ANDROID_NDK=1"
