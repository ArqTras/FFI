use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use arqma_wallet2_api::{NetworkKind, Wallet2OpenConfig, Wallet2Session};

use crate::error::WalletRpcError;
use crate::rpc_method_aliases::canonical_wallet_rpc_method;
use crate::traits::WalletJsonRpc;

/// Wallet2 `stakePending(..., amount, ...)` expects a **decimal coin string** (Arq: 9 fractional digits),
/// not raw atomic units. JSON-RPC `stake` still uses `amount` in atoms for parity with the GUI.
#[inline]
fn arq_atoms_to_stake_amount_string(amount_atoms: u64) -> String {
    const ATOMS_PER_COIN: u64 = 1_000_000_000;
    let whole = amount_atoms / ATOMS_PER_COIN;
    let frac = amount_atoms % ATOMS_PER_COIN;
    format!("{whole}.{frac:09}")
}

#[derive(Clone, Debug)]
pub struct Wallet2ApiConfig {
    pub wallet_dir: String,
    pub daemon_address: String,
    pub network: NetworkKind,
}

impl Wallet2ApiConfig {
    pub fn mainnet(wallet_dir: impl Into<String>, daemon_address: impl Into<String>) -> Self {
        Self {
            wallet_dir: wallet_dir.into(),
            daemon_address: daemon_address.into(),
            network: NetworkKind::Mainnet,
        }
    }
}

/// JSON-RPC compatibility layer over in-process `Wallet2Session` (headers + `libwallet_merged`
/// from **[arqtras/arqma](https://github.com/arqtras/arqma) `pospow`** — same `wallet2_api` surface
/// as the legacy `arqma-wallet-rpc` subprocess, without duplicating the full upstream RPC catalog).
///
/// Implemented for **Flutter / Tauri** call sites today:
/// `open_wallet`, `close_wallet` / `stop_wallet`, `create_wallet`, `restore_deterministic_wallet`,
/// `generate_from_keys`, `getheight`, `getbalance`, `get_address`, `get_transfers`, `get_address_book`,
/// `add_address_book`, `delete_address_book`, `query_key`, `set_tx_notes`, `get_transfer_by_txid`,
/// `get_address_index` (primary address only), `get_tx_notes` (empty notes; no read API in `wallet2_api`),
/// `get_payments` / `get_bulk_payments` (empty lists), `auto_refresh` / `set_daemon` / `set_log_level` /
/// `set_log_categories` / `start_mining` / `stop_mining` (no-op `{}`), `sweep_dust` / `sweep_unmixable`
/// (explicit JSON-RPC error — use subprocess RPC or GUI `sweep_all`),
/// `change_wallet_password`, `export_key_images`, `import_key_images`, `stake`, `sweep_all`,
/// `transfer` / `transfer_split`, `relay_tx`, `validate_address`, `get_accounts`, `create_address`,
/// `store`, `rescan_blockchain`, `rescan_spent`, `refresh`, `can_request_stake_unlock`,
/// `request_stake_unlock`, `register_service_node`, `incoming_transfers`, `get_version`, `get_languages`.
///
/// Notes vs upstream `arqma-wallet-rpc`:
/// - **Transfers**: `transfer_split` maps to native `createTransaction` + `exportPendingRelaySlices` /
///   `relayTxFromMetadataHex` when those symbols exist in `wallet2_api.h` (see `arqma-wallet2-api/build.rs`).
/// - **`register_service_node` / stake unlock helpers**: may return JSON-RPC `error` payloads from the
///   native stub until a `wallet2_api` hook exists; callers must check the `error` field, not assume `{}`.
/// - **`rescan_blockchain` `hard`**: GUI may send `hard: true`; `wallet2_api::Wallet` only exposes
///   `rescanBlockchain()` — the flag is accepted but not forwarded separately.
/// - **`getbalance` `per_subaddress` / `num_unspent_outputs`**: synthesized for RPC parity; `num_unspent_outputs`
///   is a conservative gate (`1` when `unlocked_balance > 0`, else `0`) because `wallet2_api` does not expose
///   per-output counts on this path. Prefer `incoming_transfers` when an accurate list is required.
/// - **Alternate method names** (e.g. `get_balance` → `getbalance`): see [`crate::rpc_method_aliases`].
#[derive(Clone)]
pub struct Wallet2ApiClient {
    cfg: Arc<Wallet2ApiConfig>,
    inner: Arc<Mutex<Option<Wallet2Session>>>,
    /// Last successful `getheight` (UI/footer); used when the session mutex is held by background work.
    height_stale_cache: Arc<AtomicU64>,
    /// Last successful `getbalance` (atomic units); same rationale as height during `rescan_*` / long refresh.
    balance_stale_cache: Arc<AtomicU64>,
    unlocked_stale_cache: Arc<AtomicU64>,
    address_stale_cache: Arc<Mutex<String>>,
    /// One in-flight background wallet job (`rescan_*`, …) — avoids overlapping native calls.
    wallet_background_busy: Arc<AtomicBool>,
}

