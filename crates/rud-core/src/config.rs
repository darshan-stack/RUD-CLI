use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Result, Context};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudConfig {
    pub daemon: DaemonConfig,
    pub aether: AetherConfig,
    pub ghost: GhostConfig,
    pub tui: TuiConfig,
    pub discovery: DiscoveryConfig,
    pub monitoring: MonitoringConfig,
    pub llm: LlmConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub pid_file: PathBuf,
    pub socket_path: PathBuf,
    pub log_level: String,
    pub log_file: PathBuf,
    pub telemetry_interval_ms: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub enable_ros2: bool,
    pub enable_zenoh: bool,
    pub enable_mqtt: bool,
    pub scan_interval_secs: u64,
    pub use_real_discovery: bool,
    pub ros_domain_id: Option<u32>,
    pub mqtt_broker: String,
    pub mqtt_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_prometheus: bool,
    pub prometheus_port: u16,
    pub enable_opentelemetry: bool,
    pub otel_endpoint: Option<String>,
    pub metrics_retention_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub enabled: bool,
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub api_url: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_tls: bool,
    pub cert_file: Option<PathBuf>,
    pub key_file: Option<PathBuf>,
    pub ca_file: Option<PathBuf>,
    pub require_auth: bool,
}

impl Default for RudConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig {
                pid_file: PathBuf::from("/tmp/rud.pid"),
                socket_path: PathBuf::from("/tmp/rud.sock"),
                log_level: "info".to_string(),
                log_file: PathBuf::from("/tmp/rud.log"),
                telemetry_interval_ms: 1000,
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
            discovery: DiscoveryConfig {
                enable_ros2: true,
                enable_zenoh: true,
                enable_mqtt: true,
                scan_interval_secs: 30,
                use_real_discovery: false, // Default to mock for compatibility
                ros_domain_id: None,
                mqtt_broker: "localhost".to_string(),
                mqtt_port: 1883,
            },
            monitoring: MonitoringConfig {
                enable_prometheus: false,
                prometheus_port: 9090,
                enable_opentelemetry: false,
                otel_endpoint: None,
                metrics_retention_secs: 3600,
            },
            llm: LlmConfig {
                enabled: false,
                provider: "openai".to_string(),
                api_key: None,
                model: "gpt-4".to_string(),
                api_url: None,
                timeout_secs: 30,
            },
            security: SecurityConfig {
                enable_tls: false,
                cert_file: None,
                key_file: None,
                ca_file: None,
                require_auth: false,
            },
        }
    }
}

impl RudConfig {
    pub fn load_or_default(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut cfg: RudConfig = toml::from_str(&content)?;
            cfg.apply_env_overrides();
            cfg.validate()?;
            Ok(cfg)
        } else {
            let mut cfg = RudConfig::default();
            cfg.apply_env_overrides();
            Ok(cfg)
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

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        if let Ok(level) = std::env::var("RUD_LOG_LEVEL") {
            self.daemon.log_level = level;
        }

        if let Ok(domain) = std::env::var("ROS_DOMAIN_ID") {
            if let Ok(id) = domain.parse() {
                self.discovery.ros_domain_id = Some(id);
            }
        }

        if let Ok(broker) = std::env::var("MQTT_BROKER") {
            self.discovery.mqtt_broker = broker;
        }

        if let Ok(port) = std::env::var("MQTT_PORT") {
            if let Ok(p) = port.parse() {
                self.discovery.mqtt_port = p;
            }
        }

        if let Ok(enabled) = std::env::var("RUD_LLM_ENABLED") {
            self.llm.enabled = enabled.to_lowercase() == "true" || enabled == "1";
        }

        if let Ok(provider) = std::env::var("RUD_LLM_PROVIDER") {
            self.llm.provider = provider;
        }

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.llm.api_key = Some(key);
        } else if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.llm.api_key = Some(key);
        }

        if let Ok(model) = std::env::var("RUD_LLM_MODEL") {
            self.llm.model = model;
        }

        if let Ok(url) = std::env::var("RUD_LLM_URL") {
            self.llm.api_url = Some(url);
        }

        if let Ok(enabled) = std::env::var("RUD_REAL_DISCOVERY") {
            self.discovery.use_real_discovery = enabled.to_lowercase() == "true" || enabled == "1";
        }
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.daemon.log_level.to_lowercase().as_str()) {
            return Err(anyhow::anyhow!("Invalid log level: {}", self.daemon.log_level));
        }

        if self.llm.enabled {
            if self.llm.provider == "openai" || self.llm.provider == "anthropic" {
                if self.llm.api_key.is_none() {
                    return Err(anyhow::anyhow!("LLM enabled but no API key provided. Set OPENAI_API_KEY or ANTHROPIC_API_KEY"));
                }
            } else if self.llm.provider == "local" {
                if self.llm.api_url.is_none() {
                    return Err(anyhow::anyhow!("Local LLM requires api_url"));
                }
            }
        }

        if self.security.enable_tls {
            if self.security.cert_file.is_none() || self.security.key_file.is_none() {
                return Err(anyhow::anyhow!("TLS enabled but cert/key files not provided"));
            }
        }

        Ok(())
    }
}

fn dirs_config() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("rud").join("config.toml")
}
