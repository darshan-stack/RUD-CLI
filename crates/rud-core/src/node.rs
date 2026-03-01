use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Sensor,
    Control,
    Inference,
    Comms,
    Bridge,
    Simulation,
    Unknown,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeKind::Sensor => "SENSOR",
            NodeKind::Control => "CONTROL",
            NodeKind::Inference => "INFER",
            NodeKind::Comms => "COMMS",
            NodeKind::Bridge => "BRIDGE",
            NodeKind::Simulation => "SIM",
            NodeKind::Unknown => "UNKNOWN",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Online,
    Degraded,
    Offline,
    Anomalous,
    Remediating,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeStatus::Online => "ONLINE",
            NodeStatus::Degraded => "DEGRADED",
            NodeStatus::Offline => "OFFLINE",
            NodeStatus::Anomalous => "ANOMALOUS",
            NodeStatus::Remediating => "REMEDIATING",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudNode {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
    pub protocol: crate::protocol::Protocol,
    pub status: NodeStatus,
    pub endpoint: String,
    pub discovered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub tags: Vec<String>,
}

impl RudNode {
    pub fn new(name: impl Into<String>, kind: NodeKind, protocol: crate::protocol::Protocol, endpoint: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: NodeId::new(),
            name: name.into(),
            kind,
            protocol,
            status: NodeStatus::Online,
            endpoint: endpoint.into(),
            discovered_at: now,
            last_seen: now,
            tags: Vec::new(),
        }
    }
}
