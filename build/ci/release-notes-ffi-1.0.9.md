## Arqma Wallet FFI 1.0.9

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes

- **Wallet sync progress**: `refresh` uses `refreshAsync()` on a background thread with a height poller (same pattern as full rescan) so `getheight` stale cache updates every ~2s during catch-up — fixes frozen **0%** footer / Live Activity while the wallet scans after open or manual refresh.
- **wallet2 API**: `wallet2_refresh_async_start`, `wallet2_read_scan_heights` FFI entry points; `Wallet2Session::refresh_async_start` / `scan_heights` in Rust.
- **Includes 1.0.8**: async `rescan_blockchain` + live height during full rescan.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.8...1.0.9

### Assets

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `arqma-wallet-ffi-linux-x86_64-1.0.9.zip` |
| macOS arm64 | `arqma-wallet-ffi-macos-arm64-1.0.9.zip` |
| Windows x86_64 GNU | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.9.zip` |
| Android arm64 | `arqma-wallet-ffi-android-arm64-1.0.9.zip` |
| Android x86_64 | `arqma-wallet-ffi-android-x86_64-1.0.9.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.9.zip` |

Desktop solo pool sidecars: `arqma-wallet-solo-pool-<platform>-1.0.9.zip`.

Checksums: `SHA256SUMS-ffi-1.0.9.txt`.
