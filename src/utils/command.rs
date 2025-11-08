// Cross-platform command execution utilities

use crate::{
    error::{CrashError, Result},
    log_info,
};
use std::process::{Command, Stdio};

/// Execute a command synchronously and return its output
pub fn execute(cmd: &str, args: &[&str]) -> Result<String> {
    log_info!("execute {} {}", cmd, args.join(" "));
    let output = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| CrashError::Platform(format!("Failed to execute command '{}': {}", cmd, e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(CrashError::Platform(format!(
            "Command '{}' failed with status {}: {}",
            cmd, output.status, stderr
        )))
    }
}

/// Execute a command asynchronously (spawn and detach)
pub fn execute_async(cmd: &str, args: &[&str]) -> Result<()> {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| CrashError::Platform(format!("Failed to spawn command '{}': {}", cmd, e)))?;

    Ok(())
}
