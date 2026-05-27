## Arqma Wallet FFI 1.0.7

Prebuilt **arqma-wallet-flutter-ffi** libraries for desktop and mobile builds.

### Fixes

- **Stake**: `wallet2` `stakePending` expects a decimal coin amount string (9 fractional digits); the in-process RPC layer was forwarding raw atomic units, causing `"Incorrect amount"` for typical stake sizes (e.g. 100–1000 ARQ).

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.6...1.0.7
