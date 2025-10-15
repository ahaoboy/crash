// Shell command executor

use crate::ShellCrashError;
use crate::common::Result;
use anyhow::Context;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

pub struct ShellExecutor {
    shell: String,
    timeout: Option<Duration>,
}

impl ShellExecutor {
    /// Create a new shell executor with default settings (fish shell)
    pub fn new() -> Self {
        Self {
            shell: "fish".to_string(),
            timeout: Some(Duration::from_secs(30)),
        }
    }

    /// Create with custom shell
    pub fn with_shell(shell: impl Into<String>) -> Self {
        Self {
            shell: shell.into(),
            timeout: Some(Duration::from_secs(30)),
        }
    }

    /// Set command timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Execute a shell command synchronously
    pub fn execute(&self, command: &str) -> Result<Output> {
        // Try fish first, fallback to sh if not available
        let shell = if self.check_command_exists(&self.shell) {
            &self.shell
        } else if self.check_command_exists("sh") {
            "sh"
        } else {
            anyhow::bail!("No suitable shell found (fish or sh)");
        };

        let output = Command::new(shell)
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context(format!("执行命令失败: {}", command))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("命令执行失败: {}", stderr);
        }

        Ok(output)
    }

    /// Execute command and return stdout as string
    pub fn execute_output(&self, command: &str) -> Result<String> {
        let output = self.execute(command)?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a command exists in PATH
    pub fn check_command_exists(&self, command: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            Command::new("where")
                .arg(command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }

        #[cfg(not(target_os = "windows"))]
        {
            Command::new("which")
                .arg(command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }

    /// Execute command with specific working directory
    pub fn execute_in_dir(&self, command: &str, dir: &std::path::Path) -> Result<Output> {
        let shell = if self.check_command_exists(&self.shell) {
            &self.shell
        } else {
            "sh"
        };

        let output = Command::new(shell)
            .arg("-c")
            .arg(command)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| ShellCrashError::ShellError(format!("命令执行失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ShellCrashError::ShellError(format!("命令执行失败: {}", stderr)).into());
        }

        Ok(output)
    }
}

impl Default for ShellExecutor {
    fn default() -> Self {
        Self::new()
    }
}
