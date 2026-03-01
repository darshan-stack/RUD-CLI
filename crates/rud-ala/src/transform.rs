// Protocol transformation functions for data conversion between different
// robotics middleware protocols (ROS2, Zenoh, MQTT).

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use bytes::Bytes;

/// Generic message container for protocol-agnostic data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericMessage {
    pub timestamp: i64,
    pub frame_id: Option<String>,
    pub data: JsonValue,
}

/// Transform ROS2 Twist message to Zenoh velocity format
pub fn ros2_twist_to_zenoh_velocity(data: &[u8]) -> Result<Bytes> {
    // Parse ROS2 Twist message (simplified CDR deserialization)
    // In production, use proper CDR parsing library
    if data.len() < 48 {
        return Err(anyhow!("Invalid Twist message size"));
    }

    // Twist has linear (x,y,z) and angular (x,y,z) components
    // Each f64 = 8 bytes, total 48 bytes
    let linear_x = f64::from_le_bytes(data[0..8].try_into()?);
    let linear_y = f64::from_le_bytes(data[8..16].try_into()?);
    let linear_z = f64::from_le_bytes(data[16..24].try_into()?);
    let angular_x = f64::from_le_bytes(data[24..32].try_into()?);
    let angular_y = f64::from_le_bytes(data[32..40].try_into()?);
    let angular_z = f64::from_le_bytes(data[40..48].try_into()?);

    let zenoh_msg = serde_json::json!({
        "linear": {
            "x": linear_x,
            "y": linear_y,
            "z": linear_z
        },
        "angular": {
            "x": angular_x,
            "y": angular_y,
            "z": angular_z
        }
    });

    Ok(Bytes::from(serde_json::to_vec(&zenoh_msg)?))
}

/// Transform Zenoh data to MQTT JSON format
pub fn zenoh_to_mqtt_json(data: &[u8]) -> Result<Bytes> {
    // Try to parse as JSON first
    if let Ok(json_value) = serde_json::from_slice::<JsonValue>(data) {
        let mqtt_msg = serde_json::json!({
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "payload": json_value
        });
        return Ok(Bytes::from(serde_json::to_vec(&mqtt_msg)?));
    }

    // If not JSON, wrap raw bytes as base64
    let mqtt_msg = serde_json::json!({
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "payload": base64::encode(data)
    });

    Ok(Bytes::from(serde_json::to_vec(&mqtt_msg)?))
}

/// Transform ROS2 message to MQTT JSON
pub fn ros2_to_mqtt_json(data: &[u8]) -> Result<Bytes> {
    // First convert ROS2 CDR to generic format
    let generic = cdr_to_generic(data)?;
    
    let mqtt_msg = serde_json::json!({
        "timestamp": generic.timestamp,
        "frame_id": generic.frame_id,
        "data": generic.data
    });

    Ok(Bytes::from(serde_json::to_vec(&mqtt_msg)?))
}

/// Transform MQTT JSON to ROS2 CDR format
pub fn mqtt_json_to_ros2(data: &[u8]) -> Result<Bytes> {
    let json: JsonValue = serde_json::from_slice(data)?;
    
    // Extract payload
    let payload = json.get("payload")
        .or_else(|| json.get("data"))
        .ok_or_else(|| anyhow!("No payload in MQTT message"))?;

    // Convert to CDR (simplified)
    generic_to_cdr(payload)
}

/// Parse ROS2 CDR format to generic message
fn cdr_to_generic(data: &[u8]) -> Result<GenericMessage> {
    // Simplified CDR parsing - in production use cyclonedds or similar
    // CDR header: 4 bytes encapsulation kind + alignment
    if data.len() < 8 {
        return Err(anyhow!("CDR data too short"));
    }

    // Try to extract as JSON-encoded CDR (common in modern ROS2)
    if let Ok(json) = serde_json::from_slice::<JsonValue>(&data[4..]) {
        return Ok(GenericMessage {
            timestamp: chrono::Utc::now().timestamp_millis(),
            frame_id: None,
            data: json,
        });
    }

    // Fallback: wrap as raw bytes
    Ok(GenericMessage {
        timestamp: chrono::Utc::now().timestamp_millis(),
        frame_id: None,
        data: serde_json::json!({
            "raw": base64::encode(&data[4..])
        }),
    })
}

