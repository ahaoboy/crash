// Process management module

use crate::error::{CrashError, Result};
use crate::platform::process::kill_process;
use crate::{log_debug, log_error, log_info};
use std::path::Path;
use std::process::{Command, Stdio};
pub mod monitor;
use crate::platform::process::is_running;
/// Start a process with the given executable path and arguments
pub fn start(exe_path: &Path, args: Vec<String>, env: Vec<(&str, &str)>) -> Result<()> {
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
        .envs(env)
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
pub fn stop(exe_name: &str) -> Result<()> {
    log_info!("Stopping process: {}", exe_name);

    if !is_running(exe_name) {
        log_debug!("Process {} is not running", exe_name);
        return Ok(());
    }

    kill_process(exe_name).map_err(|e| {
        log_error!("Failed to stop process {}: {}", exe_name, e);
        e
    })?;

    log_info!("Process stopped successfully: {}", exe_name);
    Ok(())
}

/// Restart a process
pub fn restart(
    exe_name: &str,
    exe_path: &Path,
    args: Vec<String>,
    env: Vec<(&str, &str)>,
) -> Result<()> {
    log_info!("Restarting process: {}", exe_name);

    if is_running(exe_name) {
        stop(exe_name)?;
        // Give the process time to fully terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    start(exe_path, args, env)?;

    log_info!("Process restarted successfully: {}", exe_name);
    Ok(())
}
