use thiserror::Error;

#[derive(Debug, Error)]
pub enum RudError {
    #[error("daemon error: {0}")]
    Daemon(String),

    #[error("fabric error: {0}")]
    Fabric(String),

    #[error("node error: {0}")]
    Node(String),

    #[error("protocol error: {protocol} - {msg}")]
    Protocol { protocol: String, msg: String },

    #[error("anomaly detected: {0}")]
    Anomaly(String),

    #[error("simulation error: {0}")]
    Simulation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("timeout: operation '{op}' exceeded {timeout_ms}ms")]
    Timeout { op: String, timeout_ms: u64 },

    #[error("not initialized: call `rud --init` first")]
    NotInitialized,

    #[error("already running: daemon pid {0}")]
    AlreadyRunning(u32),
}

pub type Result<T> = std::result::Result<T, RudError>;
