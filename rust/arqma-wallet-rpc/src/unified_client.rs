use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::{WalletJsonRpc, WalletRpcError};

pub enum WalletClient {
    Disabled,
    #[cfg(feature = "http-digest")]
    HttpDigest(Arc<crate::WalletRpcClient>),
    Wallet2(Arc<crate::Wallet2ApiClient>),
}

impl Clone for WalletClient {
    fn clone(&self) -> Self {
        match self {
            Self::Disabled => Self::Disabled,
            #[cfg(feature = "http-digest")]
            Self::HttpDigest(c) => Self::HttpDigest(Arc::clone(c)),
            Self::Wallet2(c) => Self::Wallet2(Arc::clone(c)),
        }
    }
}

impl WalletClient {
    #[cfg(feature = "http-digest")]
    pub fn new(http: &reqwest::Client, host: &str, port: u16, user: String, pass: String) -> Self {
        Self::HttpDigest(Arc::new(crate::WalletRpcClient::new(
            http, host, port, user, pass,
        )))
    }

    pub fn from_wallet2(client: crate::Wallet2ApiClient) -> Self {
        Self::Wallet2(Arc::new(client))
    }

    pub fn fork_for_heartbeat(&self) -> Self {
        match self {
            Self::Disabled => Self::Disabled,
            #[cfg(feature = "http-digest")]
            Self::HttpDigest(c) => Self::HttpDigest(Arc::new(c.fork_for_heartbeat())),
            Self::Wallet2(c) => Self::Wallet2(Arc::new(c.fork_for_heartbeat())),
        }
    }

    pub fn split_session(&self) -> Self {
        match self {
            Self::Disabled => Self::Disabled,
            #[cfg(feature = "http-digest")]
            Self::HttpDigest(c) => Self::HttpDigest(Arc::new(c.split_session())),
            Self::Wallet2(c) => Self::Wallet2(Arc::new(c.split_session())),
        }
    }

    pub async fn call(&self, _method: &str, _params: &Value) -> Result<Value, String> {
        match self {
            Self::Disabled => Err("wallet backend not configured".to_string()),
            #[cfg(feature = "http-digest")]
            Self::HttpDigest(c) => c.call(_method, _params).await,
            Self::Wallet2(c) => c
                .call_json(_method, _params)
                .await
                .map_err(|e| e.to_string()),
        }
    }
}

#[async_trait]
impl WalletJsonRpc for WalletClient {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, WalletRpcError> {
        self.call(method, params)
            .await
            .map_err(WalletRpcError::Transport)
    }
}
