## Arqma Wallet FFI 1.0.8

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes

- **Full rescan**: `rescan_blockchain` uses `rescanBlockchainAsync()` on a background thread so the wallet session mutex is not held for hours; a height poller updates `getheight` stale cache every ~2s for Flutter heartbeat / Live Activity progress.
- **wallet2 API**: `wallet2_rescan_blockchain_async` FFI entry point.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.7...1.0.8

### Assets

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `arqma-wallet-ffi-linux-x86_64-1.0.8.zip` |
| macOS arm64 | `arqma-wallet-ffi-macos-arm64-1.0.8.zip` |
| Windows x86_64 GNU | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.8.zip` |
| Android arm64 | `arqma-wallet-ffi-android-arm64-1.0.8.zip` |
| Android x86_64 | `arqma-wallet-ffi-android-x86_64-1.0.8.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.8.zip` |

Desktop solo pool sidecars: `arqma-wallet-solo-pool-<platform>-1.0.8.zip`.

Checksums: `SHA256SUMS-ffi-1.0.8.txt`.
