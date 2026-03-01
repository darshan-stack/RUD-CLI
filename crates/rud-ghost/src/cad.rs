// Chaos Anomaly Detector (CAD) - Maintains a rolling baseline of node metrics
// and raises anomaly events when observations exceed a configurable z-score
// threshold. Uses Welford's online algorithm for numerically stable mean/variance.

use std::collections::HashMap;

use chrono::Utc;
use tracing::{debug, warn};
use uuid::Uuid;

use rud_core::{
    node::NodeId,
    state::{AnomalyEvent, AnomalyKind, Severity, SharedState},
};

#[derive(Debug, Default, Clone)]
struct WelfordState {
    n: u64,
    mean: f64,
    m2: f64,
}

impl WelfordState {
    fn update(&mut self, value: f64) {
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn variance(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        self.m2 / (self.n - 1) as f64
    }

    fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    fn z_score(&self, value: f64) -> Option<f64> {
        if self.n < 10 {
            return None; // not enough data for a reliable baseline
        }
        let sd = self.std_dev();
        if sd < 1e-9 {
            return None;
        }
        Some((value - self.mean) / sd)
    }
}

#[derive(Debug, Default)]
struct NodeBaseline {
    latency: WelfordState,
    cpu: WelfordState,
    mem: WelfordState,
    msg_rate: WelfordState,
    drop_rate: WelfordState,
}

pub struct ChaosAnomalyDetector {
    threshold: f64,
    baselines: HashMap<NodeId, NodeBaseline>,
    learning_samples: u64,
}

impl ChaosAnomalyDetector {
    pub fn new(z_score_threshold: f64, learning_window_samples: u64) -> Self {
        Self {
            threshold: z_score_threshold,
            baselines: HashMap::new(),
            learning_samples: learning_window_samples,
        }
    }

    pub fn ingest(
        &mut self,
        state: &SharedState,
        node_id: &NodeId,
        node_name: &str,
        latency_us: f64,
        cpu_pct: f64,
        mem_mb: f64,
        msg_rate_hz: f64,
        drop_rate_pct: f64,
    ) {
        let baseline = self.baselines.entry(node_id.clone()).or_default();

        baseline.latency.update(latency_us);
        baseline.cpu.update(cpu_pct);
        baseline.mem.update(mem_mb);
        baseline.msg_rate.update(msg_rate_hz);
        baseline.drop_rate.update(drop_rate_pct);

        // Only detect anomalies after the learning period
        if baseline.latency.n < self.learning_samples {
            debug!(n = baseline.latency.n, "CAD: still in learning phase");
            return;
        }

        // Clone snapshots so the mutable borrow on `baseline` is released
        // before we call `self.check` which takes &self.
        let lat_snap = baseline.latency.clone();
        let cpu_snap = baseline.cpu.clone();
        let drop_snap = baseline.drop_rate.clone();

        self.check(state, node_id, node_name, &lat_snap, latency_us, AnomalyKind::LatencySpike, "latency_us");
        self.check(state, node_id, node_name, &cpu_snap, cpu_pct, AnomalyKind::CpuSurge, "cpu_pct");
        self.check(state, node_id, node_name, &drop_snap, drop_rate_pct, AnomalyKind::MessageDrops, "drop_rate_pct");
    }

    fn check(
        &self,
        state: &SharedState,
        node_id: &NodeId,
        node_name: &str,
        welford: &WelfordState,
        value: f64,
        kind: AnomalyKind,
        metric: &str,
    ) {
        if let Some(z) = welford.z_score(value) {
            if z.abs() > self.threshold {
                let severity = if z.abs() > self.threshold * 2.0 {
                    Severity::Critical
                } else if z.abs() > self.threshold * 1.5 {
                    Severity::High
                } else {
                    Severity::Medium
                };

                warn!(
                    node = node_name,
                    metric,
                    z_score = format!("{:.2}", z),
                    value,
                    severity = severity.to_string(),
                    "CAD: anomaly detected"
                );

                let event = AnomalyEvent {
                    id: Uuid::new_v4(),
                    node_id: node_id.clone(),
                    node_name: node_name.to_string(),
                    kind: kind.clone(),
                    severity,
                    description: format!(
                        "{}: value={:.3} z-score={:.2} (mean={:.3} sd={:.3})",
                        metric, value, z, welford.mean, welford.std_dev()
                    ),
                    detected_at: Utc::now(),
                    z_score: Some(z),
                    remediation: None,
                };

                state.push_anomaly(event);

                // Mark node as anomalous
                if let Some(mut node) = state.nodes.get_mut(node_id) {
                    node.status = rud_core::node::NodeStatus::Anomalous;
                }
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.baselines.len()
    }
}
