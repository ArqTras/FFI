use thiserror::Error;

/// Core domain errors (I/O, configuration, validation). RPC uses its own codes; this is the core layer only.
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}
