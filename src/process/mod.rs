// Process management module

use crate::error::{CrashError, Result};
use crate::platform::process::{ProcessManager, get_process_manager};
use crate::{log_debug, log_error, log_info};
use std::path::Path;
use std::process::{Command, Stdio};

pub mod monitor;

pub use monitor::ProcessMonitor;

/// Process controller for managing proxy core processes
pub struct ProcessController {
    platform_manager: Box<dyn ProcessManager>,
}

impl ProcessController {
    /// Create a new process controller
    pub fn new() -> Self {
        Self {
            platform_manager: get_process_manager(),
        }
    }

    /// Start a process with the given executable path and arguments
    pub fn start(&self, exe_path: &Path, args: Vec<String>) -> Result<()> {
        log_info!(
            "Starting process: {} with args: {:?}",
            exe_path.display(),
            args
        );

        if !exe_path.exists() {
            return Err(CrashError::Process(format!(
                "Executable not found: {}",
                exe_path.display()
            )));
        }

        Command::new(exe_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                log_error!("Failed to start process {}: {}", exe_path.display(), e);
                CrashError::Process(format!(
                    "Failed to start process {}: {}",
                    exe_path.display(),
                    e
                ))
            })?;

        log_info!("Process started successfully: {}", exe_path.display());
        Ok(())
    }

    /// Stop a process by name
    pub fn stop(&self, exe_name: &str) -> Result<()> {
        log_info!("Stopping process: {}", exe_name);

        if !self.is_running(exe_name) {
            log_debug!("Process {} is not running", exe_name);
            return Ok(());
        }

        self.platform_manager.kill_process(exe_name).map_err(|e| {
            log_error!("Failed to stop process {}: {}", exe_name, e);
            e
        })?;

        log_info!("Process stopped successfully: {}", exe_name);
        Ok(())
    }

    /// Restart a process
    pub fn restart(&self, exe_name: &str, exe_path: &Path, args: Vec<String>) -> Result<()> {
        log_info!("Restarting process: {}", exe_name);

        if self.is_running(exe_name) {
            self.stop(exe_name)?;
            // Give the process time to fully terminate
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        self.start(exe_path, args)?;

        log_info!("Process restarted successfully: {}", exe_name);
        Ok(())
    }

    /// Get the process ID for a running process
    pub fn get_pid(&self, exe_name: &str) -> Result<u32> {
        self.platform_manager.get_pid(exe_name)
    }

    /// Check if a process is currently running
    pub fn is_running(&self, exe_name: &str) -> bool {
        self.platform_manager.is_running(exe_name)
    }
}

impl Default for ProcessController {
    fn default() -> Self {
        Self::new()
    }
}