impl Wallet2ApiClient {
    pub fn new(cfg: Wallet2ApiConfig) -> Self {
        Self {
            cfg: Arc::new(cfg),
            inner: Arc::new(Mutex::new(None)),
            height_stale_cache: Arc::new(AtomicU64::new(0)),
            balance_stale_cache: Arc::new(AtomicU64::new(0)),
            unlocked_stale_cache: Arc::new(AtomicU64::new(0)),
            address_stale_cache: Arc::new(Mutex::new(String::new())),
            wallet_background_busy: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn fork_for_heartbeat(&self) -> Self {
        self.clone()
    }

    pub fn split_session(&self) -> Self {
        self.clone()
    }

    fn getheight_nonblocking_or_stale(&self) -> Result<Value, WalletRpcError> {
        match self.inner.try_lock() {
            Ok(guard) => {
                let g = guard;
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let h = s
                    .height()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                self.height_stale_cache.store(h, Ordering::Release);
                Ok(json!({ "result": { "height": h } }))
            }
            Err(_) => {
                let h = self.height_stale_cache.load(Ordering::Acquire);
                Ok(json!({ "result": { "height": h } }))
            }
        }
    }

    fn getbalance_nonblocking_or_stale(&self) -> Result<Value, WalletRpcError> {
        match self.inner.try_lock() {
            Ok(guard) => {
                let s = guard.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let b = s
                    .balance()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                self.balance_stale_cache
                    .store(b.balance, Ordering::Release);
                self.unlocked_stale_cache
                    .store(b.unlocked_balance, Ordering::Release);
                let addr = s
                    .address()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if let Ok(mut c) = self.address_stale_cache.lock() {
                    *c = addr.clone();
                }
                // `arqma-wallet-rpc` / Electron parity: `per_subaddress` + `num_unspent_outputs` are used
                // by sweep helpers when `incoming_transfers` is unavailable. Native `wallet2_api` does not
                // expose per-output counts here; use a conservative `num_unspent_outputs` gate (see docs).
                let num_unspent_outputs = if b.unlocked_balance > 0 { 1u64 } else { 0u64 };
                Ok(json!({
                    "result": {
                        "balance": b.balance,
                        "unlocked_balance": b.unlocked_balance,
                        "multisig_import_needed": false,
                        "per_subaddress": [{
                            "account_index": 0u64,
                            "address_index": 0u64,
                            "address": addr,
                            "balance": b.balance,
                            "unlocked_balance": b.unlocked_balance,
                            "num_unspent_outputs": num_unspent_outputs,
                            "blocks_to_unlock": 0u64
                        }]
                    }
                }))
            }
            Err(_) => {
                let balance = self.balance_stale_cache.load(Ordering::Acquire);
                let unlocked = self.unlocked_stale_cache.load(Ordering::Acquire);
                let addr = match self.address_stale_cache.lock() {
                    Ok(g) => g.clone(),
                    Err(e) => e.into_inner().clone(),
                };
                let num_unspent_outputs = if unlocked > 0 { 1u64 } else { 0u64 };
                Ok(json!({
                    "result": {
                        "balance": balance,
                        "unlocked_balance": unlocked,
                        "multisig_import_needed": false,
                        "per_subaddress": [{
                            "account_index": 0u64,
                            "address_index": 0u64,
                            "address": addr,
                            "balance": balance,
                            "unlocked_balance": unlocked,
                            "num_unspent_outputs": num_unspent_outputs,
                            "blocks_to_unlock": 0u64
                        }]
                    }
                }))
            }
        }
    }

    fn get_address_nonblocking_or_stale(&self) -> Result<Value, WalletRpcError> {
        match self.inner.try_lock() {
            Ok(guard) => {
                let s = guard.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let addr = s
                    .address()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if let Ok(mut c) = self.address_stale_cache.lock() {
                    *c = addr.clone();
                }
                // Upstream `COMMAND_RPC_GET_ADDRESS` shape: deprecated top-level `address` plus
                // `addresses[]` (`address`, `label`, `address_index`, `used`) — see `pospow`
                // `wallet_rpc_server_commands_defs.h`.
                Ok(json!({
                    "result": {
                        "address": addr.clone(),
                        "addresses": [{
                            "address": addr,
                            "label": "",
                            "address_index": 0u32,
                            "used": true
                        }]
                    }
                }))
            }
            Err(_) => {
                let addr = match self.address_stale_cache.lock() {
                    Ok(g) => g.clone(),
                    Err(e) => e.into_inner().clone(),
                };
                Ok(json!({
                    "result": {
                        "address": addr.clone(),
                        "addresses": [{
                            "address": addr,
                            "label": "",
                            "address_index": 0u32,
                            "used": true
                        }]
                    }
                }))
            }
        }
    }

    /// When `rescan_*` holds `inner`, a blocking `get_transfers` would freeze the Flutter isolate (FFI `block_on`).
    fn get_transfers_nonblocking_or_empty(&self, _params: &Value) -> Result<Value, WalletRpcError> {
        let in_flag = _params.get("in").and_then(|v| v.as_bool()).unwrap_or(false);
        let out_flag = _params
            .get("out")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let pending_flag = _params
            .get("pending")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let failed_flag = _params
            .get("failed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let pool_flag = _params
            .get("pool")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let min_height = _params
            .get("min_height")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let max_height = _params
            .get("max_height")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::MAX);

        let g = match self.inner.try_lock() {
            Ok(g) => g,
            Err(_) => {
                let empty: Vec<Value> = Vec::new();
                let mut m = serde_json::Map::new();
                for k in [
                    "in", "out", "pending", "failed", "pool", "miner", "snode", "gov", "stake",
                    "net",
                ] {
                    m.insert(k.to_string(), json!(empty));
                }
                return Ok(json!({ "result": Value::Object(m) }));
            }
        };

        let s = g.as_ref().ok_or_else(|| {
            WalletRpcError::Transport("wallet2: no wallet session".to_string())
        })?;
        let raw = s
            .get_transfers_json(
                in_flag,
                out_flag,
                pending_flag,
                failed_flag,
                pool_flag,
                min_height,
                max_height,
            )
            .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
        let parsed: Value = serde_json::from_str(&raw)
            .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
        Ok(json!({ "result": parsed }))
    }

    fn spawn_wallet_background_job(
        &self,
        label: &'static str,
        job: impl FnOnce(&mut Wallet2Session) -> Result<(), WalletRpcError> + Send + 'static,
    ) -> Result<Value, WalletRpcError> {
        if self.wallet_background_busy.swap(true, Ordering::SeqCst) {
            return Err(WalletRpcError::Transport(format!(
                "wallet2: background operation already running ({label})"
            )));
        }
        let inner = self.inner.clone();
        let busy = self.wallet_background_busy.clone();
        thread::spawn(move || {
            struct BusyGuard(Arc<AtomicBool>);
            impl Drop for BusyGuard {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _guard = BusyGuard(busy);
            let run = || -> Result<(), WalletRpcError> {
                let mut g = inner
                    .lock()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let Some(s) = g.as_mut() else {
                    return Err(WalletRpcError::Transport(
                        "wallet2: no wallet session".to_string(),
                    ));
                };
                job(s)
            };
            if let Err(e) = run() {
                eprintln!("[wallet2] background {label}: {e}");
            }
        });
        Ok(json!({
          "result": {
            "async": true,
            "background": label,
          }
        }))
    }

    pub async fn call_json(&self, method: &str, _params: &Value) -> Result<Value, WalletRpcError> {
        let method = canonical_wallet_rpc_method(method);
        match method {
            "rescan_blockchain" => {
                // Do **not** zero `height_stale_cache` here: `rescan_blockchain` holds `inner` for the whole job,
                // so `getheight` only sees the stale path — a forced `0` makes the GUI stuck at “0 / tip” until done.
                return self.spawn_wallet_background_job("rescan_blockchain", |s| {
                    let ok = s
                        .rescan_blockchain()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                    if !ok {
                        return Err(WalletRpcError::Transport(
                            "wallet2: rescan_blockchain unsupported".to_string(),
                        ));
                    }
                    Ok(())
                });
            }
            "rescan_spent" => {
                return self.spawn_wallet_background_job("rescan_spent", |s| {
                    let ok = s
                        .rescan_spent()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                    if !ok {
                        return Err(WalletRpcError::Transport(
                            "wallet2: rescan_spent unsupported".to_string(),
                        ));
                    }
                    Ok(())
                });
            }
            "refresh" => {
                let start_height = _params
                    .get("start_height")
                    .or_else(|| _params.get("refresh_start_height"))
                    .and_then(|v| v.as_u64());
                if let Some(h) = start_height {
                    return self.spawn_wallet_background_job("refresh_from_height", move |s| {
                        let ok = s
                            .refresh_from_height(h)
                            .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                        if !ok {
                            // After pauseRefresh+refresh in C++, false usually means daemon not ready.
                            eprintln!(
                                "[wallet2] refresh_from_height({h}) returned false (daemon busy or refresh skipped)"
                            );
                        }
                        Ok(())
                    });
                }
                return self.spawn_wallet_background_job("refresh", |s| {
                    let ok = s
                        .refresh()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                    if !ok {
                        // Often means background startRefresh already holds the refresh mutex.
                        return Ok(());
                    }
                    Ok(())
                });
            }
            "getheight" => {
                return self.getheight_nonblocking_or_stale();
            }
            "getbalance" => {
                return self.getbalance_nonblocking_or_stale();
            }
            "get_address" => {
                return self.get_address_nonblocking_or_stale();
            }
            "get_transfers" => {
                return self.get_transfers_nonblocking_or_empty(_params);
            }
            _ => {}
        }

        let mut g = self
            .inner
            .lock()
            .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
        match method {
            "open_wallet" => {
                let filename = _params
                    .get("filename")
                    .or_else(|| _params.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 open_wallet: missing filename".to_string(),
                        )
                    })?;
                let password = _params
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 open_wallet: missing password".to_string(),
                        )
                    })?;
                let path = resolve_wallet_path(&self.cfg.wallet_dir, filename);
                let session = Wallet2Session::open(&Wallet2OpenConfig {
                    wallet_path: path,
                    password: password.to_string(),
                    daemon_address: self.cfg.daemon_address.clone(),
                    network: self.cfg.network.clone(),
                })
                .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                *g = Some(session);
                Ok(json!({ "result": {} }))
            }
            "create_wallet" => {
                let filename = _params
                    .get("filename")
                    .or_else(|| _params.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 create_wallet: missing filename".to_string(),
                        )
                    })?;
                let password = _params
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 create_wallet: missing password".to_string(),
                        )
                    })?;
                let language = _params
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("English");
                let path = resolve_wallet_path(&self.cfg.wallet_dir, filename);
                // Wallet does not exist on disk yet — use a manager-only "bare" session so we
                // never call `openWallet` against the future path (would create a stale, empty
                // cache file and later fail `recoveryWallet` with "file already exists").
                let mut session = match g.take() {
                    Some(s) => s,
                    None => Wallet2Session::bare()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?,
                };
                session
                    .create_wallet(
                        &path,
                        password,
                        language,
                        &self.cfg.network,
                        &self.cfg.daemon_address,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                *g = Some(session);
                Ok(json!({ "result": {} }))
            }
            "restore_deterministic_wallet" => {
                let filename = _params
                    .get("filename")
                    .or_else(|| _params.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 restore_deterministic_wallet: missing filename".to_string(),
                        )
                    })?;
                let password = _params
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 restore_deterministic_wallet: missing password".to_string(),
                        )
                    })?;
                let seed = _params
                    .get("seed")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 restore_deterministic_wallet: missing seed".to_string(),
                        )
                    })?;
                let restore_height = _params
                    .get("restore_height")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let path = resolve_wallet_path(&self.cfg.wallet_dir, filename);
                // Same reasoning as `create_wallet`: never call `openWallet` for a wallet that
                // does not yet exist on disk — fresh manager-only bridge avoids stale cache files.
                let mut session = match g.take() {
                    Some(s) => s,
                    None => Wallet2Session::bare()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?,
                };
                session
                    .restore_deterministic_wallet(
                        &path,
                        password,
                        seed,
                        restore_height,
                        &self.cfg.network,
                        &self.cfg.daemon_address,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                *g = Some(session);
                Ok(json!({ "result": {} }))
            }
            "generate_from_keys" => {
                let filename = _params
                    .get("filename")
                    .or_else(|| _params.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 generate_from_keys: missing filename".to_string(),
                        )
                    })?;
                let password = _params
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 generate_from_keys: missing password".to_string(),
                        )
                    })?;
                let address = _params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 generate_from_keys: missing address".to_string(),
                        )
                    })?;
                let view_key = _params
                    .get("viewkey")
                    .or_else(|| _params.get("view_key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 generate_from_keys: missing viewkey".to_string(),
                        )
                    })?;
                let spend_key = _params
                    .get("spendkey")
                    .or_else(|| _params.get("spend_key"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let restore_height = _params
                    .get("restore_height")
                    .or_else(|| _params.get("refresh_start_height"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let language = _params
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("English");
                let path = resolve_wallet_path(&self.cfg.wallet_dir, filename);
                // Same reasoning as `create_wallet` / `restore_deterministic_wallet`.
                let mut session = match g.take() {
                    Some(s) => s,
                    None => Wallet2Session::bare()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?,
                };
                session
                    .generate_from_keys(
                        &path,
                        password,
                        language,
                        restore_height,
                        address,
                        view_key,
                        spend_key,
                        &self.cfg.network,
                        &self.cfg.daemon_address,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                *g = Some(session);
                Ok(json!({ "result": {} }))
            }
            "query_key" => {
                let key_type_raw = _params
                    .get("key_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let key_type = match key_type_raw {
                    "seed" => "mnemonic",
                    other => other,
                };
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let key = match key_type {
                    "mnemonic" => s.seed(),
                    "spend_key" => s.secret_spend_key(),
                    "view_key" => s.secret_view_key(),
                    other => {
                        return Ok(json!({
                          "error": {
                            "code": -32601,
                            "message": format!("query_key: unsupported key_type `{other}`")
                          }
                        }));
                    }
                }
                .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": { "key": key } }))
            }
            "get_address_book" => {
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .get_address_book_json()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .unwrap_or_else(|_| json!({ "entries": [] }));
                Ok(json!({ "result": parsed }))
            }
            "add_address_book" => {
                let address = _params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 add_address_book: missing address".to_string(),
                        )
                    })?;
                let payment_id = _params
                    .get("payment_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let description = _params
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let ok = s
                    .add_address_book(address, payment_id, description)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if ok {
                    Ok(json!({ "result": {} }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "delete_address_book" => {
                let idx = _params
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 delete_address_book: missing index".to_string(),
                        )
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let ok = s
                    .delete_address_book(idx)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if ok {
                    Ok(json!({ "result": {} }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "set_tx_notes" => {
                let txid = _params
                    .get("txids")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport("wallet2 set_tx_notes: missing txid".to_string())
                    })?;
                let note = _params
                    .get("notes")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let ok = s
                    .set_tx_note(txid, note)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if ok {
                    Ok(json!({ "result": {} }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "get_address_index" => {
                let addr = _params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 get_address_index: missing address".to_string(),
                        )
                    })?;
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let primary = s
                    .address()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if primary != addr {
                    return Ok(json!({
                        "error": {
                            "code": -2,
                            "message": "Requested address not found in this wallet"
                        }
                    }));
                }
                Ok(json!({ "result": { "index": { "major": 0u32, "minor": 0u32 } } }))
            }
            "get_tx_notes" => {
                let txids = _params
                    .get("txids")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 get_tx_notes: missing txids".to_string(),
                        )
                    })?;
                let notes: Vec<String> = txids.iter().map(|_| String::new()).collect();
                Ok(json!({ "result": { "notes": notes } }))
            }
            "get_payments" => Ok(json!({ "result": { "payments": [] }})),
            "get_bulk_payments" => Ok(json!({ "result": { "payments": [] }})),
            "auto_refresh" | "set_daemon" | "set_log_level" | "set_log_categories" | "start_mining"
            | "stop_mining" => Ok(json!({ "result": {} })),
            "sweep_dust" | "sweep_unmixable" => Ok(json!({
                "error": {
                    "code": -32601,
                    "message": "sweep_dust / sweep_unmixable are not implemented on the native wallet2 bridge; use `arqma-wallet-rpc` subprocess mode or `sweep_all` from the GUI."
                }
            })),
            "get_transfer_by_txid" => {
                let txid = _params
                    .get("txid")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 get_transfer_by_txid: missing txid".to_string(),
                        )
                    })?;
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .get_transfer_by_txid_json(txid)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let transfer = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}));
                Ok(json!({ "result": { "transfer": transfer } }))
            }
            "change_wallet_password" => {
                let new_password = _params
                    .get("new_password")
                    .or_else(|| _params.get("password"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 change_wallet_password: missing new_password (or password)"
                                .to_string(),
                        )
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let ok = s
                    .set_password(new_password)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if ok {
                    Ok(json!({ "result": {} }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "export_key_images" => {
                let filename = _params
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("key_image_export");
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let ok = s
                    .export_key_images(filename)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if ok {
                    Ok(json!({ "result": { "signed_key_images": [] } }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "import_key_images" => {
                let signed = _params.get("signed_key_images").ok_or_else(|| {
                    WalletRpcError::Transport(
                        "wallet2 import_key_images: missing signed_key_images".to_string(),
                    )
                })?;
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let tmp_file =
                    std::env::temp_dir().join(format!("arqma-wallet2-keyimages-{now}.json"));
                let body = serde_json::to_string(signed)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                fs::write(&tmp_file, body).map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let import_res = s
                    .import_key_images(&tmp_file.to_string_lossy())
                    .map_err(|e| WalletRpcError::Transport(e.to_string()));
                let _ = fs::remove_file(&tmp_file);
                let ok = import_res?;
                if ok {
                    Ok(json!({ "result": {} }))
                } else {
                    Ok(unsupported(method))
                }
            }
            "stake" => {
                let service_node_key = _params
                    .get("service_node_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 stake: missing service_node_key".to_string(),
                        )
                    })?;
                let amount_atoms = _params
                    .get("amount")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_i64().filter(|i| *i >= 0).map(|i| i as u64))
                    })
                    .ok_or_else(|| {
                        WalletRpcError::Transport("wallet2 stake: missing amount".to_string())
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let amount_str = arq_atoms_to_stake_amount_string(amount_atoms);
                let raw = s
                    .stake_prepare_json(service_node_key, &amount_str)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "sweep_all" => {
                let address = _params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport("wallet2 sweep_all: missing address".to_string())
                    })?;
                let do_not_relay = _params
                    .get("do_not_relay")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .sweep_all_prepare_json(address, do_not_relay)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "relay_tx" => {
                let hex = _params
                    .get("hex")
                    .or_else(|| _params.get("tx_metadata"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        WalletRpcError::Transport("wallet2 relay_tx: missing hex".to_string())
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .relay_tx_json(hex)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "can_request_stake_unlock" => {
                let service_node_key = _params
                    .get("service_node_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 can_request_stake_unlock: missing service_node_key"
                                .to_string(),
                        )
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .can_request_stake_unlock_json(service_node_key)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "request_stake_unlock" => {
                let service_node_key = _params
                    .get("service_node_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 request_stake_unlock: missing service_node_key".to_string(),
                        )
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .request_stake_unlock_json(service_node_key)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "get_accounts" => {
                let account_tag = _params
                    .get("tag")
                    .or_else(|| _params.get("account_tag"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .get_accounts_json(account_tag)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "create_address" => {
                let account_index = _params
                    .get("account_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let label = _params.get("label").and_then(|v| v.as_str()).unwrap_or("");
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .create_address_json(account_index, label)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "validate_address" => {
                let address = _params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 validate_address: missing address".to_string(),
                        )
                    })?;
                let any_net_type = _params
                    .get("any_net_type")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let allow_openalias = _params
                    .get("allow_openalias")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .validate_address_json(address, any_net_type, allow_openalias)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "register_service_node" => {
                let register_service_node_str = _params
                    .get("register_service_node_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 register_service_node: missing register_service_node_str"
                                .to_string(),
                        )
                    })?;
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .register_service_node_json(register_service_node_str)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed: Value = serde_json::from_str(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                if parsed.get("error").is_some() {
                    return Ok(parsed);
                }
                Ok(json!({ "result": parsed }))
            }
            "transfer_split" => {
                let dst = _params
                    .get("destinations")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 transfer_split: missing destinations".to_string(),
                        )
                    })?;
                let address = dst.get("address").and_then(|v| v.as_str()).ok_or_else(|| {
                    WalletRpcError::Transport(
                        "wallet2 transfer_split: missing destination.address".to_string(),
                    )
                })?;
                let amount = dst
                    .get("amount")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_i64().filter(|i| *i >= 0).map(|i| i as u64))
                    })
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 transfer_split: missing destination.amount".to_string(),
                        )
                    })?;
                let priority = _params
                    .get("priority")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_i64().filter(|i| *i >= 0).map(|i| i as u64))
                    })
                    .unwrap_or(0) as u32;
                let payment_id = _params
                    .get("payment_id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("");
                let do_not_relay = _params
                    .get("do_not_relay")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .transfer_split_prepare_json(
                        address,
                        payment_id,
                        amount,
                        priority,
                        do_not_relay,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "transfer" => {
                // Compatibility alias: UI and legacy code paths may still call `transfer`.
                let dst = _params
                    .get("destinations")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 transfer: missing destinations".to_string(),
                        )
                    })?;
                let address = dst.get("address").and_then(|v| v.as_str()).ok_or_else(|| {
                    WalletRpcError::Transport(
                        "wallet2 transfer: missing destination.address".to_string(),
                    )
                })?;
                let amount = dst
                    .get("amount")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_i64().filter(|i| *i >= 0).map(|i| i as u64))
                    })
                    .ok_or_else(|| {
                        WalletRpcError::Transport(
                            "wallet2 transfer: missing destination.amount".to_string(),
                        )
                    })?;
                let priority = _params
                    .get("priority")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_i64().filter(|i| *i >= 0).map(|i| i as u64))
                    })
                    .unwrap_or(0) as u32;
                let payment_id = _params
                    .get("payment_id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("");
                let do_not_relay = _params
                    .get("do_not_relay")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .transfer_split_prepare_json(
                        address,
                        payment_id,
                        amount,
                        priority,
                        do_not_relay,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed = serde_json::from_str::<Value>(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": parsed }))
            }
            "incoming_transfers" => {
                let transfer_type = _params
                    .get("transfer_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("all");
                let s = g.as_ref().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                let raw = s
                    .get_transfers_json(
                        true,
                        false,
                        false,
                        false,
                        false,
                        0,
                        u64::MAX,
                    )
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let parsed: Value = serde_json::from_str(&raw)
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                let empty: Vec<Value> = Vec::new();
                let in_rows = parsed
                    .get("in")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or(empty);
                let transfers: Vec<Value> = match transfer_type {
                    "unavailable" => Vec::new(),
                    _ => in_rows,
                };
                Ok(json!({ "result": { "transfers": transfers } }))
            }
            "get_version" => Ok(json!({
                "result": {
                    "version": 0,
                    "release": true,
                    "tag": "native-wallet2-ffi",
                    "bridge_crate": env!("CARGO_PKG_NAME"),
                    "bridge_version": env!("CARGO_PKG_VERSION"),
                }
            })),
            "get_languages" => Ok(json!({ "result": { "languages": ["English"] } })),
            "store" => {
                let s = g.as_mut().ok_or_else(|| {
                    WalletRpcError::Transport("wallet2: no wallet session".to_string())
                })?;
                s.store()
                    .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                Ok(json!({ "result": {} }))
            }
            "close_wallet" | "stop_wallet" => {
                if let Some(s) = g.as_mut() {
                    s.close()
                        .map_err(|e| WalletRpcError::Transport(e.to_string()))?;
                }
                *g = None;
                Ok(json!({ "result": {} }))
            }
            _ => Ok(unsupported(method)),
        }
    }
}

fn resolve_wallet_path(wallet_dir: &str, filename: &str) -> String {
    let p = Path::new(filename);
    if p.is_absolute() {
        return filename.to_string();
    }
    PathBuf::from(wallet_dir)
        .join(filename)
        .to_string_lossy()
        .to_string()
}

fn unsupported(method: &str) -> Value {
    json!({
      "error": {
        "code": -32601,
        "message": format!(
            "Method not found or not implemented on native wallet2 bridge: `{method}`. \
For the full upstream `arqma-wallet-rpc` command set, run the wallet RPC subprocess (HTTP digest) mode."
        )
      }
    })
}

#[async_trait]
impl WalletJsonRpc for Wallet2ApiClient {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, WalletRpcError> {
        self.call_json(method, params).await
    }
}
