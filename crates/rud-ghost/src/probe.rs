// Sentinel Debug Probe - lightweight per-node metric poller that reads system
// telemetry and feeds it into the CAD pipeline.

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use rand::Rng;
use sysinfo::System;
use tokio::time;
use tracing::debug;

use rud_core::{
    metrics::{MetricSample, MetricWindow},
    node::NodeId,
    state::SharedState,
};

use crate::cad::ChaosAnomalyDetector;

pub struct SentinelProbe {
    poll_interval: Duration,
}

impl SentinelProbe {
    pub fn new(poll_interval_ms: u64) -> Self {
        Self {
            poll_interval: Duration::from_millis(poll_interval_ms),
        }
    }

    pub async fn run(
        &self,
        state: Arc<SharedState>,
        cad: Arc<tokio::sync::Mutex<ChaosAnomalyDetector>>,
        metric_window_size: usize,
    ) {
        let mut interval = time::interval(self.poll_interval);
        let mut sys = System::new_all();

        loop {
            interval.tick().await;
            sys.refresh_all();

            let total_cpu: f64 = sys.cpus().iter().map(|c: &sysinfo::Cpu| c.cpu_usage() as f64).sum::<f64>()
                / sys.cpus().len().max(1) as f64;

            let mem_used_mb = (sys.used_memory() as f64) / (1024.0 * 1024.0);

            let node_ids: Vec<(NodeId, String)> = state
                .nodes
                .iter()
                .map(|e| (e.key().clone(), e.value().name.clone()))
                .collect();

            for (node_id, node_name) in node_ids {
                // Per-node metrics: blend real system metrics with simulated per-node variance
                let (latency_us, cpu_pct, mem_mb, msg_rate_hz, drop_rate_pct) = {
                    let mut rng = rand::thread_rng();
                    (
                        100.0 + rng.gen_range(-20.0..20.0) + total_cpu * 2.0,
                        (total_cpu + rng.gen_range(-5.0..5.0)).clamp(0.0, 100.0),
                        (mem_used_mb / 8.0 + rng.gen_range(-10.0..10.0)).max(0.0),
                        50.0 + rng.gen_range(-5.0..5.0),
                        rng.gen_range(0.0..0.5_f64),
                    )
                };

                let mut sample = MetricSample::new(node_id.clone());
                sample.timestamp = Utc::now();
                sample.cpu_pct = cpu_pct;
                sample.mem_mb = mem_mb;
                sample.latency_us = latency_us;
                sample.msg_rate_hz = msg_rate_hz;
                sample.drop_rate_pct = drop_rate_pct;

                state
                    .metrics
                    .entry(node_id.clone())
                    .or_insert_with(|| MetricWindow::new(metric_window_size))
                    .push(sample);

                let mut cad_guard = cad.lock().await;
                cad_guard.ingest(
                    &state,
                    &node_id,
                    &node_name,
                    latency_us,
                    cpu_pct,
                    mem_mb,
                    msg_rate_hz,
                    drop_rate_pct,
                );

                debug!(node = node_name, latency_us, cpu_pct, "Sentinel: sample collected");
            }
        }
    }
}
