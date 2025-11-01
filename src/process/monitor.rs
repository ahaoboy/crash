// Process monitoring and status tracking

use crate::config::CrashConfig;
use crate::error::Result;
use crate::platform::command::execute;
use crate::platform::process::get_pid;
use crate::utils::time::{current_timestamp, format_uptime};
use std::time::Duration;

/// Calculate process uptime from start timestamp
pub fn get_uptime(start_time: u64) -> Duration {
    let current = current_timestamp();
    if current < start_time {
        return Duration::from_secs(0);
    }
    Duration::from_secs(current - start_time)
}
/// Get memory usage for a process by PID (Unix only)
#[cfg(unix)]
pub fn get_memory_usage(pid: u32) -> Result<u64> {
    let output = execute("cat", &[&format!("/proc/{}/status", pid)])?;

    for line in output.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(kb) = parts[1].parse::<u64>() {
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
    }

    Ok(0)
}

/// Get memory usage for a process by PID (Windows - not implemented)
#[cfg(windows)]
pub fn get_memory_usage(pid: u32) -> Result<u64> {
    use crate::CrashError;

    let output = execute(
        "tasklist",
        &["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"],
    )?;

    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains(&format!(",\"{}\",", pid)) {
            // Parse CSV format: "name","pid","session","mem"
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let pid_str = parts[1].trim().trim_matches('"');
                return pid_str.parse::<u64>().map_err(|e| {
                    use crate::CrashError;

                    CrashError::Process(format!("Failed to parse memory '{}': {}", pid_str, e))
                });
            }
        }
    }

    Err(CrashError::Process(format!("Process '{}' not found", pid)))
}

/// Format a comprehensive status string for the application
pub fn format_status(config: &CrashConfig) -> String {
    let mut lines = vec![("version", env!("CARGO_PKG_VERSION").to_string())];
    let exe = config.core.exe_name();

    let core_name = config.core.name();
    if let Ok(ver) = config.get_version() {
        lines.push(("core", format!("{}({})", core_name, ver)));
    }

    let mut is_running = false;
    // PID if running
    if let Ok(pid) = get_pid(&exe) {
        lines.push(("pid", pid.to_string()));
        is_running = true;

        // Memory usage (Unix only)
        if let Ok(memory) = get_memory_usage(pid) {
            let kb = if cfg!(windows) { 1024 } else { 1 };
            lines.push((
                "memory",
                humansize::format_size(kb * memory, humansize::DECIMAL),
            ));
        }
    }

    // Web UI info
    if let Ok(ip) = local_ip_address::local_ip() {
        let port = config.web.host.split(':').nth(1).unwrap_or("9090");
        let ui_name = config.web.ui_name();
        lines.push(("web", format!("{} (http://{}:{}/ui)", ui_name, ip, port)));
    }

    // Status and uptime
    let status_icon = if is_running { "✅" } else { "❌" };
    let uptime = if is_running {
        format_uptime(config.start_time)
    } else {
        "0s".to_string()
    };
    lines.push(("status", format!("{} {}", status_icon, uptime)));

    let key_len = lines.iter().fold(0, |a, b| a.max(b.0.len()));
    lines
        .iter()
        .map(|(k, v)| format!("{:width$} : {}", k, v, width = key_len))
        .collect::<Vec<_>>()
        .join("\n")
}
