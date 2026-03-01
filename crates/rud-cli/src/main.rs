use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use rud_core::{
    config::RudConfig,
    state::{DaemonState, SharedState},
};

mod cli;
mod commands;
mod daemon;
mod tui;

use cli::{Cli, Commands, EsmAction};

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = Cli::parse();

    let log_level = match cli_args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_target(false)
        .with_thread_ids(false)
        .without_time()
        .init();

    let config_path = cli_args
        .config
        .unwrap_or_else(RudConfig::default_path);
    let cfg = RudConfig::load_or_default(&config_path)?;

    let state = SharedState::new(cfg.tui.max_log_lines, cfg.tui.max_metric_history);

    // --init flag
    if cli_args.init {
        commands::cmd_init(state.clone(), &cfg).await?;
        start_background_tasks(state.clone(), &cfg);
        // Keep process alive while TUI is not requested; here we block
        tokio::signal::ctrl_c().await?;
        println!("\n  received Ctrl+C - shutting down");
        *state.daemon_state.write() = DaemonState::ShuttingDown;
        return Ok(());
    }

    match cli_args.command {
        None => {
            // No subcommand: print help
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
        }

        Some(Commands::Scan { all_protocols, protocol, json }) => {
            ensure_initialized(&state);
            commands::cmd_scan(state, all_protocols, protocol.as_deref(), json).await?;
        }

        Some(Commands::Bridge { source, target, semantic_auto, mapping_file }) => {
            ensure_initialized(&state);
            commands::cmd_bridge(state, &source, &target, semantic_auto, mapping_file.as_deref()).await?;
        }

        Some(Commands::Esm { action }) => {
            match action {
                EsmAction::Init { sim_engine, robot, tick_ms } => {
                    ensure_initialized(&state);
                    commands::cmd_esm_init(state, &sim_engine, robot.as_deref(), tick_ms).await?;
                }
                EsmAction::Status => {
                    println!("ESM status: see 'rud tui' F2 panel for live divergence metrics.");
                }
                EsmAction::Pause => {
                    println!("ESM paused.");
                }
                EsmAction::Resume => {
                    println!("ESM resumed.");
                }
            }
        }

        Some(Commands::Tui { refresh_ms }) => {
            // Bootstrap state and start background collection before launching TUI
            {
                let mut ds = state.daemon_state.write();
                if *ds == DaemonState::Uninitialized {
                    *ds = DaemonState::Initializing;
                }
            }
            commands::cmd_init(state.clone(), &cfg).await?;
            start_background_tasks(state.clone(), &cfg);
            tui::run_tui(state.clone(), refresh_ms).await?;
        }

        Some(Commands::Logs { follow, node, level }) => {
            ensure_initialized(&state);
            commands::cmd_logs(state, follow, node.as_deref(), level.as_deref()).await?;
        }

        Some(Commands::Status { json }) => {
            commands::cmd_status(state, json).await?;
        }

        Some(Commands::Anomalies { json, count }) => {
            ensure_initialized(&state);
            commands::cmd_anomalies(state, json, count).await?;
        }

        Some(Commands::Remediate { dry_run, node }) => {
            ensure_initialized(&state);
            commands::cmd_remediate(state, dry_run, node.as_deref()).await?;
        }
    }

    Ok(())
}

fn ensure_initialized(state: &Arc<SharedState>) {
    let ds = state.daemon_state.read();
    match *ds {
        DaemonState::Running | DaemonState::Degraded => {}
        _ => {
            eprintln!(
                "  warning: daemon not initialized. run 'rud --init' first for full functionality."
            );
        }
    }
}

fn start_background_tasks(state: Arc<SharedState>, cfg: &RudConfig) {
    use rud_ghost::{
        cad::ChaosAnomalyDetector,
        esm::{EchoSimMirror, SimEngine, run_esm_loop},
        probe::SentinelProbe,
    };

    let cad_window = cfg.ghost.cad_learning_period_secs * (1000 / cfg.ghost.esm_tick_ms).max(1);
    let cad = Arc::new(tokio::sync::Mutex::new(ChaosAnomalyDetector::new(
        cfg.ghost.cad_z_score_threshold,
        cad_window,
    )));

    let esm = Arc::new(EchoSimMirror::new(SimEngine::Mock, cfg.ghost.esm_tick_ms));

    // Register existing nodes into ESM
    for entry in state.nodes.iter() {
        esm.register_node(entry.key().clone(), 100.0, 5.0, 128.0);
    }

    let state_esm = state.clone();
    let esm_clone = esm.clone();
    tokio::spawn(async move {
        run_esm_loop(esm_clone, state_esm).await;
    });

    let state_probe = state.clone();
    let cad_probe = cad.clone();
    let window_size = cfg.tui.max_metric_history;
    let poll_ms = cfg.ghost.esm_tick_ms;
    tokio::spawn(async move {
        let probe = SentinelProbe::new(poll_ms);
        probe.run(state_probe, cad_probe, window_size).await;
    });

    info!("background tasks started: ESM, Sentinel probe");
}
