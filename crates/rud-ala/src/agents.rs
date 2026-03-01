// ALA Node Agents - Lightweight adapters that represent physical or logical
// robotics subsystems: Sensor-ALA, Control-ALA, Inference-ALA, Comms-ALA.
// Each agent runs a Tokio task, emitting heartbeats via the shared state.

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use tokio::time;
use tracing::{debug, info};

use rud_core::{
    node::{NodeId, NodeKind, RudNode},
    protocol::Protocol,
    state::{LogLevel, SharedState},
};

pub struct AlaAgent {
    pub node: RudNode,
    pub heartbeat_interval: Duration,
}

impl AlaAgent {
    pub fn new(name: impl Into<String>, kind: NodeKind, protocol: Protocol, endpoint: impl Into<String>) -> Self {
        Self {
            node: RudNode::new(name, kind, protocol, endpoint),
            heartbeat_interval: Duration::from_millis(500),
        }
    }

    pub fn with_heartbeat(mut self, interval_ms: u64) -> Self {
        self.heartbeat_interval = Duration::from_millis(interval_ms);
        self
    }

    pub fn register(&self, state: &SharedState) {
        state.nodes.insert(self.node.id.clone(), self.node.clone());
        state.log(
            LogLevel::Info,
            "ala",
            format!(
                "registered node '{}' [{}/{}] at {}",
                self.node.name, self.node.kind, self.node.protocol, self.node.endpoint
            ),
        );
        info!(
            name = self.node.name,
            kind = self.node.kind.to_string(),
            proto = self.node.protocol.to_string(),
            "ALA: node registered"
        );
    }

    // Async heartbeat loop - updates last_seen timestamp at the configured interval
    pub async fn run_heartbeat(node_id: NodeId, state: Arc<SharedState>, interval: Duration) {
        let mut ticker = time::interval(interval);
        loop {
            ticker.tick().await;
            if let Some(mut node) = state.nodes.get_mut(&node_id) {
                node.last_seen = Utc::now();
                debug!(name = node.name, "ALA: heartbeat");
            } else {
                break; // Node removed from registry
            }
        }
    }
}

// Factory helpers for standard ALA roles
pub fn sensor_ala(name: &str, endpoint: &str) -> AlaAgent {
    AlaAgent::new(name, NodeKind::Sensor, Protocol::Ros2, endpoint)
}

pub fn control_ala(name: &str, endpoint: &str) -> AlaAgent {
    AlaAgent::new(name, NodeKind::Control, Protocol::Ros2, endpoint)
}

pub fn inference_ala(name: &str, endpoint: &str) -> AlaAgent {
    AlaAgent::new(name, NodeKind::Inference, Protocol::Zenoh, endpoint)
}

pub fn comms_ala(name: &str, endpoint: &str) -> AlaAgent {
    AlaAgent::new(name, NodeKind::Comms, Protocol::Mqtt, endpoint)
}
