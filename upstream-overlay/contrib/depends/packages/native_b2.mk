# Native b2 for cross builds (iOS): build the Boost.Build engine with the *build* toolchain.
boost_version=1.87.0
boost_download_path=https://archives.boost.io/release/$(boost_version)/source/
boost_file_name=boost_1_87_0.tar.bz2
boost_sha256_hash=af57be25cb4c4f4b413ed692fe378affb4352ea50fbe294a11ef548f4d527d89

package=native_b2
$(package)_version=$(boost_version)
$(package)_download_path=$(boost_download_path)
$(package)_file_name=$(boost_file_name)
$(package)_sha256_hash=$(boost_sha256_hash)
$(package)_build_subdir=tools/build/src/engine
# Build b2 with macOS SDK only (see scripts/native-b2-build.sh).
define $(package)_build_cmds
  sh $(BASEDIR)/scripts/native-b2-build.sh "$(build_CC)" "$(build_CXX)" "$(build_prefix)/bin"
endef

define $(package)_stage_cmds
  mkdir -p "$($(package)_staging_prefix_dir)"/bin/ && \
  cp b2 "$($(package)_staging_prefix_dir)"/bin/
endef
