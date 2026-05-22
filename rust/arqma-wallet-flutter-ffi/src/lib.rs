//! C ABI for Flutter: [`arqma_wallet_rpc::Wallet2ApiClient`] in-process (same stack as Tauri `WalletBackendMode::Wallet2`).
//!
//! Build (requires Arqma upstream + `libwallet_merged` per `rust/docs/NATIVE_WALLET2.md`):
//! `cargo build -p arqma-wallet-flutter-ffi --release`
//!
//! Dart loads the `cdylib` and calls [`arqma_wallet_ffi_configure`] then [`arqma_wallet_ffi_call_json`].

use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

use arqma_wallet_rpc::{NetworkKind, Wallet2ApiClient, Wallet2ApiConfig};
use serde_json::{json, Value};

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static CLIENT: Mutex<Option<Wallet2ApiClient>> = Mutex::new(None);

fn runtime () -> &'static tokio::runtime::Runtime {
  RUNTIME.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .worker_threads(2)
      .build()
      .expect("arqma_wallet_flutter_ffi: tokio Runtime::new")
  })
}

fn network_from_i32 (n: c_int) -> NetworkKind {
  match n {
    1 => NetworkKind::Testnet,
    2 => NetworkKind::Stagenet,
    _ => NetworkKind::Mainnet,
  }
}

fn value_to_cstring (v: &Value) -> Result<CString, ()> {
  let s = serde_json::to_string(v).map_err(|_| ())?;
  CString::new(s).map_err(|_| ())
}

/// `0` = success, negative = error (caller must not use `out_json`).
#[no_mangle]
pub unsafe extern "C" fn arqma_wallet_ffi_configure (
  wallet_dir: *const c_char,
  daemon_address: *const c_char,
  network: c_int,
) -> c_int {
  if wallet_dir.is_null() || daemon_address.is_null() {
    return -1;
  }
  let wd = match CStr::from_ptr(wallet_dir).to_str() {
    Ok(s) => s.to_string(),
    Err(_) => return -2,
  };
  let da = match CStr::from_ptr(daemon_address).to_str() {
    Ok(s) => s.to_string(),
    Err(_) => return -2,
  };
  let Ok(mut g) = CLIENT.lock() else {
    return -3;
  };
  let cfg = Wallet2ApiConfig {
    wallet_dir: wd,
    daemon_address: da,
    network: network_from_i32(network),
  };
  *g = Some(Wallet2ApiClient::new(cfg));
  0
}

/// Drop the native client (optional `close_wallet` should be done from Dart via [`arqma_wallet_ffi_call_json`] first).
#[no_mangle]
pub unsafe extern "C" fn arqma_wallet_ffi_reset () -> c_int {
  let Ok(mut g) = CLIENT.lock() else {
    return -3;
  };
  *g = None;
  0
}

/// Run one JSON-RPC-shaped wallet call (same methods as `Wallet2ApiClient::call_json`).
///
/// On success returns `0` and sets `*out_json` to a NUL-terminated UTF-8 JSON document (Dart must free with [`arqma_wallet_ffi_string_free`]).
/// On failure returns negative code and leaves `*out_json` unchanged.
#[no_mangle]
pub unsafe extern "C" fn arqma_wallet_ffi_call_json (
  method: *const c_char,
  params_json: *const c_char,
  out_json: *mut *mut c_char,
) -> c_int {
  if method.is_null() || params_json.is_null() || out_json.is_null() {
    return -1;
  }
  let method_s = match CStr::from_ptr(method).to_str() {
    Ok(s) => s,
    Err(_) => return -2,
  };
  let params_s = match CStr::from_ptr(params_json).to_str() {
    Ok(s) => s,
    Err(_) => return -2,
  };
  let params: Value = match serde_json::from_str(params_s) {
    Ok(v) => v,
    Err(_) => Value::Null,
  };
  let client = match CLIENT.lock() {
    Ok(g) => g,
    Err(_) => return -3,
  };
  let Some(c) = client.as_ref().cloned() else {
    return -4;
  };
  // Do not hold `CLIENT` across `block_on`: a long `close_wallet` / scan would block
  // `arqma_wallet_ffi_reset` on another thread (Dart watchdog isolate) and freeze shutdown.
  drop(client);

  let result = catch_unwind(AssertUnwindSafe(|| {
    runtime().block_on(c.call_json(method_s, &params))
  }));
  let mapped: Value = match result {
    Ok(Ok(v)) => v,
    Ok(Err(e)) => json!({
      "error": {
        "code": -32603,
        "message": e.to_string()
      }
    }),
    Err(_) => json!({
      "error": {
        "code": -32603,
        "message": "arqma_wallet_ffi_call_json: Rust panic"
      }
    }),
  };

  let Ok(cs) = value_to_cstring(&mapped) else {
    return -5;
  };
  *out_json = cs.into_raw();
  0
}

#[no_mangle]
pub unsafe extern "C" fn arqma_wallet_ffi_string_free (p: *mut c_char) {
  if p.is_null() {
    return;
  }
  let _ = CString::from_raw(p);
}
