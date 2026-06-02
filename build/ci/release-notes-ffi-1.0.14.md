# Arqma Wallet FFI 1.0.14

## Highlights

- **macOS / desktop open stability:** Heavy wallet2 RPC (`open_wallet`, `close_wallet`, restore, create) runs on a dedicated pthread with an **8 MiB stack**, avoiding Dart isolate stack overflow on large wallet cache files.
- **FFI call serialization:** Process-wide mutex around `configure`, `call_json`, and `reset` so concurrent Dart isolate calls cannot race wallet2.
- **Deferred sync on open:** `open_wallet` no longer calls `refresh_async_start` immediately; background sync starts on the first `refresh` RPC (pairs with Flutter post-open refresh kick).
- **Includes 1.0.13:** iOS open-after-sleep wait, `basic_string` error sanitization, 1.0.12 deferred refresh on open and `bad_alloc` messaging.

## Artifacts

Standard platform zips on this release (iOS, Android, Linux, macOS, Windows, solo pool sidecars).

**Full changelog:** https://github.com/ArqTras/FFI/compare/1.0.13...1.0.14
