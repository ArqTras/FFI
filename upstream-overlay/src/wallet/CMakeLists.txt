# Copyright (c) 2018 - 2026, The Arqma Network
# Copyright (c) 2014-2020, The Monero Project
#
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without modification, are
# permitted provided that the following conditions are met:
#
# 1. Redistributions of source code must retain the above copyright notice, this list of
#    conditions and the following disclaimer.
#
# 2. Redistributions in binary form must reproduce the above copyright notice, this list
#    of conditions and the following disclaimer in the documentation and/or othe
#    materials provided with the distribution.
#
# 3. Neither the name of the copyright holder nor the names of its contributors may be
#    used to endorse or promote products derived from this software without specific
#    prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
# EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
# MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
# THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
# SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
# PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
# INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
# STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
# THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


set(wallet_sources
  wallet2.cpp
  wallet_args.cpp
  ringdb.cpp
  node_rpc_proxy.cpp)

arqma_find_all_headers(wallet_private_headers "${CMAKE_CURRENT_SOURCE_DIR}")

arqma_private_headers(wallet
  ${wallet_private_headers})
arqma_add_library(wallet
  ${wallet_sources}
  ${wallet_private_headers})
target_link_libraries(wallet
  PUBLIC
    rpc_base
    multisig
    common
    cryptonote_core
    mnemonics
    net
    lmdb
    ${Boost_SERIALIZATION_LIBRARY}
    ${Boost_FILESYSTEM_LIBRARY}
    ${Boost_SYSTEM_LIBRARY}
    ${Boost_THREAD_LIBRARY}
    ${Boost_REGEX_LIBRARY}
  PRIVATE
    ${EXTRA_LIBRARIES})

if(NOT IOS)
  set(wallet_rpc_sources
    wallet_rpc_server.cpp)

  set(wallet_rpc_headers)

  set(wallet_rpc_private_headers
    wallet_rpc_server.h)

  arqma_private_headers(wallet_rpc_server
    ${wallet_rpc_private_headers})
  arqma_add_executable(wallet_rpc_server
    ${wallet_rpc_sources}
    ${wallet_rpc_headers}
    ${wallet_rpc_private_headers})

  target_link_libraries(wallet_rpc_server
    PRIVATE
      wallet
      rpc_base
      # core_rpc_server.cpp lives in `rpc`; wallet_rpc_server links its handlers.
      rpc
      cryptonote_core
      cncrypto
      common
      version
      daemonizer
      epee
      ${Boost_PROGRAM_OPTIONS_LIBRARY}
      ${Boost_FILESYSTEM_LIBRARY}
      ${Boost_THREAD_LIBRARY}
      ${CMAKE_THREAD_LIBS_INIT}
      ${EXTRA_LIBRARIES})
  set_property(TARGET wallet_rpc_server
    PROPERTY
      OUTPUT_NAME "arqma-wallet-rpc")
  install(TARGETS wallet_rpc_server DESTINATION bin)
endif()


# build and install libwallet_merged only if we building for GUI
if(BUILD_GUI_DEPS)
  if(NOT IOS)
    set(libs_to_merge
            wallet_api
            wallet
            multisig
            blockchain_db
            cryptonote_core
            cryptonote_basic
            cryptonote_format_utils_basic
            cryptonote_protocol
            mnemonics
            common
            cncrypto
            device
            ringct
            ringct_basic
            checkpoints
            version
            net
            rpc
            rpc_base
            arqnet
            serialization
            p2p
            lmdb_lib)
    # Arqma-GUI-MM: wallet_merged MinGW daemonizer
    # MinGW GUI link pulls `windows::check_admin` from daemonizer via cryptonote_core.
    if(MINGW)
      list(APPEND libs_to_merge daemonizer)
    endif()

  else()
    set(libs_to_merge
            wallet_api
            wallet
            multisig
            blockchain_db
            cryptonote_core
            cryptonote_basic
            cryptonote_format_utils_basic
            mnemonics
            common
            cncrypto
            device
            ringct
            ringct_basic
            checkpoints
            version
            net
            lmdb_lib)
  endif()

    foreach(lib ${libs_to_merge})
        list(APPEND objlibs $<TARGET_OBJECTS:obj_${lib}>) # matches naming convention in src/CMakeLists.txt
    endforeach()
    add_library(wallet_merged STATIC ${objlibs})
    # GUI Rust/Tauri link **only** `wallet_merged`; fold epee + easylogging + randomx into this archive
    # on every desktop OS (macOS libtool, Windows MSVC `Lib`/llvm-lib, Windows MinGW + Linux GNU `ar`).
    if(NOT IOS AND TARGET epee AND TARGET easylogging AND TARGET randomx)
      add_dependencies(wallet_merged epee easylogging randomx)
      if(APPLE)
        add_custom_command(TARGET wallet_merged POST_BUILD
          COMMAND libtool -static -o $<TARGET_FILE:wallet_merged>.fat
                  $<TARGET_FILE:wallet_merged>
                  $<TARGET_FILE:epee>
                  $<TARGET_FILE:easylogging>
                  $<TARGET_FILE:randomx>
          COMMAND ${CMAKE_COMMAND} -E rename $<TARGET_FILE:wallet_merged>.fat $<TARGET_FILE:wallet_merged>
          COMMENT "Fold epee/easylogging/randomx into libwallet_merged (fat static archive)"
          VERBATIM)
      elseif(WIN32 AND NOT MINGW)
        # MSVC / Clang-cl / Ninja: use toolchain archiver (Lib.exe or llvm-lib), not bare `lib` on PATH.
        add_custom_command(TARGET wallet_merged POST_BUILD
          COMMAND "${CMAKE_AR}" /OUT:$<TARGET_FILE:wallet_merged>.fat
                  $<TARGET_FILE:wallet_merged>
                  $<TARGET_FILE:epee>
                  $<TARGET_FILE:easylogging>
                  $<TARGET_FILE:randomx>
          COMMAND ${CMAKE_COMMAND} -E rename $<TARGET_FILE:wallet_merged>.fat $<TARGET_FILE:wallet_merged>
          COMMENT "Fold epee/easylogging/randomx into wallet_merged.lib (Windows COFF)"
          VERBATIM)
      elseif(MINGW)
        # MinGW: GNU `ar -M` MRI merges COFF `.a` archives (python extract misses `.obj` members).
        add_custom_command(TARGET wallet_merged POST_BUILD
          COMMAND ${CMAKE_COMMAND}
            -DCMAKE_AR=${CMAKE_AR}
            -DWALLET_MERGED=$<TARGET_FILE:wallet_merged>
            -DLIBEPEE=$<TARGET_FILE:epee>
            -DLIBEASYLOGGING=$<TARGET_FILE:easylogging>
            -DLIBRANDOMX=$<TARGET_FILE:randomx>
            -P "${CMAKE_CURRENT_SOURCE_DIR}/wallet_merge_gnu_ar.cmake"
          COMMENT "Fold epee/easylogging/randomx into libwallet_merged.a (MinGW GNU ar MRI)"
          VERBATIM)
      elseif(UNIX AND NOT APPLE)
        # Linux: fold at build time via build/ci/fold-wallet-merged-archive.sh (thin LLVM .a).
      endif()
    endif()
    if(IOS)
        set(lib_folder lib-${ARCH})
    else()
        set(lib_folder lib)
    endif()
    install(TARGETS wallet_merged
        ARCHIVE DESTINATION ${lib_folder})
endif()

add_subdirectory(api)
