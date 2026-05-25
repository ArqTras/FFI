## Arqma Wallet FFI 1.0.2

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes from 1.0.1

- **Windows scan stall:** `refresh_from_height` calls `pauseRefresh()` before `setRefreshFromBlockHeight()` + `refresh()` so a stuck background refresh thread releases the mutex.
- **iOS:** Link **liblmdb** alongside `wallet_merged` (fixes undefined `mdb_*` symbols at link time).
- **CI:** MinGW `wallet_merged` fold via GNU `ar` MRI; Android epee symbol fold for prebuilt JNI libs.

### macOS — Gatekeeper / quarantine

Downloaded zips may carry the quarantine extended attribute. If Gatekeeper blocks the consumer app or dylib, run once:

```bash
xattr -cr "/Applications/Arqma-Wallet.app"
```

### Platforms

| Platform | Asset |
|----------|--------|
| Linux x86_64 | `arqma-wallet-ffi-linux-x86_64-1.0.2.zip` |
| macOS arm64 | `arqma-wallet-ffi-macos-arm64-1.0.2.zip` |
| Windows x86_64 GNU | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.2.zip` |
| Android arm64 | `arqma-wallet-ffi-android-arm64-1.0.2.zip` |
| Android x86_64 | `arqma-wallet-ffi-android-x86_64-1.0.2.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.2.zip` |

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.1...1.0.2
