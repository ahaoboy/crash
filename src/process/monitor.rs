// Process monitoring and status tracking

use crate::config::CrashConfig;
use crate::error::Result;
use crate::utils::time::{current_timestamp, format_uptime};
use std::time::Duration;

/// Process monitor for tracking process status and metrics
pub struct ProcessMonitor;

impl ProcessMonitor {
    pub fn new() -> Self {
        Self
    }

    /// Calculate process uptime from start timestamp
    pub fn get_uptime(&self, start_time: u64) -> Duration {
        let current = current_timestamp();
        if current < start_time {
            return Duration::from_secs(0);
        }
        Duration::from_secs(current - start_time)
    }

    /// Get memory usage for a process by PID (Unix only)
    #[cfg(unix)]
    pub fn get_memory_usage(&self, pid: u32) -> Result<u64> {
        let executor = CommandExecutor;
        let output = executor.execute("cat", &[&format!("/proc/{}/status", pid)])?;

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
    pub fn get_memory_usage(&self, _pid: u32) -> Result<u64> {
        // Windows memory usage retrieval would require additional dependencies
        // For now, return 0
        Ok(0)
    }

    /// Format a comprehensive status string for the application
    pub fn format_status(
        &self,
        config: &CrashConfig,
        is_running: bool,
        pid: Option<u32>,
    ) -> String {
        let mut lines = Vec::new();

        // Version
        lines.push(format!("version      : {}", env!("CARGO_PKG_VERSION")));

        // Core status
        let core_name = config.core.name();
        lines.push(format!("core         : {}", core_name));

        // PID if running
        if let Some(pid) = pid {
            lines.push(format!("pid          : {}", pid));

            // Memory usage (Unix only)
            #[cfg(unix)]
            if let Ok(memory) = self.get_memory_usage(pid) {
                lines.push(format!("memory       : {}", format_size(memory)));
            }
        }

        // Web UI info
        if let Ok(ip) = local_ip_address::local_ip() {
            let port = config.web.host.split(':').nth(1).unwrap_or("9090");
            let ui_name = config.web.ui_name();
            lines.push(format!(
                "web          : {} (http://{}:{}/ui)",
                ui_name, ip, port
            ));
        }

        // Status and uptime
        let status_icon = if is_running { "✅" } else { "❌" };
        let uptime = if is_running {
            format_uptime(config.start_time)
        } else {
            "0s".to_string()
        };
        lines.push(format!("status       : {} {}", status_icon, uptime));

        lines.join("\n")
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}
