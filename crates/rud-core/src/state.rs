use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    node::{NodeId, RudNode},
    metrics::MetricWindow,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonState {
    Uninitialized,
    Initializing,
    Running,
    Degraded,
    ShuttingDown,
}

impl std::fmt::Display for DaemonState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonState::Uninitialized => write!(f, "UNINITIALIZED"),
            DaemonState::Initializing => write!(f, "INITIALIZING"),
            DaemonState::Running => write!(f, "RUNNING"),
            DaemonState::Degraded => write!(f, "DEGRADED"),
            DaemonState::ShuttingDown => write!(f, "SHUTTING_DOWN"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub id: uuid::Uuid,
    pub node_id: NodeId,
    pub node_name: String,
    pub kind: AnomalyKind,
    pub severity: Severity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub z_score: Option<f64>,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyKind {
    LatencySpike,
    CpuSurge,
    MemoryLeak,
    MessageDrops,
    NodeOffline,
    ProtocolError,
}

impl std::fmt::Display for AnomalyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalyKind::LatencySpike => write!(f, "LATENCY_SPIKE"),
            AnomalyKind::CpuSurge => write!(f, "CPU_SURGE"),
            AnomalyKind::MemoryLeak => write!(f, "MEMORY_LEAK"),
            AnomalyKind::MessageDrops => write!(f, "MSG_DROPS"),
            AnomalyKind::NodeOffline => write!(f, "NODE_OFFLINE"),
            AnomalyKind::ProtocolError => write!(f, "PROTO_ERROR"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Low => write!(f, "LOW"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::High => write!(f, "HIGH"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

pub struct SharedState {
    pub daemon_state: RwLock<DaemonState>,
    pub nodes: DashMap<NodeId, RudNode>,
    pub metrics: DashMap<NodeId, MetricWindow>,
    pub anomalies: RwLock<Vec<AnomalyEvent>>,
    pub logs: RwLock<std::collections::VecDeque<LogEntry>>,
    pub started_at: DateTime<Utc>,
    pub qds_shards_allocated: RwLock<usize>,
    pub log_capacity: usize,
}

impl SharedState {
    pub fn new(log_capacity: usize, metric_history: usize) -> Arc<Self> {
        let _ = metric_history;
        Arc::new(Self {
            daemon_state: RwLock::new(DaemonState::Uninitialized),
            nodes: DashMap::new(),
            metrics: DashMap::new(),
            anomalies: RwLock::new(Vec::new()),
            logs: RwLock::new(std::collections::VecDeque::with_capacity(log_capacity)),
            started_at: Utc::now(),
            qds_shards_allocated: RwLock::new(0),
            log_capacity,
        })
    }

    pub fn push_log(&self, entry: LogEntry) {
        let mut logs = self.logs.write();
        if logs.len() >= self.log_capacity {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    pub fn push_anomaly(&self, event: AnomalyEvent) {
        let mut anomalies = self.anomalies.write();
        anomalies.push(event);
    }

    pub fn log(&self, level: LogLevel, source: impl Into<String>, message: impl Into<String>) {
        self.push_log(LogEntry {
            timestamp: Utc::now(),
            level,
            source: source.into(),
            message: message.into(),
        });
    }
}
