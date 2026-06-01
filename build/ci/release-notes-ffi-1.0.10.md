## Arqma Wallet FFI 1.0.10

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes

- **Full rescan progress**: background `rescan_blockchain` poller uses `scan_heights()` and waits for a real rewind before treating the wallet as caught up — fixes frozen **0%** UI when height was still at the pre-rescan tip.
- **`getheight` during background jobs**: while rescan/refresh runs, returns poller cache / `scan_heights` instead of overwriting with a stale pre-rescan tip.
- **Includes 1.0.9**: async `refresh` with live height polling during wallet catch-up.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.9...1.0.10

### Assets

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `arqma-wallet-ffi-linux-x86_64-1.0.10.zip` |
| macOS arm64 | `arqma-wallet-ffi-macos-arm64-1.0.10.zip` |
| Windows x86_64 GNU | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.10.zip` |
| Android arm64 | `arqma-wallet-ffi-android-arm64-1.0.10.zip` |
| Android x86_64 | `arqma-wallet-ffi-android-x86_64-1.0.10.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.10.zip` |

Desktop solo pool sidecars: `arqma-wallet-solo-pool-<platform>-1.0.10.zip`.

Checksums: `SHA256SUMS-ffi-1.0.10.txt`.
