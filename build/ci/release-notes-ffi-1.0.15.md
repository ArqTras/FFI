# Arqma Wallet FFI 1.0.15

## Highlights

- **Oxen-style sync visibility:** `getheight` now returns `daemon_height` and `background_busy` while `refresh` / `rescan_*` runs, so Flutter can show scan progress even when wallet height tracks the daemon tip.
- **Warm balance cache during scan:** `getheight` and the background refresh poller refresh `balance` / `unlocked_balance` stale caches every **1 s** (was 2 s) so heartbeat `getbalance` does not return zeros while the session mutex is held by the scanner.
- **Safer `get_transfers` during scan:** when the session is busy with a background job and the mutex is contended, `get_transfers` returns a **retryable error** instead of empty buckets — Flutter no longer clears transaction history with a blank snapshot mid-scan.
- **Includes 1.0.14:** macOS/desktop open stability (8 MiB pthread stack), FFI call serialization, deferred sync on open.
- **Republish (2026-06-11):** Windows-gnu links **`liblmdb.a` only** (C `mdb_*` symbols; avoids duplicate LMDB static init / Win32 1114). **Scan completion:** background `refresh` / `rescan_*` uses exclusive session lock; `getheight` / `getbalance` / `get_address` return stale cache while busy; `pauseRefresh()` after background jobs — fixes ACCESS_VIOLATION crash near end of blockchain scan.

## Flutter pairing

Requires matching Dart bridge changes in Arqma-GUI-MM (Oxen parity):

- `kWalletDaemonTipToleranceBlocks = 1` for footer progress
- `wallet_syncing` / `background_busy` from `getheight`
- transaction refresh on **balance change** (not every block)

## Artifacts

Standard platform zips on this release (iOS, Android, Linux, macOS, Windows, solo pool sidecars).

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.14...1.0.15
