#!/usr/bin/env bash
# Fast CRLF + known upstream patches for contrib/depends (do not scan entire tree).
set -euo pipefail
UPSTREAM="${1:?upstream dir}"
DEPENDS="${UPSTREAM}/contrib/depends"
[[ -d "${DEPENDS}" ]] || { echo "Missing ${DEPENDS}" >&2; exit 1; }
FUNCS="${DEPENDS}/funcs.mk"

# Only metadata dirs — never work/built/sources (boost extract races with find+sed).
_depends_crlf_dirs=("${DEPENDS}/packages" "${DEPENDS}/hosts" "${DEPENDS}/patches")
for _d in "${_depends_crlf_dirs[@]}"; do
  [[ -d "${_d}" ]] || continue
  find "${_d}" -type f \( \
    -name '*.mk' -o -name '*.sh' -o -name 'Makefile' -o -name 'config.guess' -o -name 'config.sub' \
    -o -name '*.in' \) -print0 | xargs -0r sed -i 's/\r$//' 2>/dev/null || true
done
find "${DEPENDS}" -maxdepth 1 -type f \( -name '*.mk' -o -name '*.sh' -o -name 'Makefile' \) \
  -print0 | xargs -0r sed -i 's/\r$//' 2>/dev/null || true
sed -i 's/\r$//' "${FUNCS}" "${DEPENDS}/config.guess" "${DEPENDS}/config.sub" 2>/dev/null || true
chmod +x "${DEPENDS}/config.guess" "${DEPENDS}/config.sub" 2>/dev/null || true

sed -i -E 's/^define fetch_file_inne+$/define fetch_file_inner/' "${FUNCS}"
ZMQ_MK="${DEPENDS}/packages/zeromq.mk"
[[ -f "${ZMQ_MK}" ]] && sed -i 's/--disable-Werro$/--disable-Werror/' "${ZMQ_MK}" 2>/dev/null || true
[[ -f "${ZMQ_MK}" ]] && sed -i 's/--disable-Werro /--disable-Werror /' "${ZMQ_MK}" 2>/dev/null || true
NDK_MK="${DEPENDS}/packages/android_ndk.mk"
BOOST_MK="${DEPENDS}/packages/boost.mk"
if [[ -f "${BOOST_MK}" ]] && ! grep -q 'arqma_android_build_jobs' "${BOOST_MK}"; then
  sed -i 's|./b2 -d2 -j2 -d1|./b2 -d2 -j1 -d1|' "${BOOST_MK}"
  sed -i 's|./b2 -d0 -j4|./b2 -d0 -j1|' "${BOOST_MK}"
fi

if [[ -f "${NDK_MK}" ]]; then
  sed -i 's/unzip -q \$(/unzip -o -q $(/' "${NDK_MK}" 2>/dev/null || true
  sed -i 's|^  android-ndk-r|  python3 android-ndk-r|' "${NDK_MK}" 2>/dev/null || true
  grep -q 'python3 android-ndk' "${NDK_MK}" || \
    sed -i 's|  android-ndk-r|  python3 android-ndk-r|' "${NDK_MK}"
fi
