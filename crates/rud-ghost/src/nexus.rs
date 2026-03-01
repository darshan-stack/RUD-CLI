// Nexus Remediation Engine (NRE) - Performs causal chain analysis on anomaly
// events and proposes fix actions. Integrates with LLM APIs for intelligent
// reasoning, with fallback to deterministic rule-based policy.

use chrono::Utc;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

use rud_core::state::{AnomalyEvent, AnomalyKind, LogLevel, Severity, SharedState};

#[derive(Debug, Clone)]
pub struct RemediationProposal {
    pub anomaly_id: uuid::Uuid,
    pub node_name: String,
    pub action: RemediationAction,
    pub rationale: String,
    pub confidence: f32,
    pub proposed_at: chrono::DateTime<Utc>,
    pub source: RemediationSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemediationSource {
    Llm(String),    // LLM provider name
    RuleBased,       // Deterministic policy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmRequest {
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmResponse {
    choices: Vec<LlmChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmChoice {
    message: LlmMessage,
}

pub struct LlmConfig {
    pub enabled: bool,
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenAI,
    Anthropic,
    Local(String), // URL for local LLM
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: LlmProvider::OpenAI,
            api_key: String::new(),
            model: "gpt-4".to_string(),
            timeout_secs: 30,
        }
    }
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
    llm_config: LlmConfig,
    http_client: reqwest::Client,
}

impl NexusRemediationEngine {
    pub fn new() -> Self {
        Self::with_config(LlmConfig::default())
    }

    pub fn with_config(llm_config: LlmConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(llm_config.timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            proposals: Vec::new(),
            llm_config,
            http_client,
        }
    }

    pub fn enable_llm(&mut self, provider: LlmProvider, api_key: String, model: String) {
        self.llm_config.enabled = true;
        self.llm_config.provider = provider;
        self.llm_config.api_key = api_key;
        self.llm_config.model = model;
        info!("Nexus: LLM integration enabled");
    }

    pub async fn analyze(&mut self, event: &AnomalyEvent, state: &SharedState) -> RemediationProposal {
        // Try LLM first if enabled
        if self.llm_config.enabled && !self.llm_config.api_key.is_empty() {
            match self.analyze_with_llm(event, state).await {
                Ok(proposal) => {
                    info!(
                        node = event.node_name,
                        action = proposal.action.to_string(),
                        confidence = proposal.confidence,
                        source = "LLM",
                        "Nexus: LLM remediation proposal generated"
                    );
                    self.update_state(event, &proposal, state);
                    self.proposals.push(proposal.clone());
                    return proposal;
                }
                Err(e) => {
                    warn!("Nexus: LLM analysis failed: {}, falling back to rules", e);
                }
            }
        }

        // Fallback to rule-based policy
        let (action, rationale, confidence) = self.policy(event);

        let proposal = RemediationProposal {
            anomaly_id: event.id,
            node_name: event.node_name.clone(),
            action: action.clone(),
            rationale: rationale.clone(),
            confidence,
            proposed_at: Utc::now(),
            source: RemediationSource::RuleBased,
        };

        info!(
            node = event.node_name,
            action = action.to_string(),
            confidence,
            source = "RuleBased",
            "Nexus: Rule-based remediation proposal generated"
        );

        self.update_state(event, &proposal, state);
        self.proposals.push(proposal.clone());
        proposal
    }

    async fn analyze_with_llm(&self, event: &AnomalyEvent, state: &SharedState) -> Result<RemediationProposal, anyhow::Error> {
        let prompt = self.build_llm_prompt(event, state);
        
        let response = match &self.llm_config.provider {
            LlmProvider::OpenAI => self.call_openai(&prompt).await?,
            LlmProvider::Anthropic => self.call_anthropic(&prompt).await?,
            LlmProvider::Local(url) => self.call_local_llm(url, &prompt).await?,
        };

        self.parse_llm_response(&response, event)
    }

    fn build_llm_prompt(&self, event: &AnomalyEvent, state: &SharedState) -> String {
        let node_info = state.nodes.get(&event.node_id)
            .map(|n| format!("Node: {} ({}), Protocol: {}, Status: {:?}", 
                n.name, n.kind, n.protocol, n.status))
            .unwrap_or_else(|| "Node info unavailable".to_string());

        format!(
            r#"You are an expert robotics systems debugger. Analyze the following anomaly and provide a remediation action.

Anomaly Details:
- Type: {:?}
- Severity: {:?}
- Description: {}
- {}

Current System Metrics:
- CPU Usage: N/A
- Memory Usage: N/A  
- Active Nodes: {}
- Recent Anomalies: {}

Available Remediation Actions:
1. RESTART_NODE - Restart the affected node
2. THROTTLE_PUBLISHER(rate_hz) - Reduce message publish rate
3. REALLOCATE_BUFFER(size_mb) - Increase buffer allocation
4. ISOLATE_NODE - Isolate node from network
5. REBALANCE_LOAD(target_nodes) - Distribute load to other nodes
6. ADJUST_QOS(reliability) - Change QoS settings
7. NOTIFY_OPERATOR(message) - Alert human operator

Respond in JSON format:
{{
  "action": "ACTION_NAME",
  "parameters": {{}},
  "rationale": "detailed explanation",
  "confidence": 0.0-1.0
}}
"#,
            event.kind,
            event.severity,
            event.description,
            node_info,
            state.nodes.len(),
            state.anomalies.read().len()
        )
    }

