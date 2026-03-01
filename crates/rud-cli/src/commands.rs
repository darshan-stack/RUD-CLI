// Command implementations - each pub function maps to a CLI subcommand.
// They receive Arc<SharedState> and return anyhow::Result<()>.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use tracing::info;

use rud_ala::{
    bridge::ProtocolBridge,
    discovery::ProtocolScanner,
};
use rud_core::{
    protocol::Protocol,
    state::{LogLevel, SharedState},
};
use rud_ghost::esm::{EchoSimMirror, SimEngine};

// ---- helpers -----------------------------------------------------------

fn parse_protocol(s: &str) -> Result<Protocol> {
    match s.to_lowercase().as_str() {
        "ros2" | "ros" => Ok(Protocol::Ros2),
        "zenoh" => Ok(Protocol::Zenoh),
        "mqtt" => Ok(Protocol::Mqtt),
        "dds" => Ok(Protocol::Dds),
        other => anyhow::bail!("unknown protocol '{}'. supported: ros2, zenoh, mqtt, dds", other),
    }
}

fn parse_engine(s: &str) -> SimEngine {
    match s.to_lowercase().as_str() {
        "isaac" => SimEngine::Isaac,
        "gazebo" => SimEngine::Gazebo,
        _ => SimEngine::Mock,
    }
}

