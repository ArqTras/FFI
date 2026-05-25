use crate::error::CoreError;
use crate::merge::merge_json;
use crate::validate::validate_config_against_defaults;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use crate::config::ArqmaPaths;
use crate::defaults::{build_defaults, build_initial_config_data, default_ethereum};

pub fn gui_subdir (config_dir: &Path) -> std::path::PathBuf {
  config_dir.join("gui")
}

pub fn remotes_path (config_dir: &Path) -> std::path::PathBuf {
  gui_subdir(config_dir).join("remotes.json")
}

pub fn config_path (config_dir: &Path) -> std::path::PathBuf {
  gui_subdir(config_dir).join("config.json")
}

/// Removes legacy `arqma-wallet-rpc` CLI flags accidentally persisted into `config.json`
/// (`--trusted-daemon` / Monero-style booleans). Tauri never passes these; keeping them only confuses merges.
fn strip_trusted_daemon_from_config (v: &mut Value) {
  match v {
    Value::Object(map) => {
      map.remove("trusted-daemon");
      map.remove("trusted_daemon");
      map.remove("trustedDaemon");
      for child in map.values_mut() {
        strip_trusted_daemon_from_config(child);
      }
    }
    Value::Array(arr) => {
      for item in arr.iter_mut() {
        strip_trusted_daemon_from_config(item);
      }
    }
    _ => {}
  }
}

pub fn ensure_gui_dir (config_dir: &Path) -> Result<(), CoreError> {
  fs::create_dir_all(gui_subdir(config_dir))?;
  Ok(())
}

fn migrate_remote_host (v: &mut Value) {
  if let Some(o) = v.as_object_mut() {
    if o.get("host").and_then(|h| h.as_str()) == Some("arq.pool.gntl.co.uk") {
      o.insert("host".into(), json!("arq.gntl.uk"));
    }
  }
}

fn default_remotes_array () -> Vec<Value> {
  (1..=5)
    .map(|n| {
      json!({
        "host": format!("node{n}.arqma.com"),
        "port": 19994
      })
    })
    .collect()
}

/// Load / persist `remotes.json` like `init` in the legacy backend.
pub fn load_and_persist_remotes (config_dir: &Path) -> Result<Value, CoreError> {
  let path = remotes_path(config_dir);
  let def_vec = default_remotes_array();
  let mut use_default = true;
  let mut arr: Vec<Value> = if path.exists() {
    let s = fs::read_to_string(&path).map_err(CoreError::Io)?;
    let v: Value = serde_json::from_str(&s)?;
    let a = v
      .as_array()
      .ok_or_else(|| CoreError::InvalidConfig("remotes.json must be a JSON array".into()))?
      .clone();
    if a.is_empty() {
      def_vec.clone()
    } else {
      use_default = false;
      a
    }
  } else {
    def_vec.clone()
  };

  for n in &mut arr {
    migrate_remote_host(n);
  }

  if !use_default {
    for d in &def_vec {
      let dh = d.get("host").and_then(|h| h.as_str());
      let dp = d.get("port").and_then(|p| p.as_u64());
      let done = arr.iter().any(|n| {
        n.get("host").and_then(|h| h.as_str()) == dh
          && n.get("port").and_then(|p| p.as_u64()) == dp
      });
      if !done {
        arr.push(d.clone());
      }
    }
  }

  let out = Value::Array(arr);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(CoreError::Io)?;
  }
  fs::write(
    &path,
    serde_json::to_string_pretty(&out).map_err(|e| CoreError::InvalidConfig(e.to_string()))?,
  )
  .map_err(CoreError::Io)?;
  Ok(out)
}

/// Fold on-disk `config.json` into `this.config_data` (forEach loop in `startup` JS).
pub fn fold_disk_into_config (config_data: &Value, disk: &Value) -> Value {
  let (Some(c), Some(d)) = (config_data.as_object(), disk.as_object()) else {
    return config_data.clone();
  };
  let mut out: serde_json::Map<String, Value> = c.clone();
  for (k, dval) in d {
    if dval.is_object() && out.get(k).and_then(|v| v.as_object()).is_some() {
      out.insert(
        k.clone(),
        merge_json(
          out.get(k).expect("k"),
          dval,
        ),
      );
    } else {
      out.insert(k.clone(), dval.clone());
    }
  }
  Value::Object(out)
}

