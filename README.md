# RUD-CLI: Robotics Universal Debugger

A production-grade, real-time debugging and monitoring platform for heterogeneous robotics systems. RUD-CLI provides unified observability across ROS2/DDS, Zenoh, and MQTT protocols with AI-powered anomaly detection and automated remediation.

## 🚀 Features

### Core Capabilities
- **Multi-Protocol Discovery**: Real-time discovery of nodes across ROS2/DDS, Zenoh, and MQTT networks
- **Zero-Copy IPC**: Quantum Data Shards (QDS) for high-performance inter-process communication
- **Anomaly Detection**: Statistical CAD (Chaos Anomaly Detector) using Welford's algorithm and z-score analysis
- **AI-Powered Remediation**: LLM integration (OpenAI, Anthropic, or local) for intelligent troubleshooting
- **Protocol Bridging**: Seamless data transformation between ROS2, Zenoh, and MQTT
- **Digital Twin**: Echo Simulation Mirror (ESM) for predictive anomaly detection
- **Production Monitoring**: Prometheus metrics, OpenTelemetry tracing, structured logging
- **Interactive TUI**: Real-time terminal user interface for system visualization

### Supported Protocols
- **ROS2/DDS**: Native CycloneDDS integration with multicast discovery
- **Zenoh**: Peer discovery via scouting API
- **MQTT**: Broker enumeration and topic discovery

## 📦 Installation

### Prerequisites
- Rust 1.70+ (`rustup`)
- Linux (tested on Ubuntu 20.04+)
- Optional: ROS2 installation for DDS discovery
- Optional: Zenoh router for Zenoh discovery
- Optional: MQTT broker (Mosquitto, etc.)

### Build from Source
```bash
git clone https://github.com/darshan-stack/RUD-CLI.git
cd RUD-CLI
cargo build --release

# With Prometheus metrics support
cargo build --release --features prometheus

# Install binary
cargo install --path crates/rud-cli
```

## 🎯 Quick Start

### 1. Basic Usage
```bash
# Start the RUD daemon
rud daemon start

# Check system status
rud status

# Launch interactive TUI
rud tui

# Discover nodes on the network
rud discover

# View detected anomalies
rud anomalies

# Stop the daemon
rud daemon stop
```

### 2. Configuration

Create a configuration file at `~/.config/rud/config.toml`:

```toml
[daemon]
pid_file = "/tmp/rud.pid"
log_level = "info"
telemetry_interval_ms = 1000

[discovery]
enable_ros2 = true
enable_zenoh = true
enable_mqtt = true
scan_interval_secs = 30
use_real_discovery = true  # Enable real protocol discovery
ros_domain_id = 0
mqtt_broker = "localhost"
mqtt_port = 1883

[llm]
enabled = true
provider = "openai"  # or "anthropic", "local"
model = "gpt-4"
timeout_secs = 30
# api_key set via environment variable OPENAI_API_KEY

[monitoring]
enable_prometheus = true
prometheus_port = 9090
enable_opentelemetry = false

[security]
enable_tls = false
require_auth = false
```

### 3. Environment Variables

```bash
# ROS2/DDS Configuration
export ROS_DOMAIN_ID=0

# MQTT Configuration
export MQTT_BROKER=localhost
export MQTT_PORT=1883

# LLM Configuration
export RUD_LLM_ENABLED=true
export RUD_LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-...
# or
export ANTHROPIC_API_KEY=sk-ant-...

# For local LLM (e.g., Ollama, LM Studio)
export RUD_LLM_PROVIDER=local
export RUD_LLM_URL=http://localhost:11434/api/generate

# Enable real protocol discovery (vs mock)
export RUD_REAL_DISCOVERY=true

# Logging
export RUD_LOG_LEVEL=debug
```

## 📊 Architecture

### System Components

```
┌─────────────────────────────────────────────────────────┐
│                        RUD-CLI                          │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │  RUD-CORE   │  │  RUD-AETHER │  │  RUD-GHOST  │    │
│  │             │  │             │  │             │    │
│  │ • Config    │  │ • QDS (IPC) │  │ • CAD       │    │
│  │ • State     │  │ • NRE (Msg) │  │ • ESM       │    │
│  │ • Protocol  │  │ • Fabric    │  │ • Nexus     │    │
│  │ • Metrics   │  │ • Overlay   │  │ • Probe     │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
│  ┌─────────────┐  ┌─────────────┐                     │
│  │  RUD-ALA    │  │  RUD-CLI    │                     │
│  │             │  │             │                     │
│  │ • Discovery │  │ • Commands  │                     │
│  │ • Bridge    │  │ • TUI       │                     │
│  │ • Agents    │  │ • Daemon    │                     │
│  │ • Transform │  │             │                     │
│  └─────────────┘  └─────────────┘                     │
└─────────────────────────────────────────────────────────┘
           │                │                │
           ▼                ▼                ▼
    ┌──────────┐     ┌──────────┐     ┌──────────┐
    │ ROS2/DDS │     │  Zenoh   │     │   MQTT   │
    └──────────┘     └──────────┘     └──────────┘
```

### Key Technologies

- **Quantum Data Shards (QDS)**: Memory-mapped ring buffers for zero-copy IPC
- **Neural Routing Engine (NRE)**: Intelligent message routing and transformation
- **Chaos Anomaly Detector (CAD)**: Online statistical anomaly detection
- **Echo Simulation Mirror (ESM)**: Digital twin for predictive monitoring
- **Nexus Remediation Engine**: AI-powered root cause analysis and fixes

