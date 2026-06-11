mod backend_state;
mod json_rpc_client;
mod solo_pool;
mod solo_pool_sink;

pub use backend_state::WalletBackendState;

use arqma_wallet_core::{load_config_snapshot, merge_json as merge_json_value};
use serde_json::json;
use solo_pool_sink::JsonlStdoutSoloPoolSink;

/// Config base directory: first non-empty CLI arg, else `ARQMA_CONFIG_DIR`, else OS default.
pub fn resolve_paths_for_flutter_solo_pool_sidecar() -> arqma_wallet_core::ArqmaPaths {
    let mut paths = arqma_wallet_core::default_paths();
    if let Some(a) = std::env::args().nth(1) {
        let t = a.trim();
        if !t.is_empty() {
            paths.config_dir = t.to_string();
            return paths;
        }
    }
    if let Ok(d) = std::env::var("ARQMA_CONFIG_DIR") {
        let t = d.trim();
        if !t.is_empty() {
            paths.config_dir = t.to_string();
        }
    }
    paths
}

/// Standalone process: load `gui/config.json`, run Stratum solo pool, emit gateway-shaped JSON lines on stdout until Ctrl+C.
pub async fn run_flutter_solo_pool_async() -> Result<(), String> {
    let paths = resolve_paths_for_flutter_solo_pool_sidecar();
    let snap = load_config_snapshot(&paths).map_err(|e| e.to_string())?;
    let mut st = WalletBackendState::default();
    st.paths = paths;
    st.config_data = snap.config_data.clone();
    if let Some(pool) = st.config_data.get_mut("pool") {
        solo_pool::strip_legacy_uniform_pool_option(pool);
    }
    let bind_ip = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("server"))
        .and_then(|s| s.get("bindIP"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if bind_ip.is_empty() || bind_ip == "0.0.0.0" || bind_ip == "127.0.0.1" {
        st.config_data = merge_json_value(
            &st.config_data,
            &json!({ "pool": { "server": { "bindIP": solo_pool::preferred_bind_ip() } } }),
        );
    }
    st.config_data = merge_json_value(
        &st.config_data,
        &json!({ "wallet": { "rpc_bind_port": 19999_u64 } }),
    );
    solo_pool::stop(&mut st);
    solo_pool::start(JsonlStdoutSoloPoolSink, &mut st);
    if st.solo_pool_task.is_none() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
        tokio::select! {
            res = tokio::signal::ctrl_c() => res.map_err(|e| e.to_string())?,
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.map_err(|e| e.to_string())?;
    }
    solo_pool::stop(&mut st);
    Ok(())
}
