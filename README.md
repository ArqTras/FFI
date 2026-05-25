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

**Desktop solo pool** (Flutter sidecar, not mobile):

| Platform | Asset zip | Binary inside |
|----------|-----------|---------------|
| Linux x86_64 | `arqma-wallet-solo-pool-linux-x86_64-{version}.zip` | `linux-x86_64/arqma_flutter_solo_pool` |
| macOS (Apple Silicon) | `arqma-wallet-solo-pool-macos-arm64-{version}.zip` | `macos-arm64/arqma_flutter_solo_pool` |
| Windows x86_64 (MinGW GNU) | `arqma-wallet-solo-pool-windows-x86_64-gnu-{version}.zip` | `windows-x86_64-gnu/arqma_flutter_solo_pool.exe` |

Each zip includes `solo-pool-manifest.json` (`product`, `version`, `platform`, `binary`).

**Consumers** (e.g. [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM)) fetch and install into `rust/tauri-app/src-tauri/bin/`:

```bash
# Linux / macOS
ARQMA_SOLO_POOL_PLATFORMS=linux-x86_64 bash build/ci/fetch-arqma-wallet-solo-pool-release-linux.sh
```

```powershell
# Windows
.\build\ci\fetch-arqma-wallet-solo-pool-release.ps1 -Platforms windows-x86_64-gnu
```

Or: `bash build/ci/fetch-arqma-wallet-solo-pool-release.sh` (wrapper). Set `ARQMA_SOLO_POOL_RELEASE_VERSION` / `ARQMA_FFI_RELEASE_VERSION` to match the FFI tag.

Release zips: `arqma-wallet-solo-pool-{platform}-{version}.zip` (alongside `arqma-wallet-ffi-…` assets).

Download **GitHub Release** assets (e.g. `1.0.0`) from [Releases](https://github.com/ArqTras/FFI/releases).

## Build locally

1. Clone upstream: `bash build/ci/clone-arqma.sh`
2. From `rust/`:
   - **Linux / macOS:** `bash ../rust/tool/build_native_wallet_flutter_ffi_unix.sh`
   - **Windows (MSYS2):** `rust\tool\build_native_wallet_flutter_ffi_windows.ps1`
   - **Android:** NDK r28+ and `bash rust/tool/build_mobile_wallet_ffi_android.sh`
   - **iOS:** Xcode and `bash rust/tool/build_mobile_wallet_ffi_ios.sh`
   - **Desktop solo pool (no wallet_merged):** `bash rust/tool/build_flutter_solo_pool.sh`

See [rust/docs/NATIVE_WALLET2.md](rust/docs/NATIVE_WALLET2.md).

## CI

`.github/workflows/release.yml` builds all platforms on tag `1.0.0` / `v1.0.0` and uploads zip assets to the matching GitHub Release.

## Crates

- `arqma-wallet-flutter-ffi` — C ABI for Flutter
- `arqma-flutter-solo-pool` — desktop Stratum solo-pool sidecar (`arqma_flutter_solo_pool`)
- `arqma-wallet-core` — shared config/paths (solo pool + future hosts)
- `arqma-wallet2-api` — wallet2 native bindings
- `arqma-wallet-rpc` — shared RPC types (FFI dependency)

Consumer apps (e.g. [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM)) can depend on release artifacts instead of rebuilding upstream locally.
