use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletRpcError {
    #[error("JSON-RPC error: {0}")]
    JsonRpc(Value),
    #[error("transport: {0}")]
    Transport(String),
    #[error("wallet backend not configured")]
    NoBackend,
    #[error("operation not supported by this backend yet: {0}")]
    Unsupported(&'static str),
}
