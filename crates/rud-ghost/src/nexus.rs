// Nexus Remediation Engine (NRE) - Performs causal chain analysis on anomaly
// events and proposes fix actions. In production this would call an LLM API;
// here we implement a deterministic rule-based policy table that mirrors the
// kind of reasoning an LLM would produce.

use chrono::Utc;
use tracing::info;

use rud_core::state::{AnomalyEvent, AnomalyKind, LogLevel, Severity, SharedState};

#[derive(Debug, Clone)]
pub struct RemediationProposal {
    pub anomaly_id: uuid::Uuid,
    pub node_name: String,
    pub action: RemediationAction,
    pub rationale: String,
    pub confidence: f32,
    pub proposed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum RemediationAction {
    RestartNode,
    ThrottlePublisher { rate_hz: f64 },
    ReallocateBuffer { new_size_mb: usize },
    IsolateNode,
    RebalanceLoad { target_nodes: Vec<String> },
    AdjustQos { reliability: String },
    NotifyOperator { message: String },
}

impl std::fmt::Display for RemediationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemediationAction::RestartNode => write!(f, "RESTART_NODE"),
            RemediationAction::ThrottlePublisher { rate_hz } => {
                write!(f, "THROTTLE_PUBLISHER(rate={:.1}Hz)", rate_hz)
            }
            RemediationAction::ReallocateBuffer { new_size_mb } => {
                write!(f, "REALLOCATE_BUFFER(size={}MB)", new_size_mb)
            }
            RemediationAction::IsolateNode => write!(f, "ISOLATE_NODE"),
            RemediationAction::RebalanceLoad { target_nodes } => {
                write!(f, "REBALANCE_LOAD(targets=[{}])", target_nodes.join(","))
            }
            RemediationAction::AdjustQos { reliability } => {
                write!(f, "ADJUST_QOS(reliability={})", reliability)
            }
            RemediationAction::NotifyOperator { message } => {
                write!(f, "NOTIFY_OPERATOR(\"{}\")", message)
            }
        }
    }
}

pub struct NexusRemediationEngine {
    proposals: Vec<RemediationProposal>,
}

impl NexusRemediationEngine {
    pub fn new() -> Self {
        Self {
            proposals: Vec::new(),
        }
    }

    pub fn analyze(&mut self, event: &AnomalyEvent, state: &SharedState) -> RemediationProposal {
        let (action, rationale, confidence) = self.policy(event);

        let proposal = RemediationProposal {
            anomaly_id: event.id,
            node_name: event.node_name.clone(),
            action: action.clone(),
            rationale: rationale.clone(),
            confidence,
            proposed_at: Utc::now(),
        };

        info!(
            node = event.node_name,
            action = action.to_string(),
            confidence,
            "Nexus: remediation proposal generated"
        );

        // Update anomaly with remediation text
        let mut anomalies = state.anomalies.write();
        if let Some(a) = anomalies.iter_mut().find(|a| a.id == event.id) {
            a.remediation = Some(format!("{} (confidence={:.0}%)", action, confidence * 100.0));
        }
        drop(anomalies);

        // Update node status to remediating
        if let Some(mut node) = state.nodes.get_mut(&event.node_id) {
            node.status = rud_core::node::NodeStatus::Remediating;
        }

        state.log(
            LogLevel::Warn,
            "nexus",
            format!("Remediating {} with {}", event.node_name, action),
        );

        self.proposals.push(proposal.clone());
        proposal
    }

    fn policy(&self, event: &AnomalyEvent) -> (RemediationAction, String, f32) {
        match (&event.kind, &event.severity) {
            (AnomalyKind::LatencySpike, Severity::Critical) => (
                RemediationAction::RestartNode,
                "Critical latency spike indicates unrecoverable queue saturation. Node restart is the fastest path to recovery.".into(),
                0.87,
            ),
            (AnomalyKind::LatencySpike, _) => (
                RemediationAction::ThrottlePublisher { rate_hz: 10.0 },
                "Elevated latency correlated with high publish rate. Throttling upstream publisher should reduce queue depth.".into(),
                0.79,
            ),
            (AnomalyKind::CpuSurge, Severity::Critical) => (
                RemediationAction::IsolateNode,
                "CPU at saturation. Isolating node prevents cascading failures to dependent nodes.".into(),
                0.82,
            ),
            (AnomalyKind::CpuSurge, _) => (
                RemediationAction::RebalanceLoad {
                    target_nodes: vec!["node-aux-0".into(), "node-aux-1".into()],
                },
                "CPU pressure can be relieved by routing non-critical topics to auxiliary nodes.".into(),
                0.71,
            ),
            (AnomalyKind::MemoryLeak, _) => (
                RemediationAction::ReallocateBuffer { new_size_mb: 256 },
                "Memory growth trend detected. Expanding QDS buffer allocation defers OOM condition while root cause is investigated.".into(),
                0.65,
            ),
            (AnomalyKind::MessageDrops, _) => (
                RemediationAction::AdjustQos {
                    reliability: "RELIABLE".into(),
                },
                "Drop rate exceeds threshold. Upgrading QoS to RELIABLE enables sender-level retransmission.".into(),
                0.76,
            ),
            (AnomalyKind::NodeOffline, _) => (
                RemediationAction::NotifyOperator {
                    message: format!("Node '{}' has gone offline. Manual inspection required.", event.node_name),
                },
                "Node offline state cannot be remediated automatically; operator intervention required.".into(),
                1.0,
            ),
            (AnomalyKind::ProtocolError, _) => (
                RemediationAction::AdjustQos {
                    reliability: "BEST_EFFORT".into(),
                },
                "Protocol errors often indicate version mismatch. Relaxing QoS allows session recovery.".into(),
                0.60,
            ),
        }
    }

    pub fn proposals(&self) -> &[RemediationProposal] {
        &self.proposals
    }
}

impl Default for NexusRemediationEngine {
    fn default() -> Self {
        Self::new()
    }
}
