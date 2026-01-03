// Process monitoring and status tracking

use crate::config::{CrashConfig, get_config_dir};
use crate::error::Result;
use crate::utils::command::execute;
use crate::utils::process::get_pid;
use crate::utils::time::{current_timestamp, format_uptime};
use crate::utils::{format_size, get_user};
use public_ip_address::perform_lookup;
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
            if parts.len() >= 2
                && let Ok(kb) = parts[1].parse::<u64>()
            {
                return Ok(kb * 1024); // Convert KB to bytes
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
pub async fn format_status(config: &CrashConfig) -> String {
    let mut lines = vec![(
        "version",
        format!(
            "{} {} ({})",
            env!("CARGO_PKG_VERSION"),
            git_version::git_version!(),
            "https://github.com/ahaoboy/crash"
        ),
    )];
    let exe = config.core.exe_name();

    let core_name = config.core.name();
    if let Ok(ver) = config.get_version() {
        lines.push((
            "core",
            format!("{} {} ({})", core_name, ver, config.core.github(),),
        ));
    }

    let mut is_running = false;
    // PID if running
    if let Ok(pid) = get_pid(&exe) {
        lines.push(("pid", pid.to_string()));
        is_running = true;

        // Memory usage (Unix only)
        if let Ok(memory) = get_memory_usage(pid) {
            let kb = if cfg!(windows) { 1024 } else { 1 };
            lines.push(("memory", format_size(kb * memory)));
        }
    }

    // IP
    if let Ok(response) = perform_lookup(None).await {
        let ip = response.ip;
        let s = match (response.country_code, response.city) {
            (Some(country), Some(city)) => format!("{} ({}-{})", ip, country, city),
            (Some(country), None) => format!("{} ({})", ip, country),
            _ => format!("{}", ip),
        };

        lines.push(("ip", s));
    };

    // Web UI info
    if let Ok(ip) = local_ip_address::local_ip() {
        let port = config.web.host.split(':').nth(1).unwrap_or("9090");
        let ui_name = config.web.ui_name();
        lines.push(("webui", format!("{} (http://{}:{}/ui)", ui_name, ip, port)));
    }

    // Status and uptime
    let status_icon = if is_running { "✅" } else { "❌" };
    let uptime = if is_running && config.start_time > 0 {
        format_uptime(config.start_time)
    } else {
        "0s".to_string()
    };

    // Add max runtime info to status if enabled
    let status_text = if config.max_runtime_hours > 0 {
        format!(
            "{} {} (max: {}h)",
            status_icon, uptime, config.max_runtime_hours
        )
    } else {
        format!("{} {}", status_icon, uptime)
    };

    lines.push(("status", status_text));
    lines.push(("proxy", config.proxy.to_string()));
    let user_prefix = if is_admin::is_admin() { "#" } else { "$" };
    lines.push(("user", format!("{}{}", user_prefix, get_user())));
    lines.push((
        "config",
        format!(
            "{} ({} / {})",
            // config.config_dir.to_string_lossy(),
            get_config_dir().to_string_lossy(),
            format_size(config.get_size(),),
            format_size(fs4::available_space(get_config_dir()).unwrap_or(0))
        ),
    ));

    let key_len = lines.iter().fold(0, |a, b| a.max(b.0.len()));
    lines
        .iter()
        .map(|(k, v)| format!("{:width$} : {}", k, v, width = key_len))
        .collect::<Vec<_>>()
        .join("\n")
}