    async fn call_openai(&self, prompt: &str) -> Result<String, anyhow::Error> {
        let request = LlmRequest {
            model: self.llm_config.model.clone(),
            messages: vec![
                LlmMessage {
                    role: "system".to_string(),
                    content: "You are an expert robotics systems debugger.".to_string(),
                },
                LlmMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: 0.3,
            max_tokens: 500,
        };

        let response = self.http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.llm_config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API error {}: {}", status, text));
        }

        let llm_response: LlmResponse = response.json().await?;
        Ok(llm_response.choices.first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?
            .message.content.clone())
    }

    async fn call_anthropic(&self, prompt: &str) -> Result<String, anyhow::Error> {
        let request = serde_json::json!({
            "model": self.llm_config.model,
            "max_tokens": 500,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        let response = self.http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.llm_config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow::anyhow!("Anthropic API error {}: {}", status, text));
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["content"][0]["text"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid Anthropic response"))?;
        Ok(content.to_string())
    }

    async fn call_local_llm(&self, url: &str, prompt: &str) -> Result<String, anyhow::Error> {
        let request = serde_json::json!({
            "prompt": prompt,
            "max_tokens": 500,
            "temperature": 0.3
        });

        let response = self.http_client
            .post(url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Local LLM error: {}", response.status()));
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["response"].as_str()
            .or_else(|| json["text"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid local LLM response"))?;
        Ok(content.to_string())
    }

    fn parse_llm_response(&self, response: &str, event: &AnomalyEvent) -> Result<RemediationProposal, anyhow::Error> {
        // Try to parse JSON from response
        let json: serde_json::Value = if let Ok(j) = serde_json::from_str(response) {
            j
        } else {
            // Try to extract JSON from markdown code blocks
            let json_start = response.find('{').ok_or_else(|| anyhow::anyhow!("No JSON in response"))?;
            let json_end = response.rfind('}').ok_or_else(|| anyhow::anyhow!("No JSON in response"))?;
            serde_json::from_str(&response[json_start..=json_end])?
        };

        let action_str = json["action"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing action field"))?;
        
        let action = self.parse_action(action_str, &json["parameters"])?;
        
        let rationale = json["rationale"].as_str()
            .unwrap_or("LLM-generated remediation")
            .to_string();
        
        let confidence = json["confidence"].as_f64()
            .unwrap_or(0.75) as f32;

        Ok(RemediationProposal {
            anomaly_id: event.id,
            node_name: event.node_name.clone(),
            action,
            rationale,
            confidence,
            proposed_at: Utc::now(),
            source: RemediationSource::Llm(format!("{:?}", self.llm_config.provider)),
        })
    }

    fn parse_action(&self, action_str: &str, params: &serde_json::Value) -> Result<RemediationAction, anyhow::Error> {
        match action_str.to_uppercase().as_str() {
            "RESTART_NODE" => Ok(RemediationAction::RestartNode),
            "THROTTLE_PUBLISHER" => {
                let rate = params["rate_hz"].as_f64().unwrap_or(10.0);
                Ok(RemediationAction::ThrottlePublisher { rate_hz: rate })
            }
            "REALLOCATE_BUFFER" => {
                let size = params["size_mb"].as_u64().unwrap_or(256) as usize;
                Ok(RemediationAction::ReallocateBuffer { new_size_mb: size })
            }
            "ISOLATE_NODE" => Ok(RemediationAction::IsolateNode),
            "REBALANCE_LOAD" => {
                let targets = params["target_nodes"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_else(|| vec!["node-aux-0".into()]);
                Ok(RemediationAction::RebalanceLoad { target_nodes: targets })
            }
            "ADJUST_QOS" => {
                let reliability = params["reliability"].as_str()
                    .unwrap_or("RELIABLE")
                    .to_string();
                Ok(RemediationAction::AdjustQos { reliability })
            }
            "NOTIFY_OPERATOR" => {
                let message = params["message"].as_str()
                    .unwrap_or("Operator attention required")
                    .to_string();
                Ok(RemediationAction::NotifyOperator { message })
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_str))
        }
    }

    fn update_state(&self, event: &AnomalyEvent, proposal: &RemediationProposal, state: &SharedState) {
        // Update anomaly with remediation text
        let mut anomalies = state.anomalies.write();
        if let Some(a) = anomalies.iter_mut().find(|a| a.id == event.id) {
            a.remediation = Some(format!("{} (confidence={:.0}%)", proposal.action, proposal.confidence * 100.0));
        }
        drop(anomalies);

        // Update node status to remediating
        if let Some(mut node) = state.nodes.get_mut(&event.node_id) {
            node.status = rud_core::node::NodeStatus::Remediating;
        }

        state.log(
            LogLevel::Warn,
            "nexus",
            format!("Remediating {} with {}", event.node_name, proposal.action),
        );
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
