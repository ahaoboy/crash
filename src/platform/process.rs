// Cross-platform process management abstractions

use crate::error::{CrashError, Result};
use crate::platform::command::execute;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "macos")]
pub fn get_pid(name: &str) -> Result<u32> {
    let output = execute("pgrep", &["-x", name])?;

    let pid = output
        .trim()
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no pid output"))?
        .parse()?;
    Ok(pid)
}

#[cfg(target_os = "linux")]
pub fn get_pid(name: &str) -> Result<u32> {
    let output = execute("pidof", &[name])?;

    let pid_str = output
        .trim()
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
