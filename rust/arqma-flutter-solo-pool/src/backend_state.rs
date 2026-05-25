use arqma_wallet_core::ArqmaPaths;
use serde_json::Value;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Minimal backend state for the Flutter solo-pool sidecar (config + pool task handles).
pub struct WalletBackendState {
    pub paths: ArqmaPaths,
    pub config_data: Value,
    pub solo_pool_task: Option<JoinHandle<()>>,
    pub solo_pool_shutdown: Option<oneshot::Sender<()>>,
}

impl Default for WalletBackendState {
    fn default() -> Self {
        let paths = arqma_wallet_core::default_paths();
        Self {
            config_data: arqma_wallet_core::build_initial_config_data(&paths),
            paths,
            solo_pool_task: None,
            solo_pool_shutdown: None,
        }
    }
}
