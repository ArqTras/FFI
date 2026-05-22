//! Arqma wallet RPC integration for the desktop GUI.
//!
//! The **default** path is the statically linked **`wallet2_api`** stack (`arqma-wallet2-api` → C++), exposed
//! through [`Wallet2ApiClient`] with JSON-RPC method names compatible with **`arqma-wallet-rpc`** from
//! **[arqtras/arqma](https://github.com/arqtras/arqma) `pospow`** (same headers / `libwallet_merged` on
//! **Windows, Linux, and macOS** — see `rust/docs/NATIVE_WALLET2.md`).
//!
//! Optional **`http-digest`** feature: subprocess `arqma-wallet-rpc` + HTTP digest for full upstream RPC.
//!
//! See `rust/docs/NATIVE_WALLET2.md` and `docs/WALLET_RUST_PORT.md`.

mod error;
mod rpc_method_aliases;
mod traits;
mod upstream_paths;

#[cfg(feature = "http-digest")]
mod http_digest;
mod unified_client;
mod wallet2_client;
#[cfg(feature = "http-digest")]
mod wallet_http_client;

pub use error::WalletRpcError;
pub use traits::WalletJsonRpc;
pub use upstream_paths::{
    find_in_path, resolve_arqma_executable, resolve_daemon_path, resolve_wallet_rpc_path,
    ArqmaExecutableKind,
};

pub use arqma_wallet2_api::NetworkKind;
pub use unified_client::WalletClient;
pub use wallet2_client::{Wallet2ApiClient, Wallet2ApiConfig};
#[cfg(feature = "http-digest")]
pub use wallet_http_client::WalletRpcClient;
