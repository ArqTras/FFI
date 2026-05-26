# Native `wallet2` backend (no JSON-RPC transport to wallet logic)

The GUI **always** links the real **`wallet2_api`** stack (`arqma-wallet2-api` → C++/FFI). That requires:

1. **Arqma core sources** (headers; after building upstream, also static libraries for linking).
2. **`npm run tauri:dev`** / **`npm run tauri:build`** / **`npm run ci:tauri`** — no extra Cargo feature flags; the Tauri crate always depends on the native bridge.

**CI note:** **[desktop release CI](../../.github/workflows/desktop-release.yml)** — **`flutter-linux`**, **`flutter-macos`**, i **`flutter-windows`** współdzielą upstream **`wallet_merged`** (**[arqtras/arqma](https://github.com/arqtras/arqma)**, **`pospow`**): klon i skrypty **`build/ci/build-arqma-*.sh`** służą **wyłącznie** do bibliotek natywnych (FFI / `wallet_merged`), **bez** budowy **`arqmad`**. **`arqmad`** dla bundli Flutter pochodzi z **najnowszego release [arqma/arqma](https://github.com/arqma/arqma/releases/latest)**: Linux i macOS — **`build/ci/fetch-arqmad-github-release.sh`**, Windows — **`build/ci/flutter-windows-fetch-arqma-binaries.ps1`**. Lokalnie możesz nadal zbudować **`arqmad`** z drzewa CMake: **`build/ci/build-arqma-daemon-copy.sh`**.

## contrib/depends (Linux / macOS)

With **`ARQMA_WALLET_FFI_USE_DEPENDS=1`**, **`build/ci/build-arqma-linux.sh`** / **`build/ci/build-arqma-macos.sh`** run **`make -C contrib/depends HOST=<triplet>`** under **`rust/arqma-rpc-upstream`**, then CMake with **`contrib/depends/<HOST>/share/toolchain.cmake`** and **`STATIC=ON`**, producing **`libwallet_merged.a`** under **`build/ci-depends-release/`** (same helper targets as the default path). Linux host packages for the depends build are installed via **`build/ci/install-arqma-depends-linux.sh`**. Use **`ARQMA_WALLET_FFI_USE_DEPENDS=0`** for the faster host-dev-package path (**`build/ci-native-release/`**). For a **portable Flutter FFI** `cdylib`, combine with **`ARQMA_WALLET_FFI_STATIC_HYBRID=1`**: **`arqma-wallet-flutter-ffi/build.rs`** then prefers static libs from **`contrib/depends/.../lib`** (including zmq/unbound when that tree exists).

## 0. Building Arqma core on macOS (native + `libwallet_merged`)

To produce **`libwallet_merged.a`** (required for linking the GUI on macOS), reconfigure your CMake build with **`BUILD_GUI_DEPS=ON`** and build the **`wallet_merged`** target:

```bash
cd rust/arqma-rpc-upstream/build/<your_cmake_output_dir>/release
cmake -D BUILD_GUI_DEPS=ON -D CMAKE_BUILD_TYPE=Release ../../../..
cmake --build . --target wallet_merged -j"$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
```

If you already ran `make release` in that tree, you usually only need to reconfigure and run `wallet_merged` as above.

## 1. Upstream directory (headers)

From `rust/arqma-wallet2-api`, the default path is **`../arqma-rpc-upstream`**, i.e. in this repo: **`rust/arqma-rpc-upstream`** — the root of an Arqma checkout (must contain `src/wallet/api/wallet2_api.h`).

```bash
cd rust
git clone -b pospow https://github.com/arqtras/arqma.git arqma-rpc-upstream
```

For the daemon only, use official binaries from **`arqma/arqma` Releases** (see `build/download-binaries.js`), not the checkout above.

Custom location:

```bash
export ARQMA_WALLET2_UPSTREAM_DIR=/path/to/Arqma
```

## 2. Building the Tauri GUI (native — default)

From **`rust/tauri-app`**:

```bash
npm run tauri:dev
npm run tauri:build
```

Legacy aliases (same as above): `tauri:dev:native`, `tauri:build:native`.

Without npm:

```bash
node scripts/with-rust-target.mjs tauri dev
node scripts/with-rust-target.mjs tauri build
```

## 3. Linking the C++ wallet library (by platform)

- **Windows (MSVC / GNU):** `build.rs` has helper paths; you may need **`ARQMA_WALLET2_LIB_DIR`** and optionally **`ARQMA_WALLET2_LIB_NAME`** after building `wallet_merged` in the upstream tree.
- **macOS:** `build.rs` searches Homebrew prefixes and can auto-detect `libwallet_merged.a` under `arqma-rpc-upstream/build/...`; ensure **`BUILD_GUI_DEPS`** / **`wallet_merged`** were built (section 0).
- **Linux:** you may need to set **`ARQMA_WALLET2_LIB_DIR`** (and matching system dev packages) similar to Windows until first-class auto-linking is added.

If the build fails on **missing `wallet2_api.h`**, fix **`ARQMA_WALLET2_UPSTREAM_DIR`** or clone upstream into **`rust/arqma-rpc-upstream`** (section 1).

## 4. Native JSON-RPC adapter (`rust/arqma-wallet-rpc`)

Flutter and Tauri do not talk to a separate **`arqma-wallet-rpc`** process by default. The **`Wallet2ApiClient`** in **`arqma-wallet-rpc`** exposes the same JSON-RPC **method names** the GUI already uses, backed by **`Wallet2Session`** (C++ **`wallet2_api`** from **`arqtras/arqma`** **`pospow`**). Optional HTTP digest mode (`http-digest` feature + subprocess) still targets real **`arqma-wallet-rpc`** for migration.

Behavioral notes:

- **Transfers**: `transfer_split` / `transfer` use native `createTransaction` + `exportPendingRelaySlices` + `relay_tx` + `relayTxFromMetadataHex` when those APIs exist in your headers and merged library (see **`arqma-wallet2-api/build.rs`**). Always rebuild **`wallet_merged`** from the **same** Arqma commit as **`wallet2_api.h`**.
- **`refresh`**: forwarded to **`Wallet::refresh()`** on a background worker (same pattern as **`rescan_spent`**).
- **`register_service_node`**: implemented via **`Wallet::registerServiceNode`** in upstream **`wallet2_api`** (parses the daemon `prepare_registration` line, builds and relays the tx). Rebuild **`wallet_merged`** after changing **`wallet2_api.h`** / **`wallet.cpp`** (CI applies `patch-arqma-register-service-node.sh` on clone).
- **`can_request_stake_unlock` / `request_stake_unlock`**: still return JSON **`error`** stubs from the C++ bridge until exposed on **`wallet2_api`**.
- **`getbalance`**: includes **`per_subaddress`** and **`num_unspent_outputs`** for RPC shape parity. On the native bridge, **`num_unspent_outputs`** is a conservative gate (`1` when **`unlocked_balance > 0`**, else `0`) because `wallet2_api` does not expose per-coin counts on this path; use **`incoming_transfers`** when you need an output list.
- **`incoming_transfers` / `get_version`**: implemented on the Rust adapter; **`incoming_transfers`** maps to the native history **`in`** bucket (see `Wallet2ApiClient`).
- **Alternate JSON-RPC method names** (e.g. `get_balance` → `getbalance`): see **`arqma-wallet-rpc/src/rpc_method_aliases.rs`**.

## Windows MinGW (`x86_64-pc-windows-gnu`)

**Windows only:** `npm run ci:tauri:native:windows-gnu` runs **`scripts/run-windows-gnu-tauri-ci.mjs`**, which exits with a hint on Linux/macOS — on those platforms use **`npm run ci:tauri:native`** (same as **`npm run ci:tauri`**).

From **`rust/tauri-app`**, after installing MSYS2 **MINGW64** toolchain and deps (Boost, OpenSSL, … — mirror **`desktop-release.yml`** job **tauri** MSYS package list):

```bash
npm run clone:arqma
npm run build:arqma:mingw
```

Equivalent: run **`build/ci/clone-arqma.sh`** and **`build/ci/build-arqma-mingw.sh`** from a **MINGW64** shell (`bash`), with **`ARQMA_WALLET2_UPSTREAM_DIR`** pointing at your checkout if it is not **`rust/arqma-rpc-upstream`**.

1. **Rebuild upstream** after CMake fixes: **`build-arqma-mingw.sh`** sets **`CMAKE_SYSTEM_PROCESSOR=x86_64`** and **`ARCH=native`** so RandomX includes **`jit_compiler_x86`**. If **`librandomx.a`** was built without x86 JIT objects, GNU ld reports undefined references to **`randomx::JitCompilerX86::*`** when linking the Tauri DLL.
2. **`rust/tauri-app/scripts/with-rust-target.mjs`** sets **`CARGO_PROFILE_RELEASE_LTO=thin`** when the command line includes **`x86_64-pc-windows-gnu`** (unless **`CARGO_PROFILE_RELEASE_LTO`** is already set). Applies to **`npm run ci:tauri:native:windows-gnu`**, **`npm run release:win`**, and any **`node scripts/with-rust-target.mjs cargo … --target x86_64-pc-windows-gnu`**. This avoids occasional MinGW / libstdc++ issues such as **`__real___cxa_throw`** during the final link.
3. **`npm run ci:tauri:native:windows-gnu`** runs **`scripts/run-windows-gnu-tauri-ci.mjs`**: Vite/npm use a **`PATH`** without **`msys64`** (avoids native-addon / **`bad_weak_ptr`** crashes). The **`cargo tauri build`** step appends **`%MSYS2_ROOT%\mingw64\bin`** and **`usr\bin`** at the **end** of **`PATH`** (root override: **`MSYS2_ROOT`** / **`ARQMA_MSYS2_ROOT`**), sets **`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER`**, **`ARQMA_WALLET2_MSYS_ROOT`**, **`ARQMA_MINGW_BIN`**, and default **`ARQMA_WALLET2_UPSTREAM_DIR`** when unset — so **`arqma-wallet2-api`** emits the right **`-l…`** for MinGW while **MSVC host** `build.rs` (e.g. **`vswhom-sys`**) still uses **`cl`+`link`**. It clears stray **`CC`/`CXX`** for that Cargo step (MSYS shells often set them). It uses **`cargo tauri build`** (not **`npx tauri`**) and merges **`scripts/tauri-ci-gnu-no-frontend.json`** so **`beforeBuildCommand`** is not run twice.
