// Aether-Link Fabric - top-level bootstrap that wires together QDS and NRE.

use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use rud_core::config::AetherConfig;

use crate::{
    nre::NeuralRoutingEngine,
    qds::{QdsConfig, QdsFabric},
};

pub struct AetherFabric {
    pub qds: QdsFabric,
    pub nre: Arc<NeuralRoutingEngine>,
}

impl AetherFabric {
    pub fn bootstrap(cfg: &AetherConfig) -> Result<Self> {
        info!("Aether-Link: bootstrapping fabric");

        let qds_config = QdsConfig {
            shard_count: cfg.qds_shard_count,
            shard_size_bytes: cfg.qds_shard_size_mb * 1024 * 1024,
            slot_size: 4096,
            base_path: cfg.qds_path.clone(),
        };

        let qds = QdsFabric::initialize(qds_config)?;
        let nre = Arc::new(NeuralRoutingEngine::new());

        info!(
            shards = qds.shard_count(),
            total_mb = qds.total_capacity_mb(),
            "Aether-Link: QDS fabric online"
        );

        info!("Aether-Link: NRE routing engine online");

        Ok(Self { qds, nre })
    }

    pub fn stats(&self) -> FabricStats {
        let qds = self.qds.stats();
        FabricStats {
            qds_shards: qds.shard_count,
            qds_capacity_mb: qds.total_capacity_mb,
            nre_topics: self.nre.topic_count(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FabricStats {
    pub qds_shards: usize,
    pub qds_capacity_mb: usize,
    pub nre_topics: usize,
}
