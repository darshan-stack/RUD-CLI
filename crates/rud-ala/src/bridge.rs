// Protocol Bridge - Semantic auto-mapping between heterogeneous protocols.
// Maps ROS2 topics to Zenoh key expressions and MQTT topics using a
// configurable translation table with optional field-level transforms.

use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use rud_core::{
    protocol::{BridgeMapping, Protocol, TopicMapping},
    state::{LogLevel, SharedState},
};

pub struct ProtocolBridge {
    pub mapping: BridgeMapping,
    active: bool,
}

impl ProtocolBridge {
    pub fn new(source: Protocol, target: Protocol, semantic_auto: bool) -> Self {
        let topic_map = if semantic_auto {
            Self::auto_map(&source, &target)
        } else {
            Vec::new()
        };

        Self {
            mapping: BridgeMapping {
                source,
                target,
                topic_map,
                semantic_auto,
            },
            active: false,
        }
    }

    fn auto_map(source: &Protocol, target: &Protocol) -> Vec<TopicMapping> {
        // Semantic auto-mapping rules between known protocol namespaces
        match (source, target) {
            (Protocol::Ros2, Protocol::Zenoh) => vec![
                TopicMapping {
                    source_topic: "/sensor/lidar".to_string(),
                    target_topic: "robot/sensor/lidar".to_string(),
                    transform: None,
                },
                TopicMapping {
                    source_topic: "/cmd_vel".to_string(),
                    target_topic: "robot/control/cmd_vel".to_string(),
                    transform: Some("ros2_twist_to_zenoh_velocity".to_string()),
                },
                TopicMapping {
                    source_topic: "/joint_states".to_string(),
                    target_topic: "robot/sensor/joints".to_string(),
                    transform: None,
                },
            ],
            (Protocol::Zenoh, Protocol::Mqtt) => vec![
                TopicMapping {
                    source_topic: "robot/telemetry/*".to_string(),
                    target_topic: "rud/telemetry".to_string(),
                    transform: Some("zenoh_to_mqtt_json".to_string()),
                },
            ],
            (Protocol::Ros2, Protocol::Mqtt) => vec![
                TopicMapping {
                    source_topic: "/sensor/*".to_string(),
                    target_topic: "rud/ros2/sensor".to_string(),
                    transform: Some("ros2_to_mqtt_json".to_string()),
                },
            ],
            _ => Vec::new(),
        }
    }

    pub fn activate(&mut self, state: &Arc<SharedState>) -> Result<()> {
        let count = self.mapping.topic_map.len();
        info!(
            source = self.mapping.source.to_string(),
            target = self.mapping.target.to_string(),
            topic_count = count,
            semantic_auto = self.mapping.semantic_auto,
            "Bridge: activating"
        );

        for mapping in &self.mapping.topic_map {
            info!(
                src = mapping.source_topic,
                dst = mapping.target_topic,
                transform = mapping.transform.as_deref().unwrap_or("none"),
                "Bridge: topic mapped"
            );
            state.log(
                LogLevel::Info,
                "bridge",
                format!(
                    "{} -> {}: {} => {}",
                    self.mapping.source, self.mapping.target,
                    mapping.source_topic, mapping.target_topic
                ),
            );
        }

        self.active = true;
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}
