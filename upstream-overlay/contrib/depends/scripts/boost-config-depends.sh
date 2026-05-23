#!/bin/sh
# Configure step for contrib/depends boost package (must be a simple command after config_env).
set -e
host_os="$1"
build_prefix="$2"
libraries="$3"
if [ "$host_os" = ios ]; then
  cp "${build_prefix}/bin/b2" ./b2
  chmod +x ./b2
  # bootstrap.sh normally writes project-config.jam with --with-libraries; without it b2 builds everything.
  libs=$(printf '%s' "$libraries" | tr -d '"' | tr ',' ' ')
  # Toolset lives only in user-config.jam (preprocess); do not duplicate "using clang" here.
  {
    printf '%s\n' 'project : default-build <variant>release ;'
    for lib in $libs; do
      printf 'option.set with-%s : true ;\n' "$lib"
    done
  } > project-config.jam
else
  ./bootstrap.sh --without-icu --with-libraries="$libraries"
fi
