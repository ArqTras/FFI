# Arqma Wallet FFI

Prebuilt **native wallet FFI** libraries for Flutter and other hosts, built from [arqtras/arqma](https://github.com/arqtras/arqma) (`wallet_merged` + `arqma-wallet-flutter-ffi`).

## Platforms

| Platform | Artifact |
|----------|----------|
| Linux x86_64 | `libarqma_wallet_flutter_ffi.so` |
| macOS (Apple Silicon) | `libarqma_wallet_flutter_ffi.dylib` |
| Windows x86_64 (MinGW GNU) | `arqma_wallet_flutter_ffi.dll` |
| Android | `jniLibs/{arm64-v8a,x86_64}/libarqma_wallet_flutter_ffi.so` |
| iOS | device + simulator `.dylib` |

Download **GitHub Release** assets (e.g. `1.0.0`) from [Releases](https://github.com/ArqTras/FFI/releases).

## Build locally

1. Clone upstream: `bash build/ci/clone-arqma.sh`
2. From `rust/`:
   - **Linux / macOS:** `bash ../rust/tool/build_native_wallet_flutter_ffi_unix.sh`
   - **Windows (MSYS2):** `rust\tool\build_native_wallet_flutter_ffi_windows.ps1`
   - **Android:** NDK r28+ and `bash rust/tool/build_mobile_wallet_ffi_android.sh`
   - **iOS:** Xcode and `bash rust/tool/build_mobile_wallet_ffi_ios.sh`

See [rust/docs/NATIVE_WALLET2.md](rust/docs/NATIVE_WALLET2.md).

## CI

`.github/workflows/release.yml` builds all platforms on tag `1.0.0` / `v1.0.0` and uploads zip assets to the matching GitHub Release.

## Crates

- `arqma-wallet-flutter-ffi` — C ABI for Flutter
- `arqma-wallet2-api` — wallet2 native bindings
- `arqma-wallet-rpc` — shared RPC types (FFI dependency)

Consumer apps (e.g. [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM)) can depend on release artifacts instead of rebuilding upstream locally.
