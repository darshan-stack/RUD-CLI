use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::node::NodeId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub node_id: NodeId,
    pub timestamp: DateTime<Utc>,
    pub cpu_pct: f64,
    pub mem_mb: f64,
    pub latency_us: f64,
    pub msg_rate_hz: f64,
    pub drop_rate_pct: f64,
    pub custom: std::collections::HashMap<String, f64>,
}

impl MetricSample {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            timestamp: Utc::now(),
            cpu_pct: 0.0,
            mem_mb: 0.0,
            latency_us: 0.0,
            msg_rate_hz: 0.0,
            drop_rate_pct: 0.0,
            custom: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricWindow {
    pub samples: std::collections::VecDeque<MetricSample>,
    pub capacity: usize,
}

impl MetricWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, sample: MetricSample) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn mean_latency(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|s| s.latency_us).sum();
        sum / self.samples.len() as f64
    }

    pub fn mean_cpu(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|s| s.cpu_pct).sum();
        sum / self.samples.len() as f64
    }

    pub fn std_dev_latency(&self) -> f64 {
        let mean = self.mean_latency();
        if self.samples.len() < 2 {
            return 0.0;
        }
        let variance: f64 = self.samples
            .iter()
            .map(|s| (s.latency_us - mean).powi(2))
            .sum::<f64>()
            / (self.samples.len() - 1) as f64;
        variance.sqrt()
    }
}
