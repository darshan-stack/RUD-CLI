// Daemon lifecycle management - reads/writes PID file and manages process state.

use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

pub fn write_pid(path: &Path) -> Result<()> {
    let pid = std::process::id();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, pid.to_string())
        .with_context(|| format!("failed to write PID file: {}", path.display()))?;
    info!(pid, "daemon PID written");
    Ok(())
}

pub fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn remove_pid(path: &Path) {
    let _ = std::fs::remove_file(path);
}

pub fn is_running(path: &Path) -> bool {
    if let Some(pid) = read_pid(path) {
        // Check if process exists by sending signal 0
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        result == 0
    } else {
        false
    }
}
