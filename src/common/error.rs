// Error types for ShellCrash

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShellCrashError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Shell command execution failed: {0}")]
    ShellError(String),

    #[error("Network request failed: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("File operation failed: {0}")]
    IoError(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Service not running")]
    ServiceNotRunning,

    #[error("Port {0} is already in use")]
    PortInUse(u16),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = anyhow::Result<T>;
