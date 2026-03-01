use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rud",
    about = "Robotics Universal Debugger - terminal-native observability for robotics systems",
    version,
    long_about = None,
)]
pub struct Cli {
    /// Initialize the RUD environment and start the background daemon.
    #[arg(long, global = false)]
    pub init: bool,

    /// Path to config file (default: ~/.config/rud/config.toml)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<std::path::PathBuf>,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Discover and register all reachable network nodes across all supported protocols.
    Scan {
        /// Scan all protocols (ROS2/DDS, Zenoh, MQTT) concurrently.
        #[arg(long)]
        all_protocols: bool,

        /// Limit scan to a single protocol.
        #[arg(long, value_name = "PROTOCOL")]
        protocol: Option<String>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Create a transparent protocol bridge between two middleware stacks.
    Bridge {
        /// Source protocol (ros2, zenoh, mqtt, dds).
        source: String,

        /// Target protocol (ros2, zenoh, mqtt, dds).
        target: String,

        /// Automatically derive topic mappings from semantic analysis.
        #[arg(long)]
        semantic_auto: bool,

        /// Path to a manual topic mapping TOML file.
        #[arg(long, value_name = "FILE")]
        mapping_file: Option<std::path::PathBuf>,
    },

    /// Echo Simulation Mirror commands.
    Esm {
        #[command(subcommand)]
        action: EsmAction,
    },

    /// Launch the real-time TUI debug dashboard.
    Tui {
        /// Refresh rate in milliseconds.
        #[arg(long, default_value = "100")]
        refresh_ms: u64,
    },

    /// Stream live logs from all registered nodes.
    Logs {
        /// Follow mode (like tail -f).
        #[arg(short, long)]
        follow: bool,

        /// Filter by node name.
        #[arg(long, value_name = "NODE")]
        node: Option<String>,

        /// Filter by log level.
        #[arg(long, value_name = "LEVEL")]
        level: Option<String>,
    },

    /// Show live node status summary.
    Status {
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Show current anomaly feed.
    Anomalies {
        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Show last N anomalies.
        #[arg(short = 'n', long, default_value = "20")]
        count: usize,
    },

    /// Nexus remediation engine: analyze and apply fixes.
    Remediate {
        /// Analyze anomalies and print proposals without applying.
        #[arg(long)]
        dry_run: bool,

        /// Target specific node.
        #[arg(long, value_name = "NODE")]
        node: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum EsmAction {
    /// Initialize the Echo Simulation Mirror.
    Init {
        /// Simulation engine to use.
        #[arg(long, default_value = "mock", value_name = "ENGINE")]
        sim_engine: String,

        /// Path to robot URDF file.
        #[arg(long, value_name = "URDF")]
        robot: Option<std::path::PathBuf>,

        /// Tick rate in milliseconds.
        #[arg(long, default_value = "50")]
        tick_ms: u64,
    },
    /// Show divergence between digital twins and live nodes.
    Status,
    /// Pause the simulation.
    Pause,
    /// Resume the simulation.
    Resume,
}
