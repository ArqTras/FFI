## Arqma Wallet FFI 1.0.4

Prebuilt **native wallet FFI** libraries, **desktop solo pool** sidecar binaries, and **Arqma-GUI-MM 5.1.0** installable app packages (same release train).

Built from [ArqTras/FFI](https://github.com/ArqTras/FFI) tag **1.0.4**. Desktop/mobile GUI bundles are from [ArqTras/Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM) tag **5.1.0** (wallet FFI + solo pool fetched from this FFI release at build time).

---

### Wallet FFI libraries (developers / integrators)

Use these zips to link or bundle `libarqma_wallet_flutter_ffi` without building `wallet_merged` locally. **Not** end-user installers.

| File | Platform |
|------|----------|
| `arqma-wallet-ffi-linux-x86_64-1.0.4.zip` | Linux x86_64 — `libarqma_wallet_flutter_ffi.so` |
| `arqma-wallet-ffi-macos-arm64-1.0.4.zip` | macOS Apple Silicon — `libarqma_wallet_flutter_ffi.dylib` |
| `arqma-wallet-ffi-windows-x86_64-gnu-1.0.4.zip` | Windows x86_64 MinGW — `arqma_wallet_flutter_ffi.dll` |
| `arqma-wallet-ffi-android-arm64-1.0.4.zip` | Android arm64 — `jniLibs/arm64-v8a/libarqma_wallet_flutter_ffi.so` |
| `arqma-wallet-ffi-android-x86_64-1.0.4.zip` | Android x86_64 — `jniLibs/x86_64/libarqma_wallet_flutter_ffi.so` |
| `arqma-wallet-ffi-ios-1.0.4.zip` | iOS — device + simulator `libarqma_wallet_flutter_ffi.dylib` |

---

### Solo pool sidecar (desktop only)

Stratum solo-mining helper `arqma_flutter_solo_pool` for **Windows, Linux, macOS** Flutter/Tauri desktop shells. **Not** included on Android or iOS.

| File | Platform |
|------|----------|
| `arqma-wallet-solo-pool-linux-x86_64-1.0.4.zip` | Linux — `arqma_flutter_solo_pool` |
| `arqma-wallet-solo-pool-macos-arm64-1.0.4.zip` | macOS — `arqma_flutter_solo_pool` |
| `arqma-wallet-solo-pool-windows-x86_64-gnu-1.0.4.zip` | Windows — `arqma_flutter_solo_pool.exe` |

Each zip contains `solo-pool-manifest.json` (product, version, platform, binary name).

---

### GUI desktop installers (end users — Flutter desktop)

Full **Arqma Wallet** desktop apps from GUI **5.1.0** CI. Include bundled daemon, wallet FFI, and solo pool under each app’s `bin/` (or macOS `.app` layout).

| File | Platform / format |
|------|-------------------|
| `Arqma-Wallet-Flutter-5.1.0-1-linux-x64.tar.gz` | Linux x64 portable bundle |
| `Arqma-Wallet-Flutter-5.1.0-1-x86_64.AppImage` | Linux x64 AppImage |
| `Arqma-Wallet-Flutter-5.1.0-1-macos.zip` | macOS `.app` zip |
| `Arqma-Wallet-Flutter-5.1.0-1-macos.dmg` | macOS drag-to-Applications DMG |
| `Arqma-Wallet-Flutter-5.1.0-1-windows-x64.zip` | Windows portable folder zip |
| `Arqma-Wallet-Flutter-5.1.0-1-windows-x64-Setup.exe` | Windows Inno Setup installer |

---

### Mobile GUI (end users — Android / iOS)

From GUI **5.1.0**; use **wallet FFI** zips above only when building from source. These are store/sideload packages.

| File | Description |
|------|-------------|
| `Arqma-Wallet-Android-5.1.0.apk` | Android APK (sideload) |
| `Arqma-Wallet-Android-5.1.0.aab` | Android App Bundle (Play Store) |
| `Arqma-Wallet-Android-5.1.0-manifest.txt` | Android build manifest |
| `SHA256SUMS-android-5.1.0.txt` | SHA-256 checksums (Android artifacts) |
| `Arqma-Wallet-Mobile-5.1.0-ios-testflight.ipa` | iOS IPA (TestFlight / distribution) |
| `Arqma-Wallet-Mobile-5.1.0-ios.xcarchive.zip` | Xcode archive (symbols / resign) |
| `Arqma-Wallet-Mobile-5.1.0-ios-manifest.txt` | iOS build manifest |
| `Arqma-Wallet-Mobile-5.1.0-app-store-screenshots.zip` | App Store screenshot assets |
| `SHA256SUMS.txt` | SHA-256 checksums (iOS artifacts) |
| `TESTFLIGHT.md` | TestFlight upload notes |

---

### macOS — Gatekeeper / quarantine

Downloaded zips may carry quarantine. If macOS blocks the app or dylib:

```bash
xattr -cr "/Applications/Arqma-Wallet.app"
```

Or right-click the app → **Open** → confirm **Open** once.

---

### Consumers

- **Fetch prebuilts:** [Arqma-GUI-MM](https://github.com/ArqTras/Arqma-GUI-MM) `build/ci/fetch-arqma-wallet-ffi-release*` and `fetch-arqma-wallet-solo-pool-release*` (default: Latest FFI release).
- **Build FFI from source:** this repo — see `README.md` and `.github/workflows/release.yml`.

**Full FFI changelog:** https://github.com/ArqTras/FFI/compare/1.0.3...1.0.4

**GUI 5.1.0 release:** https://github.com/ArqTras/Arqma-GUI-MM/releases/tag/5.1.0
