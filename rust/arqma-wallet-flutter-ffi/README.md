# `arqma-wallet-flutter-ffi`

C ABI (`cdylib`) around [`Wallet2ApiClient`](https://github.com/ArqTras/Arqma-GUI-MM/blob/main/rust/arqma-wallet-rpc/src/wallet2_client.rs) so the **Flutter** shell can call the same **linked `wallet2_api`** stack as **Tauri** (no `arqma-wallet-rpc` subprocess when the library is bundled).

## Build

Requires an Arqma core checkout and `libwallet_merged` (or equivalent) per [`rust/docs/NATIVE_WALLET2.md`](../docs/NATIVE_WALLET2.md).

This crate ships a `build.rs` so the **`cdylib`** link pulls the same MinGW system libraries as the Tauri app (OpenSSL, Boost, ICU, Win32 imports, …). It also wraps upstream CMake archives **`libepee.a`**, **`libeasylogging.a`**, **`libcryptonote_format_utils_basic.a`**, **`liblmdb.a`** in `-Wl,--whole-archive` (same objects must be on the link line as for `wallet_merged`).

**Windows:** ship **`arqma_wallet_flutter_ffi.dll`** plus the usual MinGW dependency DLLs in **`runner/Release/`** next to the app (see `flutter/arqma_wallet_gui/tool/package_flutter_release.ps1`). Optional: **`libwallet_merged.a`** is copied there when `rust/arqma-rpc-upstream/build-mingw/` exists (CI / local MinGW build). Fully static “one DLL only” is not supported with stock MSYS2 because **Boost.Locale** is built against ICU **shared** imports (`__imp_*`), which conflicts with linking ICU entirely as static `.a` archives.

**Linux / macOS (portable bundles, AppImage, tarballs):** the default `cdylib` link leaves **`NEEDED`** entries for Boost/OpenSSL/etc. matching the **build host**. Set **`ARQMA_WALLET_FFI_STATIC_HYBRID=1`** when building `arqma-wallet-flutter-ffi` to fold native deps into the library. If **`contrib/depends/<host>/lib`** exists under **`ARQMA_WALLET2_UPSTREAM_DIR`** (after **`make -C contrib/depends`** — see **`build/ci/build-arqma-wallet-ffi-deps.sh`**), **`build.rs`** prepends that path and links **zmq** / **unbound** **statically** too (PIC archives). Without depends, **zmq** / **unbound** stay **dynamic**. **ICU** (Boost.Locale) and **`libstdc++`** (Linux) typically stay **dynamic**. Override the search path with **`ARQMA_WALLET_FFI_DEPENDS_LIB_DIR`** if needed. Release CI sets **`ARQMA_WALLET_FFI_USE_DEPENDS`** and **`ARQMA_WALLET_FFI_STATIC_HYBRID`** for Linux/macOS.

**One-shot helpers**

- **Windows (PowerShell):** `rust/tool/build_native_wallet_flutter_ffi_windows.ps1` — runs `npm run build:arqma:mingw` then `cargo build …--target x86_64-pc-windows-gnu` and optionally `flutter build windows --release`.
- **Linux / macOS:** `bash rust/tool/build_native_wallet_flutter_ffi_unix.sh` (from repo root or `rust/`, after upstream clone).

From the repository `rust/` directory:

```bash
bash tool/build_wallet_flutter_ffi.sh
```

Artifacts (host triple):

- macOS: `target/release/libarqma_wallet_flutter_ffi.dylib`
- Linux: `target/release/libarqma_wallet_flutter_ffi.so`
- Windows: `target/release/arqma_wallet_flutter_ffi.dll`

## C symbols

- `arqma_wallet_ffi_configure(wallet_dir, daemon_address, network)` — `network`: `0` mainnet, `1` testnet, `2` stagenet; UTF-8 C strings.
- `arqma_wallet_ffi_call_json(method, params_json, out_json)` — `out_json` receives an allocated JSON document; free with `arqma_wallet_ffi_string_free`.
- `arqma_wallet_ffi_reset` — drop the client.
- `arqma_wallet_ffi_string_free`

## Flutter packaging

- **macOS:** Xcode “Copy Arqma Tauri bins” copies the dylib into `Contents/Frameworks/` when present under `rust/target/…`.
- **Linux / Windows:** `flutter/arqma_wallet_gui/linux|windows/CMakeLists.txt` installs the library into the Flutter bundle (`lib/` on Linux and **`runner/Release/`** next to the exe on Windows).
- **Manual:** `flutter/arqma_wallet_gui/tool/copy_arqma_tauri_bins.sh` supports `.app`, Linux `bundle/`, and Windows `runner/Release`.

Dart discovery and env vars: `flutter/arqma_wallet_gui/lib/core/desktop/wallet_native_ffi.dart`.

## CI

GitHub Actions **`.github/workflows/desktop-release.yml`** (Flutter jobs on tag pushes `v*` / semver `*.*.*`) builds **`wallet_merged`**, copies **`arqmad`** into `src-tauri/bin/` (no `arqma-wallet-rpc` there), then `cargo build -p arqma-wallet-flutter-ffi --release` on **macOS**, **Linux**, and **Windows** (Windows: `--target x86_64-pc-windows-gnu`, same idea as the Tauri Windows job).
