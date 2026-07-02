// Process monitoring and status tracking

use crate::config::{CrashConfig, get_config_dir};
use crate::error::Result;
use crate::utils::command::execute;
use crate::utils::process::get_pid;
use crate::utils::time::format_uptime;
use crate::utils::{format_size, get_user};
use public_ip_address::perform_lookup;
use std::time::Duration;

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

/// Get memory usage for a process by PID (Windows).
/// Returns the working set size in bytes.
#[cfg(windows)]
pub fn get_memory_usage(pid: u32) -> Result<u64> {
    use crate::CrashError;

    let output = execute(
        "tasklist",
        &["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"],
    )?;

    for line in output.lines() {
        let fields = split_csv(line);
        // CSV columns: "Image Name","PID","Session Name","Session#","Mem Usage"
        if fields.len() < 5 {
            continue;
        }
        if fields.get(1).and_then(|s| s.trim().parse::<u32>().ok()) == Some(pid) {
            // Mem usage field looks like "8,124 K" — keep digits only, value is in KB.
            let kb: String = fields[4].chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(kb) = kb.parse::<u64>() {
                return Ok(kb * 1024); // KB -> bytes
            }
        }
    }

    Err(CrashError::Process(format!("Process '{}' not found", pid)))
}

/// Split a single CSV line, respecting double-quoted fields so that commas
/// inside quotes (e.g. `"8,124 K"`) are not treated as separators.
#[cfg(windows)]
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// Look up the public IP address (async, network-bound) with a 5s timeout.
async fn lookup_public_ip() -> String {
    if let Ok(Ok(response)) =
        tokio::time::timeout(Duration::from_secs(5), perform_lookup(None)).await
    {
        let ip = response.ip;
        match (response.country_code, response.city) {
            (Some(country), Some(city)) => format!("{} ({}-{})", ip, country, city),
            (Some(country), None) => format!("{} ({})", ip, country),
            _ => format!("{}", ip),
        }
    } else {
        "Unknown".to_string()
    }
}

/// Collect all status key/value pairs using blocking operations only:
/// subprocess calls (`get_version`, `get_pid`, `tasklist`/`pidof`), a
/// recursive directory-size walk, and `fs4` disk-space queries. Designed to
/// be run on a `spawn_blocking` thread so the async runtime is not stalled.
fn build_status_lines(config: &CrashConfig, ip_str: &str) -> Vec<(&'static str, String)> {
    let mut lines: Vec<(&'static str, String)> = vec![(
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
            format!("{} {} ({})", core_name, ver, config.core.github()),
        ));
    }

    let mut is_running = false;
    if let Ok(pid) = get_pid(&exe) {
        lines.push(("pid", pid.to_string()));
        is_running = true;

        if let Ok(memory) = get_memory_usage(pid) {
            lines.push(("memory", format_size(memory)));
        }
    }

    lines.push(("ip", ip_str.to_string()));

    if let Ok(ip) = local_ip_address::local_ip() {
        let port = config.web.host.split(':').nth(1).unwrap_or("9090");
        let ui_name = config.web.ui_name();

        let mut version_str = String::new();
        if let Some(version) = config.web.ui_version(&get_config_dir()) {
            version_str = format!(" {}", version);
        }

        lines.push((
            "webui",
            format!("{}{version_str} (http://{}:{}/ui)", ui_name, ip, port),
        ));
    }

    let status_icon = if is_running { "✅" } else { "❌" };
    let uptime = if is_running && config.start_time > 0 {
        format_uptime(config.start_time)
    } else {
        "0s".to_string()
    };

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

    let config_dir = get_config_dir();
    lines.push((
        "config",
        format!(
            "{} (used: {} | free: {} | total: {})",
            config_dir.to_string_lossy(),
            format_size(config.get_size()),
            format_size(fs4::available_space(&config_dir).unwrap_or(0)),
            format_size(fs4::total_space(&config_dir).unwrap_or(0))
        ),
    ));

    lines
}

/// Render a list of `(key, value)` pairs as an aligned `key : value` block.
fn render_lines(lines: &[(&str, String)]) -> String {
    let key_len = lines.iter().fold(0, |a, b| a.max(b.0.len()));
    lines
        .iter()
        .map(|(k, v)| format!("{:width$} : {}", k, v, width = key_len))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format a comprehensive status string for the application.
///
/// The public-IP lookup is network-bound and stays on the async runtime
/// (with a timeout). Everything else is blocking work — subprocess calls,
/// directory walks, disk-space queries — and is dispatched to a blocking
/// thread pool so it cannot stall the runtime.
pub async fn format_status(config: &CrashConfig) -> String {
    let ip_str = lookup_public_ip().await;

    let config = config.clone();
    let lines = tokio::task::spawn_blocking(move || build_status_lines(&config, &ip_str))
        .await
        .unwrap_or_else(|e| vec![("error", format!("status build failed: {}", e))]);

    render_lines(&lines)
}
