package=boost
$(package)_version=1.87.0
$(package)_download_path=https://archives.boost.io/release/$($(package)_version)/source/
$(package)_file_name=boost_$(subst .,_,$($(package)_version)).tar.bz2
$(package)_sha256_hash=af57be25cb4c4f4b413ed692fe378affb4352ea50fbe294a11ef548f4d527d89

ifeq ($(host_os),ios)
$(package)_dependencies=native_b2
# Cross-build libs with iPhoneOS SDK (b2 engine is macOS-native via native_b2).
$(package)_build_env_ios=SDKROOT=$(IOS_SDK) IPHONEOS_DEPLOYMENT_TARGET=$(IOS_MIN_VERSION)
$(package)_stage_env_ios=SDKROOT=$(IOS_SDK) IPHONEOS_DEPLOYMENT_TARGET=$(IOS_MIN_VERSION)
# bootstrap.sh (--with-libraries) is skipped on iOS; b2 must get an explicit --with-* list.
boost_ios_lib_list := chrono filesystem program_options system thread date_time regex serialization atomic
boost_b2_with_flags := $(foreach lib,$(boost_ios_lib_list),--with-$(lib) )
endif

define $(package)_set_vars
$(package)_config_opts_release=variant=release
$(package)_config_opts_debug=variant=debug
# Omit --build-type=complete (OOM on CI; only listed libraries are needed).
$(package)_config_opts+=--layout=tagged --user-config=user-config.jam
$(package)_config_opts+=threading=multi link=static -sNO_BZIP2=1 -sNO_ZLIB=1
$(package)_config_opts_linux=target-os=linux threadapi=pthread runtime-link=static
$(package)_config_opts_android=threadapi=pthread runtime-link=static target-os=android
$(package)_config_opts_darwin=--toolset=darwin runtime-link=static target-os=darwin
$(package)_config_opts_ios=target-os=iphone runtime-link=static threading=multi architecture=arm address-model=64 $(boost_b2_with_flags)
$(package)_config_opts_mingw32=binary-format=pe target-os=windows threadapi=win32 runtime-link=static
$(package)_config_opts_x86_64=architecture=x86 address-model=64
$(package)_config_opts_aarch64=address-model=64
ifneq (,$(findstring clang,$($(package)_cxx)))
$(package)_toolset_$(host_os)=clang
else
$(package)_toolset_$(host_os)=gcc
endif
$(package)_archiver_$(host_os)=$($(package)_ar)
$(package)_toolset_darwin=darwin
$(package)_archiver_darwin=$($(package)_libtool)
$(package)_config_libraries_$(host_os)="chrono,filesystem,program_options,system,thread,test,date_time,regex,serialization"
$(package)_config_libraries_ios="chrono,filesystem,program_options,system,thread,date_time,regex,serialization,atomic"
$(package)_config_libraries_mingw32="chrono,filesystem,program_options,system,thread,test,date_time,regex,serialization,locale"
$(package)_cxxflags=-std=c++17
$(package)_cxxflags_linux+=-fPIC
$(package)_cxxflags_darwin+=-ffile-prefix-map=$($(package)_extract_dir)=/usr
endef

define $(package)_preprocess_cmds
  echo "using $(boost_toolset_$(host_os)) : : $($(package)_cxx) : <cxxflags>\"$($(package)_cxxflags) $($(package)_cppflags)\" <linkflags>\"$($(package)_ldflags)\" <archiver>\"$(boost_archiver_$(host_os))\" <arflags>\"$($(package)_arflags)\" <striper>\"$(host_STRIP)\"  <ranlib>\"$(host_RANLIB)\" <rc>\"$(host_WINDRES)\" : ;" > user-config.jam
endef

define $(package)_config_cmds
  sh $(BASEDIR)/scripts/boost-config-depends.sh "$(host_os)" "$(build_prefix)" $(boost_config_libraries_$(host_os))
endef

boost_build_jobs=$(if $(filter ios,$(host_os)),1,2)
boost_stage_jobs=$(if $(filter ios,$(host_os)),1,4)

define $(package)_build_cmds
  ./b2 -d1 -j$(boost_build_jobs) --prefix=$($(package)_staging_prefix_dir) $($(package)_config_opts) stage
endef

define $(package)_stage_cmds
  ./b2 -d0 -j$(boost_stage_jobs) --prefix=$($(package)_staging_prefix_dir) $($(package)_config_opts) install
endef
