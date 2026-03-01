// Semantic Data Overlay - normalizes heterogeneous protocol payloads into
// a unified RudFrame that can be indexed, filtered, and routed by Ghost-Trace.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rud_core::{node::NodeId, protocol::Protocol};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudFrame {
    pub seq: u64,
    pub timestamp: DateTime<Utc>,
    pub source_node: NodeId,
    pub source_protocol: Protocol,
    pub topic: String,
    pub payload: FramePayload,
    pub qos: QosLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FramePayload {
    Json(Value),
    Binary(Vec<u8>),
    Scalar(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QosLevel {
    BestEffort,
    Reliable,
    ExactlyOnce,
}

impl RudFrame {
    pub fn from_ros2(node_id: NodeId, topic: &str, data: &[u8]) -> Self {
        // In a real impl this would CDR-decode the ROS2 message.
        // Here we treat the payload as raw bytes and attempt JSON interpretation.
        let payload = serde_json::from_slice::<Value>(data)
            .map(FramePayload::Json)
            .unwrap_or_else(|_| FramePayload::Binary(data.to_vec()));

        Self {
            seq: 0,
            timestamp: Utc::now(),
            source_node: node_id,
            source_protocol: Protocol::Ros2,
            topic: topic.to_string(),
            payload,
            qos: QosLevel::Reliable,
        }
    }

    pub fn from_zenoh(node_id: NodeId, key_expr: &str, data: &[u8]) -> Self {
        let payload = serde_json::from_slice::<Value>(data)
            .map(FramePayload::Json)
            .unwrap_or_else(|_| FramePayload::Binary(data.to_vec()));

        Self {
            seq: 0,
            timestamp: Utc::now(),
            source_node: node_id,
            source_protocol: Protocol::Zenoh,
            topic: key_expr.to_string(),
            payload,
            qos: QosLevel::BestEffort,
        }
    }

    pub fn from_mqtt(node_id: NodeId, topic: &str, data: &[u8]) -> Self {
        let payload = std::str::from_utf8(data)
            .map(|s| FramePayload::Text(s.to_string()))
            .unwrap_or_else(|_| FramePayload::Binary(data.to_vec()));

        Self {
            seq: 0,
            timestamp: Utc::now(),
            source_node: node_id,
            source_protocol: Protocol::Mqtt,
            topic: topic.to_string(),
            payload,
            qos: QosLevel::BestEffort,
        }
    }

    pub fn as_scalar(&self) -> Option<f64> {
        match &self.payload {
            FramePayload::Scalar(v) => Some(*v),
            FramePayload::Json(Value::Number(n)) => n.as_f64(),
            FramePayload::Text(s) => s.parse().ok(),
            _ => None,
        }
    }
}
