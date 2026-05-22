# `arqma-wallet-rpc` (Rust workspace crate)

This is **not** a rebuild of the C++ `arqma-wallet-rpc` binary. It is the **Rust integration layer** for the Arqma GUI so wallet access can move from “spawn exe + HTTP” toward “FFI / linked upstream” without rewriting wallet2.

- **`resolve_wallet_rpc_path` / `resolve_daemon_path`**: find **`arqma-wallet-rpc`** and **`arqmad`** produced by upstream (`ARQMA_BUILD_DIR` → `bin/`, `ARQMA_INSTALL_PREFIX`, explicit env, `PATH`, then caller-supplied bundle paths). See `src/upstream_paths.rs`.
- **Feature `http-digest`**: `WalletRpcClient` (HTTP JSON-RPC + MD5 digest) and `impl WalletJsonRpc` — enable from the Tauri crate (already wired in workspace).
- Plan and upstream file map: `../../docs/WALLET_RUST_PORT.md`
- Upstream repo: <https://github.com/arqma/arqma>

Next steps for contributors: implement `WalletJsonRpc` for the existing `reqwest` digest client in `tauri-app`, then optionally add an `ffi` feature and `build.rs` with `-DBUILD_GUI_DEPS=ON` against a `vendor/arqma` checkout.
