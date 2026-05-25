## Arqma Wallet FFI 1.0.3

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes from 1.0.2

- **Windows wallet sync stall:** When background `startRefresh` holds the refresh mutex but block height stops advancing, `refresh_from_height` now calls `pauseRefresh()`, `setRefreshFromBlockHeight()`, synchronous `refresh()`, and on failure falls back to `refreshAsync()` before resuming with `startRefresh()`.
- **Diagnostics:** On sync `refresh()` failure, logs `connected`, `daemon_h`, and `wallet_h` to stderr for stall triage.
- Fixes wallets stuck thousands of blocks behind the daemon tip on Windows (confirmed with checkpoint-height wallets).

### Also includes (from 1.0.2 / 1.0.1)

- **Windows:** `pauseRefresh()` before `refresh_from_height` catch-up; `setRefreshFromBlockHeight` + `refresh` for upstream without `refreshFromHeight` API.
- **iOS:** Link **liblmdb** alongside `wallet_merged` (fixes undefined `mdb_*` symbols at link time).
- **CI:** MinGW `wallet_merged` fold via GNU `ar` MRI; Android epee symbol fold for prebuilt JNI libs.
- Bare `refresh` returning `false` while `startRefresh` is active is treated as non-fatal in the wallet RPC layer.

### macOS — Gatekeeper / quarantine

Downloaded zips may carry the quarantine extended attribute. If Gatekeeper blocks the consumer app or dylib, run once:

```bash
xattr -cr "/Applications/Arqma-Wallet.app"
```

Or right-click the app → **Open** → confirm **Open** once (see [Arqma-GUI-MM README](https://github.com/ArqTras/Arqma-GUI-MM#macos--running-on-other-macs)).

### Platforms

| Platform | Asset |
|----------|--------|
| Linux x86_64 | `arqma-wallet-ffi-linux-x86_64-1.0.3.zip` |
| macOS arm64 | `arqma-wallet-ffi-macos-arm64-1.0.3.zip` |
| Windows x86_64 GNU | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.3.zip` |
| Android arm64 | `arqma-wallet-ffi-android-arm64-1.0.3.zip` |
| Android x86_64 | `arqma-wallet-ffi-android-x86_64-1.0.3.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.3.zip` |

### Consumers

- [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM) desktop CI (`ARQMA_FFI_RELEASE_VERSION=1.0.3`) and release **5.1.0**.
- Set `ARQMA_FFI_RELEASE_VERSION=1.0.3` when running `build/ci/fetch-arqma-wallet-ffi-release.ps1` locally.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.2...1.0.3
