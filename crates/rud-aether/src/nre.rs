// Neural Routing Engine (NRE) - Routes messages between ALA nodes and Ghost-Trace
// using a topic-based publish/subscribe model backed by crossbeam channels.

use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use tracing::{debug, warn};

use rud_core::node::NodeId;

const CHANNEL_CAPACITY: usize = 4096;

#[derive(Debug, Clone)]
pub struct Envelope {
    pub from: NodeId,
    pub topic: String,
    pub payload: bytes::Bytes,
    pub seq: u64,
    pub timestamp_ns: u64,
}

impl Envelope {
    pub fn new(from: NodeId, topic: impl Into<String>, payload: impl Into<bytes::Bytes>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            from,
            topic: topic.into(),
            payload: payload.into(),
            seq: SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            timestamp_ns: now,
        }
    }
}

type TopicSubscribers = Vec<Sender<Envelope>>;

pub struct NeuralRoutingEngine {
    subscriptions: Arc<DashMap<String, TopicSubscribers>>,
    // wildcard prefix subscriptions: prefix -> senders
    prefix_subs: Arc<DashMap<String, TopicSubscribers>>,
}

impl NeuralRoutingEngine {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(DashMap::new()),
            prefix_subs: Arc::new(DashMap::new()),
        }
    }

    pub fn subscribe(&self, topic: &str) -> Receiver<Envelope> {
        let (tx, rx) = bounded(CHANNEL_CAPACITY);
        if topic.ends_with("/*") {
            let prefix = topic.trim_end_matches("/*").to_string();
            self.prefix_subs.entry(prefix).or_default().push(tx);
        } else {
            self.subscriptions.entry(topic.to_string()).or_default().push(tx);
        }
        debug!(topic, "NRE: new subscriber");
        rx
    }

    pub fn publish(&self, envelope: Envelope) {
        let mut delivered = 0usize;

        if let Some(mut subs) = self.subscriptions.get_mut(&envelope.topic) {
            subs.retain(|tx| {
                match tx.try_send(envelope.clone()) {
                    Ok(_) => { delivered += 1; true }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => false,
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        warn!(topic = envelope.topic, "NRE: subscriber channel full, dropping");
                        true
                    }
                }
            });
        }

        // Prefix wildcard dispatch
        for mut entry in self.prefix_subs.iter_mut() {
            if envelope.topic.starts_with(entry.key().as_str()) {
                entry.value_mut().retain(|tx| {
                    match tx.try_send(envelope.clone()) {
                        Ok(_) => { delivered += 1; true }
                        Err(crossbeam_channel::TrySendError::Disconnected(_)) => false,
                        Err(_) => true,
                    }
                });
            }
        }

        debug!(topic = envelope.topic, delivered, "NRE: message routed");
    }

    pub fn topic_count(&self) -> usize {
        self.subscriptions.len() + self.prefix_subs.len()
    }
}

impl Default for NeuralRoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}
