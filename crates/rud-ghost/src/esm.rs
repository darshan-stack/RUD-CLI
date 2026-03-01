// Echo Simulation Mirror (ESM) - Maintains a digital twin of registered nodes.
// Each tick it advances node state estimates and checks for divergence between
// real telemetry and simulated predictions.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use parking_lot::Mutex;
use rand::Rng;
use tracing::{debug, info};

use rud_core::{node::NodeId, state::SharedState};

#[derive(Debug, Clone)]
pub struct SimNode {
    pub node_id: NodeId,
    pub sim_latency_us: f64,
    pub sim_cpu_pct: f64,
    pub sim_mem_mb: f64,
    pub sim_msg_rate_hz: f64,
    pub last_tick: chrono::DateTime<Utc>,
}

impl SimNode {
    fn advance(&mut self, _dt_ms: u64) {
        let mut rng = rand::thread_rng();
        // Drift simulation: apply small Gaussian noise per tick
        self.sim_latency_us = (self.sim_latency_us + rng.gen_range(-0.5..0.5)).max(0.0);
        self.sim_cpu_pct = (self.sim_cpu_pct + rng.gen_range(-0.2..0.2)).clamp(0.0, 100.0);
        self.sim_mem_mb = (self.sim_mem_mb + rng.gen_range(-0.05..0.1)).max(0.0);
        self.last_tick = Utc::now();
    }

    pub fn divergence(&self, real_latency: f64, real_cpu: f64) -> f64 {
        let lat_err = (self.sim_latency_us - real_latency).powi(2);
        let cpu_err = (self.sim_cpu_pct - real_cpu).powi(2);
        (lat_err + cpu_err).sqrt()
    }
}

pub struct EchoSimMirror {
    twins: Arc<Mutex<HashMap<NodeId, SimNode>>>,
    engine: SimEngine,
    tick_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimEngine {
    Isaac,
    Gazebo,
    Mock,
}

impl std::fmt::Display for SimEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimEngine::Isaac => write!(f, "NVIDIA Isaac"),
            SimEngine::Gazebo => write!(f, "Gazebo"),
            SimEngine::Mock => write!(f, "Mock"),
        }
    }
}

impl EchoSimMirror {
    pub fn new(engine: SimEngine, tick_ms: u64) -> Self {
        info!(engine = engine.to_string(), tick_ms, "ESM: initialized");
        Self {
            twins: Arc::new(Mutex::new(HashMap::new())),
            engine,
            tick_ms,
        }
    }

    pub fn register_node(&self, node_id: NodeId, initial_latency: f64, initial_cpu: f64, initial_mem: f64) {
        let twin = SimNode {
            node_id: node_id.clone(),
            sim_latency_us: initial_latency,
            sim_cpu_pct: initial_cpu,
            sim_mem_mb: initial_mem,
            sim_msg_rate_hz: 10.0,
            last_tick: Utc::now(),
        };
        self.twins.lock().insert(node_id, twin);
        debug!("ESM: node twin registered");
    }

    pub fn tick(&self) {
        let mut twins = self.twins.lock();
        for twin in twins.values_mut() {
            twin.advance(self.tick_ms);
        }
    }

    pub fn divergence_for(&self, node_id: &NodeId, real_latency: f64, real_cpu: f64) -> Option<f64> {
        self.twins.lock().get(node_id).map(|t| t.divergence(real_latency, real_cpu))
    }

    pub fn twin_count(&self) -> usize {
        self.twins.lock().len()
    }

    pub fn tick_interval(&self) -> Duration {
        Duration::from_millis(self.tick_ms)
    }

    pub fn engine(&self) -> &SimEngine {
        &self.engine
    }

    pub fn get_twins_snapshot(&self) -> Vec<SimNode> {
        self.twins.lock().values().cloned().collect()
    }
}

// Background task that advances the simulation at the configured tick rate
pub async fn run_esm_loop(esm: Arc<EchoSimMirror>, state: Arc<SharedState>) {
    let interval = esm.tick_interval();
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        esm.tick();
        // Sync real node statuses into twins
        for entry in state.nodes.iter() {
            let node = entry.value();
            if !esm.twins.lock().contains_key(&node.id) {
                esm.register_node(node.id.clone(), 100.0, 5.0, 128.0);
            }
        }
    }
}
