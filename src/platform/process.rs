// Cross-platform process management abstractions

use crate::error::{CrashError, Result};
use crate::platform::command::CommandExecutor;
use std::path::Path;
use std::process::Command;

/// Trait for platform-specific process management operations
pub trait ProcessManager: Send + Sync {
    /// Get the process ID for a running process by name
    fn get_pid(&self, name: &str) -> Result<u32>;

    /// Kill a process by name or path
    fn kill_process(&self, name: &str) -> Result<()>;

    /// Check if a process is currently running
    fn is_running(&self, name: &str) -> bool {
        self.get_pid(name).is_ok()
    }
}

/// Unix-specific process manager implementation
#[cfg(unix)]
pub struct UnixProcessManager {
    executor: CommandExecutor,
}

#[cfg(unix)]
impl UnixProcessManager {
    pub fn new() -> Self {
        Self {
            executor: CommandExecutor,
        }
    }
}

#[cfg(unix)]
impl ProcessManager for UnixProcessManager {
    fn get_pid(&self, name: &str) -> Result<u32> {
        let output = self.executor.execute("pidof", &[name])?;

        let pid_str =
            output.trim().split_whitespace().next().ok_or_else(|| {
                CrashError::Process(format!("No process found with name: {}", name))
            })?;

        pid_str
            .parse::<u32>()
            .map_err(|e| CrashError::Process(format!("Failed to parse PID '{}': {}", pid_str, e)))
    }

    fn kill_process(&self, name_or_path: &str) -> Result<()> {
        let process_name = Path::new(name_or_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(name_or_path);

        // Try pkill first
        if let Ok(_) = Command::new("pkill").args(&["-f", process_name]).output() {
            return Ok(());
        }

        // Fallback to killall
        let output = Command::new("killall")
            .arg(process_name)
            .output()
            .map_err(|e| CrashError::Process(format!("Failed to execute killall: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(CrashError::Process(format!(
                "Failed to kill process '{}': {}",
                process_name, stderr
            )))
        }
    }
}

/// Windows-specific process manager implementation
#[cfg(windows)]
pub struct WindowsProcessManager {
    executor: CommandExecutor,
}

#[cfg(windows)]
impl Default for WindowsProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsProcessManager {
    pub fn new() -> Self {
        Self {
            executor: CommandExecutor,
        }
    }
}

#[cfg(windows)]
impl ProcessManager for WindowsProcessManager {
    fn get_pid(&self, name: &str) -> Result<u32> {
        let output = self.executor.execute(
            "tasklist",
            &[
                "/FI",
                &format!("IMAGENAME eq {}", name),
                "/NH",
                "/FO",
                "CSV",
            ],
        )?;

        for line in output.lines() {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with(&format!("\"{}\"", name.to_lowercase())) {
                // Parse CSV format: "name","pid","session","mem"
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 2 {
                    let pid_str = parts[1].trim().trim_matches('"');
                    return pid_str.parse::<u32>().map_err(|e| {
                        CrashError::Process(format!("Failed to parse PID '{}': {}", pid_str, e))
                    });
                }
            }
        }

        Err(CrashError::Process(format!("Process '{}' not found", name)))
    }

    fn kill_process(&self, name_or_path: &str) -> Result<()> {
        let process_name = Path::new(name_or_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(name_or_path);

        let output = Command::new("taskkill")
            .args(["/F", "/IM", process_name])
            .output()
            .map_err(|e| CrashError::Process(format!("Failed to execute taskkill: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(CrashError::Process(format!(
                "Failed to kill process '{}': {}",
                process_name, stderr
            )))
        }
    }
}

/// Factory function to get the appropriate process manager for the current platform
pub fn get_process_manager() -> Box<dyn ProcessManager> {
    #[cfg(unix)]
    return Box::new(UnixProcessManager::new());

    #[cfg(windows)]
    return Box::new(WindowsProcessManager::new());
}
