## Arqma Wallet FFI 1.0.4

Prebuilt **arqma-wallet-flutter-ffi** libraries and **desktop solo pool** sidecar.

### New in 1.0.4

- **Desktop solo pool:** `arqma_flutter_solo_pool` built for **Windows, Linux, macOS only** (not Android/iOS).
- Release assets: `arqma-wallet-solo-pool-{platform}-{version}.zip` with `{platform}/<binary>` + `solo-pool-manifest.json`.
- Pure Rust sidecar (no Tauri / no `wallet_merged` link) — crate `arqma-flutter-solo-pool` in this repo.

### Solo pool assets

| Platform | Zip |
|----------|-----|
| Linux x86_64 | `arqma-wallet-solo-pool-linux-x86_64-1.0.4.zip` |
| macOS arm64 | `arqma-wallet-solo-pool-macos-arm64-1.0.4.zip` |
| Windows x86_64 GNU | `arqma-wallet-solo-pool-windows-x86_64-gnu-1.0.4.zip` |

### Consumers

[Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM) installs the binary into `rust/tauri-app/src-tauri/bin/` via `build/ci/fetch-arqma-wallet-solo-pool-release*.sh|ps1`.

### Wallet FFI platforms

Same as 1.0.3 — see `build/ci/release-notes-ffi-1.0.3.md` for FFI library assets and macOS Gatekeeper notes.
