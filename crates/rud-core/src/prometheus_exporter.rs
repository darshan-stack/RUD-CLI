// Prometheus metrics exporter for production monitoring

use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, Opts, Registry, Encoder, TextEncoder,
};
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, error};

pub struct MetricsExporter {
    registry: Registry,
    
    // Node metrics
    pub nodes_total: Gauge,
    pub nodes_online: Gauge,
    pub nodes_offline: Gauge,
    pub nodes_degraded: Gauge,
    
    // Anomaly metrics
    pub anomalies_total: Counter,
    pub anomalies_critical: Counter,
    pub anomalies_high: Counter,
    pub anomalies_medium: Counter,
    
    // Performance metrics
    pub cpu_usage: Gauge,
    pub memory_usage: Gauge,
    pub latency: Histogram,
    pub message_rate: Gauge,
    
    // Remediation metrics
    pub remediations_attempted: Counter,
    pub remediations_successful: Counter,
    pub remediations_failed: Counter,
    
    // Protocol metrics
    pub ros2_messages: Counter,
    pub zenoh_messages: Counter,
    pub mqtt_messages: Counter,
}

impl MetricsExporter {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        let nodes_total = Gauge::with_opts(Opts::new(
            "rud_nodes_total",
            "Total number of discovered nodes"
        ))?;
        
        let nodes_online = Gauge::with_opts(Opts::new(
            "rud_nodes_online",
            "Number of online nodes"
        ))?;
        
        let nodes_offline = Gauge::with_opts(Opts::new(
            "rud_nodes_offline",
            "Number of offline nodes"
        ))?;
        
        let nodes_degraded = Gauge::with_opts(Opts::new(
            "rud_nodes_degraded",
            "Number of degraded nodes"
        ))?;

        let anomalies_total = Counter::with_opts(Opts::new(
            "rud_anomalies_total",
            "Total number of anomalies detected"
        ))?;
        
        let anomalies_critical = Counter::with_opts(Opts::new(
            "rud_anomalies_critical",
            "Number of critical anomalies"
        ))?;
        
        let anomalies_high = Counter::with_opts(Opts::new(
            "rud_anomalies_high",
            "Number of high severity anomalies"
        ))?;
        
        let anomalies_medium = Counter::with_opts(Opts::new(
            "rud_anomalies_medium",
            "Number of medium severity anomalies"
        ))?;

        let cpu_usage = Gauge::with_opts(Opts::new(
            "rud_cpu_usage_percent",
            "CPU usage percentage"
        ))?;
        
        let memory_usage = Gauge::with_opts(Opts::new(
            "rud_memory_usage_percent",
            "Memory usage percentage"
        ))?;
        
        let latency = Histogram::with_opts(HistogramOpts::new(
            "rud_message_latency_microseconds",
            "Message latency in microseconds"
        ))?;
        
        let message_rate = Gauge::with_opts(Opts::new(
            "rud_message_rate_hz",
            "Message processing rate in Hz"
        ))?;

        let remediations_attempted = Counter::with_opts(Opts::new(
            "rud_remediations_attempted_total",
            "Total remediation attempts"
        ))?;
        
        let remediations_successful = Counter::with_opts(Opts::new(
            "rud_remediations_successful_total",
            "Successful remediations"
        ))?;
        
        let remediations_failed = Counter::with_opts(Opts::new(
            "rud_remediations_failed_total",
            "Failed remediations"
        ))?;

        let ros2_messages = Counter::with_opts(Opts::new(
            "rud_ros2_messages_total",
            "Total ROS2 messages processed"
        ))?;
        
        let zenoh_messages = Counter::with_opts(Opts::new(
            "rud_zenoh_messages_total",
            "Total Zenoh messages processed"
        ))?;
        
        let mqtt_messages = Counter::with_opts(Opts::new(
            "rud_mqtt_messages_total",
            "Total MQTT messages processed"
        ))?;

        // Register all metrics
        registry.register(Box::new(nodes_total.clone()))?;
        registry.register(Box::new(nodes_online.clone()))?;
        registry.register(Box::new(nodes_offline.clone()))?;
        registry.register(Box::new(nodes_degraded.clone()))?;
        registry.register(Box::new(anomalies_total.clone()))?;
        registry.register(Box::new(anomalies_critical.clone()))?;
        registry.register(Box::new(anomalies_high.clone()))?;
        registry.register(Box::new(anomalies_medium.clone()))?;
        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(memory_usage.clone()))?;
        registry.register(Box::new(latency.clone()))?;
        registry.register(Box::new(message_rate.clone()))?;
        registry.register(Box::new(remediations_attempted.clone()))?;
        registry.register(Box::new(remediations_successful.clone()))?;
        registry.register(Box::new(remediations_failed.clone()))?;
        registry.register(Box::new(ros2_messages.clone()))?;
        registry.register(Box::new(zenoh_messages.clone()))?;
        registry.register(Box::new(mqtt_messages.clone()))?;

        info!("Prometheus metrics exporter initialized");

        Ok(Self {
            registry,
            nodes_total,
            nodes_online,
            nodes_offline,
            nodes_degraded,
            anomalies_total,
            anomalies_critical,
            anomalies_high,
            anomalies_medium,
            cpu_usage,
            memory_usage,
            latency,
            message_rate,
            remediations_attempted,
            remediations_successful,
            remediations_failed,
            ros2_messages,
            zenoh_messages,
            mqtt_messages,
        })
    }

    /// Update node count metrics
    pub fn update_node_counts(&self, online: usize, offline: usize, degraded: usize) {
        let total = online + offline + degraded;
        self.nodes_total.set(total as f64);
        self.nodes_online.set(online as f64);
        self.nodes_offline.set(offline as f64);
        self.nodes_degraded.set(degraded as f64);
    }

    /// Record an anomaly
    pub fn record_anomaly(&self, severity: &str) {
        self.anomalies_total.inc();
        match severity.to_lowercase().as_str() {
            "critical" => self.anomalies_critical.inc(),
            "high" => self.anomalies_high.inc(),
            "medium" => self.anomalies_medium.inc(),
            _ => {}
        }
    }

    /// Update system metrics
    pub fn update_system_metrics(&self, cpu: f64, memory: f64, msg_rate: f64) {
        self.cpu_usage.set(cpu);
        self.memory_usage.set(memory);
        self.message_rate.set(msg_rate);
    }

    /// Record message latency
    pub fn record_latency(&self, latency_us: f64) {
        self.latency.observe(latency_us);
    }

    /// Record remediation attempt
    pub fn record_remediation(&self, success: bool) {
        self.remediations_attempted.inc();
        if success {
            self.remediations_successful.inc();
        } else {
            self.remediations_failed.inc();
        }
    }

    /// Get metrics in Prometheus text format
    pub fn gather(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    /// Start HTTP server for Prometheus scraping
    pub async fn serve(self: Arc<Self>, port: u16) -> Result<()> {
        use warp::Filter;

        info!("Starting Prometheus metrics server on port {}", port);

        let exporter = self.clone();
        let metrics_route = warp::path("metrics")
            .map(move || {
                match exporter.gather() {
                    Ok(metrics) => warp::reply::with_status(
                        metrics,
                        warp::http::StatusCode::OK,
                    ),
                    Err(e) => {
                        error!("Failed to gather metrics: {}", e);
                        warp::reply::with_status(
                            "Error gathering metrics".to_string(),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    }
                }
            });

        warp::serve(metrics_route)
            .run(([0, 0, 0, 0], port))
            .await;

        Ok(())
    }
}

impl Default for MetricsExporter {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics exporter")
    }
}