fn print_banner(cmd: &str) {
    println!();
    println!("  RUD :: Robotics Universal Debugger");
    println!("  command : {}", cmd);
    println!("  time    : {}", Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"));
    println!("  ----------------------------------------");
    println!();
}

// ---- commands ----------------------------------------------------------

pub async fn cmd_init(state: Arc<SharedState>, cfg: &rud_core::config::RudConfig) -> Result<()> {
    print_banner("init");

    {
        let mut ds = state.daemon_state.write();
        *ds = rud_core::state::DaemonState::Initializing;
    }

    println!("  [1/4] bootstrapping Aether-Link fabric ...");

    let fabric = rud_aether::fabric::AetherFabric::bootstrap(&cfg.aether)?;
    let stats = fabric.stats();

    println!(
        "        QDS  : {} shards x {} MB/shard = {} MB total",
        stats.qds_shards,
        cfg.aether.qds_shard_size_mb,
        stats.qds_capacity_mb
    );
    println!("        NRE  : online (topics={})", stats.nre_topics);

    println!("  [2/4] initializing Ghost-Trace intelligence layer ...");
    println!("        CAD  : z-score threshold = {}", cfg.ghost.cad_z_score_threshold);
    println!("        ESM  : tick = {}ms", cfg.ghost.esm_tick_ms);
    println!("        Nexus: {}", if cfg.ghost.nexus_enabled { "enabled" } else { "disabled" });

    println!("  [3/4] registering default ALA node agents ...");
    let agents = default_agents();
    for agent in &agents {
        agent.register(&state);
        println!(
            "        [+] {} ({}/{})",
            agent.node.name, agent.node.kind, agent.node.protocol
        );
    }

    println!("  [4/4] starting Sentinel probe ...");

    {
        let mut ds = state.daemon_state.write();
        *ds = rud_core::state::DaemonState::Running;
        *state.qds_shards_allocated.write() = stats.qds_shards;
    }

    state.log(LogLevel::Info, "daemon", "rud daemon initialized");
    info!("RUD daemon initialized and running");

    println!();
    println!("  daemon state : RUNNING");
    println!("  pid file     : {}", cfg.daemon.pid_file.display());
    println!("  socket       : {}", cfg.daemon.socket_path.display());
    println!();
    println!("  run 'rud tui' to open the live dashboard");
    println!();

    Ok(())
}

fn default_agents() -> Vec<rud_ala::agents::AlaAgent> {
    vec![
        rud_ala::agents::sensor_ala("sensor-lidar-0", "239.255.0.1:7400"),
        rud_ala::agents::sensor_ala("sensor-imu-0", "239.255.0.1:7401"),
        rud_ala::agents::control_ala("control-base-0", "239.255.0.1:7400"),
        rud_ala::agents::inference_ala("infer-pose-0", "tcp/localhost:7447"),
        rud_ala::agents::comms_ala("comms-mqtt-0", "tcp://localhost:1883"),
    ]
}

pub async fn cmd_scan(state: Arc<SharedState>, all_protocols: bool, protocol: Option<&str>, json_out: bool) -> Result<()> {
    if !json_out {
        print_banner("scan");
        println!("  scanning for nodes ({}) ...", if all_protocols { "all-protocols" } else { protocol.unwrap_or("ros2") });
        println!();
    }

    let endpoints = ProtocolScanner::scan_all(&state).await?;

    if json_out {
        let out: Vec<serde_json::Value> = endpoints.iter().map(|ep| {
            json!({
                "protocol": ep.protocol.to_string(),
                "address": ep.address,
                "topic": ep.topic_or_service,
                "kind": ep.node_type_hint.to_string(),
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("  {:<12} {:<24} {:<36} {:<12}", "PROTOCOL", "ADDRESS", "TOPIC/SERVICE", "KIND");
        println!("  {}", "-".repeat(88));
        for ep in &endpoints {
            println!(
                "  {:<12} {:<24} {:<36} {:<12}",
                ep.protocol.to_string(),
                ep.address,
                ep.topic_or_service,
                ep.node_type_hint.to_string()
            );
        }
        println!();
        println!("  {} nodes discovered", endpoints.len());
        println!();
    }

    Ok(())
}

pub async fn cmd_bridge(
    state: Arc<SharedState>,
    source: &str,
    target: &str,
    semantic_auto: bool,
    _mapping_file: Option<&std::path::Path>,
) -> Result<()> {
    print_banner("bridge");

    let src_proto = parse_protocol(source)?;
    let tgt_proto = parse_protocol(target)?;

    println!("  source   : {}", src_proto);
    println!("  target   : {}", tgt_proto);
    println!("  semantic : {}", if semantic_auto { "auto" } else { "manual" });
    println!();

    let mut bridge = ProtocolBridge::new(src_proto.clone(), tgt_proto.clone(), semantic_auto);
    bridge.activate(&state)?;

    println!("  {:<40} {:<40} {:<30}", "SOURCE TOPIC", "TARGET TOPIC", "TRANSFORM");
    println!("  {}", "-".repeat(112));
    for m in &bridge.mapping.topic_map {
        println!(
            "  {:<40} {:<40} {:<30}",
            m.source_topic,
            m.target_topic,
            m.transform.as_deref().unwrap_or("identity")
        );
    }
    println!();
    println!("  bridge active: {} -> {}", src_proto, tgt_proto);
    println!();

    Ok(())
}

pub async fn cmd_esm_init(
    state: Arc<SharedState>,
    sim_engine: &str,
    robot: Option<&std::path::Path>,
    tick_ms: u64,
) -> Result<()> {
    print_banner("esm init");

    let engine = parse_engine(sim_engine);

    println!("  engine   : {}", engine);
    println!("  robot    : {}", robot.map(|p| p.display().to_string()).unwrap_or_else(|| "none (headless)".into()));
    println!("  tick     : {}ms", tick_ms);
    println!();

    let esm = Arc::new(EchoSimMirror::new(engine, tick_ms));

    // Register existing nodes into ESM
    let node_list: Vec<(rud_core::node::NodeId, String)> = state
        .nodes
        .iter()
        .map(|e| (e.key().clone(), e.value().name.clone()))
        .collect();

    for (id, name) in &node_list {
        esm.register_node(id.clone(), 100.0, 5.0, 128.0);
        println!("  [+] twin created for '{}'", name);
    }

    println!();
    println!("  ESM online: {} digital twins active", esm.twin_count());
    println!("  use 'rud esm status' to view divergence metrics");
    println!();

    state.log(LogLevel::Info, "esm", format!("ESM initialized with {} twins", esm.twin_count()));

    Ok(())
}

pub async fn cmd_status(state: Arc<SharedState>, json_out: bool) -> Result<()> {
    if !json_out {
        print_banner("status");
    }

    let daemon_state = state.daemon_state.read().clone();
    let node_count = state.nodes.len();
    let anomaly_count = state.anomalies.read().len();
    let shards = *state.qds_shards_allocated.read();
    let uptime = Utc::now() - state.started_at;

    if json_out {
        println!("{}", serde_json::to_string_pretty(&json!({
            "daemon_state": daemon_state.to_string(),
            "node_count": node_count,
            "anomaly_count": anomaly_count,
            "qds_shards": shards,
            "uptime_secs": uptime.num_seconds(),
        }))?);
    } else {
        println!("  daemon       : {}", daemon_state);
        println!("  nodes        : {}", node_count);
        println!("  anomalies    : {}", anomaly_count);
        println!("  qds shards   : {}", shards);
        println!("  uptime       : {}s", uptime.num_seconds());
        println!();

        if node_count > 0 {
            println!("  {:<20} {:<10} {:<14} {:<12}", "NAME", "KIND", "PROTOCOL", "STATUS");
            println!("  {}", "-".repeat(60));
            for entry in state.nodes.iter() {
                let n = entry.value();
                println!(
                    "  {:<20} {:<10} {:<14} {:<12}",
                    n.name, n.kind, n.protocol, n.status
                );
            }
            println!();
        }
    }

    Ok(())
}

pub async fn cmd_anomalies(state: Arc<SharedState>, json_out: bool, count: usize) -> Result<()> {
    if !json_out {
        print_banner("anomalies");
    }

    let anomalies = state.anomalies.read();
    let start = anomalies.len().saturating_sub(count);
    let slice = &anomalies[start..];

    if json_out {
        let out: Vec<serde_json::Value> = slice.iter().map(|a| {
            json!({
                "id": a.id.to_string(),
                "node": a.node_name,
                "kind": a.kind.to_string(),
                "severity": a.severity.to_string(),
                "description": a.description,
                "detected_at": a.detected_at.to_rfc3339(),
                "z_score": a.z_score,
                "remediation": a.remediation,
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if slice.is_empty() {
        println!("  no anomalies recorded");
        println!();
    } else {
        println!("  {:<8} {:<20} {:<16} {:<10} {:<30}", "ID", "NODE", "KIND", "SEVERITY", "DESCRIPTION");
        println!("  {}", "-".repeat(90));
        for a in slice {
            println!(
                "  {:<8} {:<20} {:<16} {:<10} {:<30}",
                &a.id.to_string()[..8],
                a.node_name,
                a.kind.to_string(),
                a.severity.to_string(),
                a.description.chars().take(30).collect::<String>()
            );
        }
        println!();
    }

    Ok(())
}

pub async fn cmd_logs(state: Arc<SharedState>, follow: bool, node: Option<&str>, level: Option<&str>) -> Result<()> {
    print_banner("logs");

    let print_logs = |logs: &std::collections::VecDeque<rud_core::state::LogEntry>| {
        for entry in logs {
            if let Some(n) = node {
                if !entry.source.contains(n) {
                    continue;
                }
            }
            if let Some(l) = level {
                if entry.level.to_string().to_lowercase() != l.to_lowercase() {
                    continue;
                }
            }
            println!(
                "  {} [{:<5}] [{}] {}",
                entry.timestamp.format("%H:%M:%S%.3f"),
                entry.level.to_string(),
                entry.source,
                entry.message
            );
        }
    };

    {
        let logs = state.logs.read();
        print_logs(&logs);
    }

    if follow {
        let mut last_len = state.logs.read().len();
        println!("  -- following (Ctrl+C to exit) --");
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let logs = state.logs.read();
            let current_len = logs.len();
            if current_len > last_len {
                let new_entries: std::collections::VecDeque<_> =
                    logs.iter().skip(last_len).cloned().collect();
                print_logs(&new_entries);
                last_len = current_len;
            }
        }
    }

    Ok(())
}

pub async fn cmd_remediate(state: Arc<SharedState>, dry_run: bool, node: Option<&str>) -> Result<()> {
    print_banner("remediate");

    let anomalies = state.anomalies.read().clone();
    let unresolved: Vec<_> = anomalies
        .iter()
        .filter(|a| a.remediation.is_none())
        .filter(|a| node.map(|n| a.node_name.contains(n)).unwrap_or(true))
        .cloned()
        .collect();

    if unresolved.is_empty() {
        println!("  no unresolved anomalies found");
        return Ok(());
    }

    drop(anomalies);

    let mut nexus = rud_ghost::nexus::NexusRemediationEngine::new();

    println!("  mode: {}", if dry_run { "DRY RUN (no changes applied)" } else { "LIVE" });
    println!();
    println!("  analyzing {} anomalies ...", unresolved.len());
    println!();

    for anomaly in &unresolved {
        let proposal = nexus.analyze(anomaly, &state).await;
        println!("  anomaly  : {} ({}/{})", anomaly.node_name, anomaly.kind, anomaly.severity);
        println!("  action   : {}", proposal.action);
        println!("  rationale: {}", proposal.rationale);
        println!("  confidence: {:.0}%", proposal.confidence * 100.0);
        if !dry_run {
            println!("  status   : applied");
        } else {
            println!("  status   : skipped (dry-run)");
        }
        println!();
    }

    Ok(())
}
