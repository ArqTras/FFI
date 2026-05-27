## Arqma Wallet FFI 1.0.6

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Fixes (Windows)

- **LoadLibrary error 1114** on `arqma_wallet_flutter_ffi.dll` (Inno Setup / portable installs): stop re-linking `libepee` / `libeasylogging` / `libcryptonote_format_utils_basic` / `liblmdb` with `--whole-archive` when `wallet_merged` already includes those objects on **windows-gnu** (duplicate static init).
- **`windows::check_admin`**: omit the FFI stub when MinGW `wallet_merged` already links **daemonizer** (`patch-arqma-mingw-gui`).
- **`register_service_node`**: compile only when upstream `wallet2_api.h` exposes `registerServiceNode` (after `patch-arqma-register-service-node.sh`).

### Flutter desktop

- Removed legacy **`ARQMA_FLUTTER_WALLET_RPC_MODE=subprocess`** / **`arqma-wallet-rpc`** fallback — native FFI only.

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.5...1.0.6
