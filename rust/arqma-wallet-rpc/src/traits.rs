use async_trait::async_trait;
use serde_json::Value;

use crate::WalletRpcError;

/// One JSON-RPC round-trip to the wallet (same surface as `arqma-wallet-rpc` over HTTP).
///
/// Implementations: `WalletRpcClient` (feature `http-digest`), future FFI backend.
#[async_trait]
pub trait WalletJsonRpc: Send + Sync {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, WalletRpcError>;
}
