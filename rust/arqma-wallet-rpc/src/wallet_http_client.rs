//! HTTP JSON-RPC client with digest auth — same wire format as upstream `arqma-wallet-rpc`.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::WalletRpcError;
use crate::http_digest::{build_digest_header, generate_cnonce, inc_nc};
use crate::traits::WalletJsonRpc;

const PATH_JSON_RPC: &str = "/json_rpc";

fn wallet_rpc_trace_enabled() -> bool {
    std::env::var("ARQMA_SYNC_DEBUG")
        .map(|v| {
            let s = v.trim().to_ascii_lowercase();
            matches!(s.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut i = max;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    format!("{}…", &s[..i])
}

/// Queued JSON-RPC to local `arqma-wallet-rpc` with `401` → digest retry (legacy Electron parity).
pub struct WalletRpcClient {
    pub base: String,
    pub user: String,
    pub pass: String,
    pub next_id: AtomicU64,
    pub nc: std::sync::Mutex<String>,
    pub cnonce: String,
    pub http: reqwest::Client,
}

impl WalletRpcClient {
    pub fn new(http: &reqwest::Client, host: &str, port: u16, user: String, pass: String) -> Self {
        Self {
            base: format!("http://{host}:{port}"),
            user,
            pass,
            next_id: AtomicU64::new(0),
            nc: std::sync::Mutex::new("00000001".to_string()),
            cnonce: generate_cnonce(),
            http: http.clone(),
        }
    }

    /// Separate digest session for heartbeat (distinct JSON-RPC `id` stream).
    pub fn fork_for_heartbeat(&self) -> Self {
        Self {
            base: self.base.clone(),
            user: self.user.clone(),
            pass: self.pass.clone(),
            next_id: AtomicU64::new(50_000_000),
            nc: std::sync::Mutex::new("00000001".to_string()),
            cnonce: generate_cnonce(),
            http: self.http.clone(),
        }
    }

    /// Another digest session (e.g. parallel `get_transfers`).
    pub fn split_session(&self) -> Self {
        Self {
            base: self.base.clone(),
            user: self.user.clone(),
            pass: self.pass.clone(),
            next_id: AtomicU64::new(200_000_000),
            nc: std::sync::Mutex::new("00000001".to_string()),
            cnonce: generate_cnonce(),
            http: self.http.clone(),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// POST JSON-RPC; returns full JSON value (includes `error` / `result` like upstream).
    pub async fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let id = self.next_id();
        let mut body = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if !params.is_null() && !params.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            body.as_object_mut()
                .unwrap()
                .insert("params".to_string(), params.clone());
        }
        let s = body.to_string();
        let out = self.post_with_digest(&s).await;
        if let Err(ref e) = out {
            if wallet_rpc_trace_enabled() {
                eprintln!(
                    "[sync-debug][wallet-rpc] {} id={} err={}",
                    method,
                    id,
                    truncate(e, 200)
                );
            }
        }
        out
    }

    async fn post_with_digest(&self, body: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.base, PATH_JSON_RPC);
        let first = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if first.status() == reqwest::StatusCode::UNAUTHORIZED {
            let www = first
                .headers()
                .get("www-authenticate")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());
            let _ = first.text().await;
            let www = www.ok_or_else(|| "missing WWW-Authenticate".to_string())?;
            let nc = { self.nc.lock().map_err(|e| e.to_string())?.clone() };
            let path = PATH_JSON_RPC;
            let header = build_digest_header(
                "POST",
                path,
                &www,
                &self.user,
                &self.pass,
                &nc,
                &self.cnonce,
            )?;
            let r = self
                .http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", header)
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !r.status().is_success() {
                return Err(format!("HTTP {} (digest)", r.status()));
            }
            let t = r.text().await.map_err(|e| e.to_string())?;
            let v = serde_json::from_str(&t).map_err(|e| e.to_string())?;
            {
                let mut g = self.nc.lock().map_err(|e| e.to_string())?;
                *g = inc_nc(&nc);
            }
            return Ok(v);
        }
        if !first.status().is_success() {
            return Err(format!("HTTP {}", first.status()));
        }
        let t = first.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str(&t).map_err(|e| e.to_string())
    }
}

#[async_trait]
impl WalletJsonRpc for WalletRpcClient {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, WalletRpcError> {
        WalletRpcClient::call(self, method, params)
            .await
            .map_err(WalletRpcError::Transport)
    }
}
