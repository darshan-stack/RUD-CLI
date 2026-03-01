use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudConfig {
    pub daemon: DaemonConfig,
    pub aether: AetherConfig,
    pub ghost: GhostConfig,
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub pid_file: PathBuf,
    pub socket_path: PathBuf,
    pub log_level: String,
    pub log_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetherConfig {
    pub qds_shard_count: usize,
    pub qds_shard_size_mb: usize,
    pub qds_path: PathBuf,
    pub nre_worker_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostConfig {
    pub cad_window_size: usize,
    pub cad_z_score_threshold: f64,
    pub cad_learning_period_secs: u64,
    pub esm_tick_ms: u64,
    pub nexus_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    pub refresh_rate_ms: u64,
    pub max_log_lines: usize,
    pub max_metric_history: usize,
}

impl Default for RudConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig {
                pid_file: PathBuf::from("/tmp/rud.pid"),
                socket_path: PathBuf::from("/tmp/rud.sock"),
                log_level: "info".to_string(),
                log_file: PathBuf::from("/tmp/rud.log"),
            },
            aether: AetherConfig {
                qds_shard_count: 8,
                qds_shard_size_mb: 64,
                qds_path: PathBuf::from("/tmp/rud_qds"),
                nre_worker_threads: 4,
            },
            ghost: GhostConfig {
                cad_window_size: 100,
                cad_z_score_threshold: 3.0,
                cad_learning_period_secs: 30,
                esm_tick_ms: 50,
                nexus_enabled: true,
            },
            tui: TuiConfig {
                refresh_rate_ms: 100,
                max_log_lines: 1000,
                max_metric_history: 300,
            },
        }
    }
}

impl RudConfig {
    pub fn load_or_default(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let cfg: RudConfig = toml::from_str(&content)?;
            Ok(cfg)
        } else {
            Ok(RudConfig::default())
        }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn default_path() -> PathBuf {
        dirs_config()
    }
}

fn dirs_config() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("rud").join("config.toml")
}