## 🔧 Advanced Usage

### Protocol Bridging

Create bridges between protocols:

```bash
# Bridge ROS2 topics to Zenoh
rud bridge create --source ros2 --target zenoh \
  --mapping "/sensor/lidar:robot/sensor/lidar" \
  --transform ros2_pointcloud_to_json

# Bridge Zenoh to MQTT
rud bridge create --source zenoh --target mqtt \
  --mapping "robot/telemetry/*:rud/telemetry" \
  --transform zenoh_to_mqtt_json

# List active bridges
rud bridge list

# Monitor bridge throughput
rud bridge stats
```

### LLM-Powered Remediation

```bash
# Analyze an anomaly with AI
rud remediate --anomaly-id <uuid> --use-llm

# Get remediation history
rud remediate history

# Apply suggested remediation
rud remediate apply --proposal-id <uuid>
```

### Metrics and Monitoring

```bash
# Start Prometheus metrics server
rud metrics serve --port 9090

# Export current metrics
rud metrics export --format prometheus > metrics.txt

# View metrics in TUI
rud tui --metrics-only
```

### Testing Protocol Discovery

```bash
# Test ROS2/DDS discovery
export ROS_DOMAIN_ID=0
export RUD_REAL_DISCOVERY=true
rud discover --protocol ros2

# Test Zenoh discovery
rud discover --protocol zenoh

# Test MQTT discovery
export MQTT_BROKER=test.mosquitto.org
export MQTT_PORT=1883
rud discover --protocol mqtt
```

## 📈 Prometheus Metrics

When `enable_prometheus = true`, the following metrics are exposed on `:9090/metrics`:

- `rud_nodes_total` - Total discovered nodes
- `rud_nodes_online` - Online node count
- `rud_anomalies_total` - Total anomalies detected
- `rud_anomalies_critical` - Critical severity count
- `rud_cpu_usage_percent` - System CPU usage
- `rud_memory_usage_percent` - System memory usage
- `rud_message_latency_microseconds` - Message latency histogram
- `rud_remediations_attempted_total` - Remediation attempts
- `rud_ros2_messages_total` - ROS2 messages processed
- `rud_zenoh_messages_total` - Zenoh messages processed
- `rud_mqtt_messages_total` - MQTT messages processed

Example Grafana dashboard available in `docs/grafana-dashboard.json`.

## 🔐 Security

### TLS/SSL Configuration

```toml
[security]
enable_tls = true
cert_file = "/path/to/cert.pem"
key_file = "/path/to/key.pem"
ca_file = "/path/to/ca.pem"
require_auth = true
```

### API Key Management

Never commit API keys! Use environment variables:

```bash
# For CI/CD
echo "$OPENAI_API_KEY" | rud config set-secret llm.api_key -

# For local development
export OPENAI_API_KEY=$(pass show openai/api-key)
```

## 🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Run with real protocol integration tests (requires setup)
cargo test --workspace --features integration-tests

# Run benchmarks
cargo bench --workspace

# Check for clippy warnings
cargo clippy --workspace --all-targets --all-features
```

## 📚 Examples

### Example 1: Basic Monitoring

```bash
# Terminal 1: Start RUD
rud daemon start

# Terminal 2: Launch TUI
rud tui

# Terminal 3: Simulate nodes (for testing)
rud simulate nodes --count 5 --protocol ros2
```

### Example 2: Multi-Protocol Setup

```bash
# Configure for multi-protocol environment
cat > ~/.config/rud/config.toml <<EOF
[discovery]
enable_ros2 = true
enable_zenoh = true
enable_mqtt = true
use_real_discovery = true

[llm]
enabled = true
provider = "openai"
EOF

export OPENAI_API_KEY=sk-...
export ROS_DOMAIN_ID=0

# Start monitoring
rud daemon start
rud tui
```

### Example 3: Custom Remediation

```rust
// custom_remediation.rs
use rud_ghost::nexus::{RemediationAction, NexusRemediationEngine};

let mut engine = NexusRemediationEngine::new();

// Add custom remediation logic
engine.add_custom_policy(|event| {
    if event.node_name.contains("critical") {
        RemediationAction::NotifyOperator {
            message: format!("Critical node {} needs attention", event.node_name)
        }
    } else {
        RemediationAction::RestartNode
    }
});
```

## 🤝 Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md).

### Development Setup

```bash
# Clone and setup
git clone https://github.com/darshan-stack/RUD-CLI.git
cd RUD-CLI

# Install dev dependencies
cargo install cargo-watch cargo-edit cargo-audit

# Run in watch mode
cargo watch -x "run -- tui"

# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features
```

## 📝 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- Built with Rust 🦀
- Uses CycloneDDS for ROS2 discovery
- Powered by Eclipse Zenoh for high-performance pub/sub
- AI reasoning via OpenAI and Anthropic APIs

## 📞 Support

- GitHub Issues: https://github.com/darshan-stack/RUD-CLI/issues
- Documentation: https://rud-cli.readthedocs.io
- Discord: https://discord.gg/rud-cli

## 🗺️ Roadmap

- [ ] Support for additional protocols (OPC UA, Modbus)
- [ ] Web-based dashboard UI
- [ ] Kubernetes operator for cloud deployments
- [ ] Historical data replay and analysis
- [ ] Integration with Isaac Sim and Gazebo
- [ ] Plugin system for custom protocol support
- [ ] Machine learning model training for anomaly detection
- [ ] Distributed tracing across robotic fleets

---

**Made with ❤️ for the robotics community**
