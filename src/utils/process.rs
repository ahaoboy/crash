// Process management module

use crate::error::{CrashError, Result};
use crate::{log_debug, log_error, log_info};
use std::path::Path;
use std::process::{Command, Stdio};
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

use crate::utils::command::execute;

#[cfg(target_os = "macos")]
pub fn get_pid(name: &str) -> Result<u32> {
    let output = execute("pgrep", &["-x", name])?;

    let pid_str = output
        .trim()
        .split_whitespace()
        .next()
        .ok_or_else(|| CrashError::Process(format!("No process found with name: {}", name)))?;

    pid_str
        .parse::<u32>()
        .map_err(|e| CrashError::Process(format!("Failed to parse PID '{}': {}", pid_str, e)))
}

#[cfg(target_os = "linux")]
pub fn get_pid(name: &str) -> Result<u32> {
    let output = execute("pidof", &[name])?;

    let pid_str = output
        .split_whitespace()
        .next()
        .ok_or_else(|| CrashError::Process(format!("No process found with name: {}", name)))?;

    pid_str
        .parse::<u32>()
        .map_err(|e| CrashError::Process(format!("Failed to parse PID '{}': {}", pid_str, e)))
}

#[cfg(unix)]
pub fn kill_process(name_or_path: &str) -> Result<()> {
    let process_name = Path::new(name_or_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name_or_path);

    // Try pkill first
    if Command::new("pkill")
        .args(["-f", process_name])
        .output()
        .is_ok()
    {
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

#[cfg(windows)]
pub fn get_pid(name: &str) -> Result<u32> {
    let output = execute(
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

#[cfg(windows)]
pub fn kill_process(name_or_path: &str) -> Result<()> {
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

pub fn is_running(name: &str) -> bool {
    get_pid(name).is_ok()
}
