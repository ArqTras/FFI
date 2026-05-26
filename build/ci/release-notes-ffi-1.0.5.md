## Arqma Wallet FFI 1.0.5

Prebuilt **native wallet FFI** libraries and **desktop solo pool** sidecars from [ArqTras/FFI](https://github.com/ArqTras/FFI) tag **1.0.5**.

### Changes

- **`register_service_node`**: native `Wallet::registerServiceNode` (daemon `prepare_registration` line → build and relay registration tx). Fixes Flutter/desktop/mobile FFI returning *unavailable in current native build*.
- CI applies `patch-arqma-register-service-node.sh` on upstream clone before `wallet_merged` build.

### Wallet FFI zips

| File | Platform |
|------|----------|
| `arqma-wallet-ffi-linux-x86_64-1.0.5.zip` | Linux x86_64 |
| `arqma-wallet-ffi-macos-arm64-1.0.5.zip` | macOS Apple Silicon |
| `arqma-wallet-ffi-windows-x86_64-gnu-1.0.5.zip` | Windows x86_64 MinGW |
| `arqma-wallet-ffi-android-arm64-1.0.5.zip` | Android arm64 |
| `arqma-wallet-ffi-android-x86_64-1.0.5.zip` | Android x86_64 |
| `arqma-wallet-ffi-ios-1.0.5.zip` | iOS device + simulator |

### Solo pool (desktop)

| File | Platform |
|------|----------|
| `arqma-wallet-solo-pool-linux-x86_64-1.0.5.zip` | Linux |
| `arqma-wallet-solo-pool-macos-arm64-1.0.5.zip` | macOS |
| `arqma-wallet-solo-pool-windows-x86_64-gnu-1.0.5.zip` | Windows |

**Consumers:** [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM) `build/ci/fetch-arqma-wallet-ffi-release*` (default: Latest → **1.0.5** after publish).

**Compare:** https://github.com/ArqTras/FFI/compare/1.0.4...1.0.5
