// Node discovery - simulates protocol scanning for ROS2/DDS, Zenoh, and MQTT
// endpoints on the local network. In production, this would use DDS discovery
// multicast, Zenoh scouting, and MQTT broker enumeration.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

use rud_core::{
    node::{NodeId, NodeKind, NodeStatus, RudNode},
    protocol::Protocol,
    state::{LogLevel, SharedState},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredEndpoint {
    pub protocol: Protocol,
    pub address: String,
    pub topic_or_service: String,
    pub node_type_hint: NodeKind,
}

pub struct ProtocolScanner;

impl ProtocolScanner {
    pub async fn scan_all(state: &Arc<SharedState>) -> Result<Vec<DiscoveredEndpoint>> {
        let mut found = Vec::new();

        let ros2_nodes = Self::scan_ros2().await;
        let zenoh_nodes = Self::scan_zenoh().await;
        let mqtt_nodes = Self::scan_mqtt().await;

        for ep in ros2_nodes.iter().chain(zenoh_nodes.iter()).chain(mqtt_nodes.iter()) {
            info!(
                proto = ep.protocol.to_string(),
                addr = ep.address,
                topic = ep.topic_or_service,
                "discovery: endpoint found"
            );

            let node = RudNode {
                id: NodeId::new(),
                name: format!(
                    "{}-{}",
                    ep.protocol.to_string().to_lowercase().replace('/', "-"),
                    ep.topic_or_service.trim_start_matches('/').replace('/', "-")
                ),
                kind: ep.node_type_hint.clone(),
                protocol: ep.protocol.clone(),
                status: NodeStatus::Online,
                endpoint: ep.address.clone(),
                discovered_at: Utc::now(),
                last_seen: Utc::now(),
                tags: vec![ep.topic_or_service.clone()],
            };

            state.nodes.insert(node.id.clone(), node);
            state.log(
                LogLevel::Info,
                "discovery",
                format!("found {} node at {}", ep.protocol, ep.address),
            );
        }

        found.extend(ros2_nodes);
        found.extend(zenoh_nodes);
        found.extend(mqtt_nodes);

        Ok(found)
    }

    async fn scan_ros2() -> Vec<DiscoveredEndpoint> {
        // Simulated: in production, parse DDS participant discovery data
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        vec![
            DiscoveredEndpoint {
                protocol: Protocol::Ros2,
                address: "239.255.0.1:7400".to_string(),
                topic_or_service: "/sensor/lidar".to_string(),
                node_type_hint: NodeKind::Sensor,
            },
            DiscoveredEndpoint {
                protocol: Protocol::Ros2,
                address: "239.255.0.1:7400".to_string(),
                topic_or_service: "/cmd_vel".to_string(),
                node_type_hint: NodeKind::Control,
            },
            DiscoveredEndpoint {
                protocol: Protocol::Ros2,
                address: "239.255.0.1:7401".to_string(),
                topic_or_service: "/joint_states".to_string(),
                node_type_hint: NodeKind::Sensor,
            },
        ]
    }

    async fn scan_zenoh() -> Vec<DiscoveredEndpoint> {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        vec![
            DiscoveredEndpoint {
                protocol: Protocol::Zenoh,
                address: "tcp/localhost:7447".to_string(),
                topic_or_service: "robot/inference/pose".to_string(),
                node_type_hint: NodeKind::Inference,
            },
            DiscoveredEndpoint {
                protocol: Protocol::Zenoh,
                address: "tcp/localhost:7447".to_string(),
                topic_or_service: "robot/telemetry/imu".to_string(),
                node_type_hint: NodeKind::Sensor,
            },
        ]
    }

    async fn scan_mqtt() -> Vec<DiscoveredEndpoint> {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        vec![
            DiscoveredEndpoint {
                protocol: Protocol::Mqtt,
                address: "tcp://localhost:1883".to_string(),
                topic_or_service: "robot/comms/telemetry".to_string(),
                node_type_hint: NodeKind::Comms,
            },
        ]
    }
}