/// Convert generic JSON to CDR format
fn generic_to_cdr(data: &JsonValue) -> Result<Bytes> {
    // CDR header: [0x00, 0x01, 0x00, 0x00] = little-endian encapsulation
    let mut cdr_data = vec![0x00, 0x01, 0x00, 0x00];
    
    // Serialize JSON payload
    let json_bytes = serde_json::to_vec(data)?;
    cdr_data.extend_from_slice(&json_bytes);

    Ok(Bytes::from(cdr_data))
}

/// Transform ROS2 PointCloud2 to simplified JSON
pub fn ros2_pointcloud_to_json(data: &[u8]) -> Result<Bytes> {
    // PointCloud2 has header + height + width + fields + is_bigendian + point_step + row_step + data
    if data.len() < 32 {
        return Err(anyhow!("Invalid PointCloud2 size"));
    }

    let json = serde_json::json!({
        "type": "pointcloud2",
        "size_bytes": data.len(),
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "note": "Full pointcloud data omitted for efficiency"
    });

    Ok(Bytes::from(serde_json::to_vec(&json)?))
}

/// Transform ROS2 Image to simplified JSON with metadata
pub fn ros2_image_to_json(data: &[u8]) -> Result<Bytes> {
    // Image has header + height + width + encoding + is_bigendian + step + data
    if data.len() < 40 {
        return Err(anyhow!("Invalid Image size"));
    }

    let json = serde_json::json!({
        "type": "image",
        "size_bytes": data.len(),
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "note": "Full image data omitted for efficiency"
    });

    Ok(Bytes::from(serde_json::to_vec(&json)?))
}

/// Apply named transformation
pub fn apply_transform(transform_name: &str, data: &[u8]) -> Result<Bytes> {
    match transform_name {
        "ros2_twist_to_zenoh_velocity" => ros2_twist_to_zenoh_velocity(data),
        "zenoh_to_mqtt_json" => zenoh_to_mqtt_json(data),
        "ros2_to_mqtt_json" => ros2_to_mqtt_json(data),
        "mqtt_json_to_ros2" => mqtt_json_to_ros2(data),
        "ros2_pointcloud_to_json" => ros2_pointcloud_to_json(data),
        "ros2_image_to_json" => ros2_image_to_json(data),
        "identity" | "none" => Ok(Bytes::from(data.to_vec())),
        _ => Err(anyhow!("Unknown transform: {}", transform_name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ros2_twist_transform() {
        let mut data = vec![0u8; 48];
        // Set linear.x = 1.0
        data[0..8].copy_from_slice(&1.0f64.to_le_bytes());
        // Set angular.z = 0.5
        data[40..48].copy_from_slice(&0.5f64.to_le_bytes());

        let result = ros2_twist_to_zenoh_velocity(&data).unwrap();
        let json: JsonValue = serde_json::from_slice(&result).unwrap();
        
        assert_eq!(json["linear"]["x"], 1.0);
        assert_eq!(json["angular"]["z"], 0.5);
    }

    #[test]
    fn test_zenoh_to_mqtt() {
        let zenoh_data = serde_json::json!({"value": 42});
        let zenoh_bytes = serde_json::to_vec(&zenoh_data).unwrap();

        let result = zenoh_to_mqtt_json(&zenoh_bytes).unwrap();
        let json: JsonValue = serde_json::from_slice(&result).unwrap();

        assert!(json.get("timestamp").is_some());
        assert_eq!(json["payload"]["value"], 42);
    }
}
