## Arqma Wallet FFI 1.0.12

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Changes

- **Lower iOS/mobile open RAM peak**: `wallet2_open` runs daemon `init` only and defers background `startRefresh` until the first `refresh` RPC (avoids overlapping cache load with refresh on memory-constrained devices).
- **Clearer OOM errors**: `wallet2_open` and shared exception handling map `std::bad_alloc` to actionable messages when wallet cache files exceed available RAM.
- **`open_wallet` lifecycle**: close any existing session before opening another wallet; start async refresh after a successful open so the UI can sync without an immediate full refresh in native open.
- **Includes 1.0.11**: wallet file safety during background jobs, mutating RPC guard while `wallet_background_busy`.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.11...1.0.12

### Assets

| Platform | Archive |
|----------|---------|
| Android (arm64) | `arqma-wallet-ffi-android-arm64-1.0.12.zip` |
| Android (x86_64) | `arqma-wallet-ffi-android-x86_64-1.0.12.zip` |
| iOS | `arqma-wallet-ffi-ios-1.0.12.zip` |
| Linux (x86_64) | `arqma-wallet-ffi-linux-x86_64-1.0.12.zip` |
| macOS (arm64) | `arqma-wallet-ffi-macos-arm64-1.0.12.zip` |
| Windows (x86_64-gnu) | `arqma-wallet-ffi-windows-x86_64-gnu-1.0.12.zip` |
| Solo pool (Linux) | `arqma-wallet-solo-pool-linux-x86_64-1.0.12.zip` |
| Solo pool (macOS) | `arqma-wallet-solo-pool-macos-arm64-1.0.12.zip` |
| Solo pool (Windows) | `arqma-wallet-solo-pool-windows-x86_64-gnu-1.0.12.zip` |

Checksums: `SHA256SUMS-ffi-1.0.12.txt`.