pub struct StartupSnapshot {
  pub defaults: Value,
  /// State like `this.config_data` after disk load and validation.
  pub config_data: Value,
  pub remotes: Value,
  /// `this.ethereum` from disk (or `default_ethereum` after merge in startup).
  pub ethereum: Value,
  pub had_config_file: bool,
}

/// Read `config.json`, merge, validate — middle stage of `Backend.startup()`.
pub fn load_config_snapshot (paths: &ArqmaPaths) -> Result<StartupSnapshot, CoreError> {
  let config_dir = Path::new(&paths.config_dir);
  ensure_gui_dir(config_dir)?;
  let remotes = load_and_persist_remotes(config_dir)?;
  let defaults = build_defaults(paths);
  let initial = build_initial_config_data(paths);
  let cpath = config_path(config_dir);
  if !cpath.exists() {
    return Ok(StartupSnapshot {
      defaults: defaults.clone(),
      config_data: initial,
      remotes: remotes.clone(),
      ethereum: default_ethereum(),
      had_config_file: false,
    });
  }
  let s = fs::read_to_string(&cpath).map_err(CoreError::Io)?;
  let disk: Value = serde_json::from_str(&s).map_err(CoreError::Serde)?;
  let mut config_data = fold_disk_into_config(&initial, &disk);
  let v = validate_config_against_defaults(&config_data, &defaults);
  config_data = v;
  strip_trusted_daemon_from_config(&mut config_data);
  let ethereum = config_data
    .get("ethereum")
    .cloned()
    .unwrap_or_else(default_ethereum);
  Ok(StartupSnapshot {
    defaults,
    config_data,
    remotes,
    ethereum,
    had_config_file: true,
  })
}

/// Write current `config_data` to `config.json` (pretty-printed, like Node).
pub fn write_config_file (paths: &ArqmaPaths, config_data: &Value) -> Result<(), CoreError> {
  let p = config_path(Path::new(&paths.config_dir));
  if let Some(parent) = p.parent() {
    fs::create_dir_all(parent).map_err(CoreError::Io)?;
  }
  let mut out = config_data.clone();
  strip_trusted_daemon_from_config(&mut out);
  let s = serde_json::to_string_pretty(&out).map_err(|e| CoreError::InvalidConfig(e.to_string()))?;
  fs::write(p, s).map_err(CoreError::Io)
}

/// Create `wallet_data_dir`, `app.data_dir` / stagenet / testnet, `logs` (as in `startup` JS).
pub fn ensure_datadir_layout (config_data: &Value) -> Result<(), CoreError> {
  let (Some(app), Some(net)) = (
    config_data.get("app"),
    config_data
      .get("app")
      .and_then(|a| a.get("net_type"))
      .and_then(|n| n.as_str()),
  ) else {
    return Ok(());
  };
  let data_dir = app.get("data_dir").and_then(|d| d.as_str()).ok_or_else(|| {
    CoreError::InvalidConfig("app.data_dir".into())
  })?;
  let wdir = app
    .get("wallet_data_dir")
    .and_then(|d| d.as_str())
    .ok_or_else(|| CoreError::InvalidConfig("app.wallet_data_dir".into()))?;
  fs::create_dir_all(wdir).map_err(CoreError::Io)?;
  let net_path = match net {
    "stagenet" => Path::new(data_dir).join("stagenet"),
    "testnet" => Path::new(data_dir).join("testnet"),
    _ => Path::new(data_dir).to_path_buf(),
  };
  fs::create_dir_all(&net_path).map_err(CoreError::Io)?;
  let logs = net_path.join("logs");
  fs::create_dir_all(&logs).map_err(CoreError::Io)?;
  Ok(())
}

/// Whether required directories exist (`dirs_to_check` loop in `startup` JS).
pub fn required_dirs_exist (config_data: &Value) -> Result<(), String> {
  let app = config_data
    .get("app")
    .ok_or_else(|| "app".to_string())?;
  for (key, err) in [
    ("data_dir", "Data Storage path not found"),
    ("wallet_data_dir", "Wallet Data Storage path not found"),
  ] {
    let p = app.get(key).and_then(|d| d.as_str()).ok_or_else(|| err.to_string())?;
    if !Path::new(p).exists() {
      return Err(err.to_string());
    }
  }
  Ok(())
}
