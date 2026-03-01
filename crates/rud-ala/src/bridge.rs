// Protocol Bridge - Semantic auto-mapping between heterogeneous protocols.
// Maps ROS2 topics to Zenoh key expressions and MQTT topics with real
// data transformation capabilities.

use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use tracing::{info, error};

use rud_core::{
    protocol::{BridgeMapping, Protocol, TopicMapping},
    state::{LogLevel, SharedState},
};

use crate::transform;

pub struct ProtocolBridge {
    pub mapping: BridgeMapping,
    active: bool,
    runtime_handle: Option<tokio::task::JoinHandle<()>>,
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
            runtime_handle: None,
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

    /// Transform a message from source to target protocol
    pub fn transform_message(&self, source_topic: &str, data: &[u8]) -> Result<Option<(String, Bytes)>> {
        // Find matching topic mapping
        let mapping = self.mapping.topic_map.iter()
            .find(|m| {
                // Support wildcard matching
                if m.source_topic.ends_with("/*") {
                    let prefix = m.source_topic.trim_end_matches("/*");
                    source_topic.starts_with(prefix)
                } else {
                    m.source_topic == source_topic
                }
            });

        if let Some(mapping) = mapping {
            let transformed_data = if let Some(ref transform_name) = mapping.transform {
                transform::apply_transform(transform_name, data)?
            } else {
                Bytes::from(data.to_vec())
            };

            Ok(Some((mapping.target_topic.clone(), transformed_data)))
        } else {
            Ok(None)
        }
    }

    /// Start active bridging (spawns background task)
    pub async fn start_active_bridging(&mut self, state: Arc<SharedState>) -> Result<()> {
        if self.active && self.runtime_handle.is_none() {
            let mapping = self.mapping.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = Self::run_bridge_loop(mapping, state).await {
                    error!("Bridge loop error: {}", e);
                }
            });
            self.runtime_handle = Some(handle);
            info!("Bridge: Active bridging started");
        }
        Ok(())
    }

    async fn run_bridge_loop(mapping: BridgeMapping, state: Arc<SharedState>) -> Result<()> {
        match (&mapping.source, &mapping.target) {
            (Protocol::Ros2, Protocol::Zenoh) => {
                Self::bridge_ros2_to_zenoh(mapping, state).await
            }
            (Protocol::Zenoh, Protocol::Mqtt) => {
                Self::bridge_zenoh_to_mqtt(mapping, state).await
            }
            (Protocol::Ros2, Protocol::Mqtt) => {
                Self::bridge_ros2_to_mqtt(mapping, state).await
            }
            _ => {
                info!("Bridge: Unsupported direction, passive mode only");
                Ok(())
            }
        }
    }

    async fn bridge_ros2_to_zenoh(_mapping: BridgeMapping, _state: Arc<SharedState>) -> Result<()> {
        // TODO: Implement ROS2 subscriber -> Zenoh publisher
        // Would use cyclonedds-rs to subscribe and zenoh to publish
        info!("ROS2->Zenoh bridge: Running in passive mode");
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        Ok(())
    }

    async fn bridge_zenoh_to_mqtt(_mapping: BridgeMapping, _state: Arc<SharedState>) -> Result<()> {
        // TODO: Implement Zenoh subscriber -> MQTT publisher
        info!("Zenoh->MQTT bridge: Running in passive mode");
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        Ok(())
    }

    async fn bridge_ros2_to_mqtt(_mapping: BridgeMapping, _state: Arc<SharedState>) -> Result<()> {
        // TODO: Implement ROS2 subscriber -> MQTT publisher
        info!("ROS2->MQTT bridge: Running in passive mode");
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        Ok(())
    }
}

impl Drop for ProtocolBridge {
    fn drop(&mut self) {
        if let Some(handle) = self.runtime_handle.take() {
            handle.abort();
        }
    }
}
