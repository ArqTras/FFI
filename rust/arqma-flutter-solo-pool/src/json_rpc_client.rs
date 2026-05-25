//! Plain HTTP JSON-RPC for `arqmad` (no wallet digest client).

use serde_json::{json, Value};

const PATH_JSON_RPC: &str = "/json_rpc";

/// `Daemon.sendRPC` — unauthenticated plain POST.
pub async fn daemon_post(
    client: &reqwest::Client,
    host: &str,
    port: u16,
    method: &str,
    id: u64,
    params: &Value,
) -> Result<Value, String> {
    let url = format!("http://{host}:{port}{PATH_JSON_RPC}");
    let mut body = json!({ "jsonrpc": "2.0", "id": id, "method": method });
    if !params.is_null() && !params.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        body.as_object_mut()
            .unwrap()
            .insert("params".to_string(), params.clone());
    }
    let r = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    r.json().await.map_err(|e| e.to_string())
}
