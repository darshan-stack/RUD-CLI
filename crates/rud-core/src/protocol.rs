use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Ros2,
    Zenoh,
    Mqtt,
    Dds,
    Custom(String),
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Ros2 => write!(f, "ROS2/DDS"),
            Protocol::Zenoh => write!(f, "Zenoh"),
            Protocol::Mqtt => write!(f, "MQTT"),
            Protocol::Dds => write!(f, "DDS"),
            Protocol::Custom(s) => write!(f, "Custom({})", s),
        }
    }
}

impl Protocol {
    pub fn default_port(&self) -> Option<u16> {
        match self {
            Protocol::Mqtt => Some(1883),
            Protocol::Zenoh => Some(7447),
            Protocol::Ros2 | Protocol::Dds => None,
            Protocol::Custom(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMapping {
    pub source: Protocol,
    pub target: Protocol,
    pub topic_map: Vec<TopicMapping>,
    pub semantic_auto: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    pub source_topic: String,
    pub target_topic: String,
    pub transform: Option<String>,
}
