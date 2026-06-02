# Arqma Wallet FFI 1.0.13

## Highlights

- **iOS open after sleep:** `open_wallet` waits up to 30s for background refresh/rescan to finish before replacing the session.
- **Clearer open failures:** map useless C++ labels (`basic_string`) to a user-facing message in `wallet2_api_wrapper.cpp`.
- **Includes 1.0.12:** deferred refresh on open, `bad_alloc` messaging, `refresh_async_start` after open, session close before open.

## Artifacts

Standard platform zips on this release (iOS, Android, Linux, macOS, Windows, solo pool sidecars).
