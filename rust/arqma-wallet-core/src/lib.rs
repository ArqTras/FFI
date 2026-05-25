//! Arqma wallet domain logic — shared by Tauri, daemon tooling, and any future web build.

pub mod config;
pub mod defaults;
pub mod error;
pub mod merge;
pub mod startup;
pub mod validate;

pub use config::{AppConfigSnapshot, default_paths, default_remote_nodes, ArqmaPaths, RemoteNode};
pub use defaults::{build_initial_config_data, build_defaults, default_ethereum};
pub use error::CoreError;
pub use merge::merge_json;
pub use startup::{
  config_path, ensure_datadir_layout, ensure_gui_dir, load_and_persist_remotes, load_config_snapshot,
  required_dirs_exist, remotes_path, write_config_file, StartupSnapshot,
};
