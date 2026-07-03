// Configuration management module.
//
// Only the persistent storage shape (load / save / validate) and on-disk
// layout helpers live here. Behavioural operations are split across the
// sibling modules:
//   - `runtime`  : start / stop / version probing
//   - `installer`: download / install / upgrade / update-from-url
//   - `patcher`  : core-specific config patching

use crate::config::core::Core;
use crate::error::{CrashError, Result};
use crate::log_info;
use crate::utils::fs::{atomic_write, ensure_dir};
use crate::utils::get_dir_size;
use github_proxy::Proxy;
use guess_target::{Target, get_local_target};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod core;
pub mod installer;
pub mod patcher;
pub mod runtime;
pub mod web;

pub use web::WebConfig;

const APP_CONFIG_DIR: &str = "crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";
const APP_LOG_DIR: &str = "logs";

/// Main configuration structure for the Crash application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashConfig {
    pub version: String,
    pub start_time: u64,
    pub core: Core,
    pub proxy: Proxy,
    pub target: Target,
    pub web: WebConfig,
    pub url: String,
    pub max_runtime_hours: u64,

    #[serde(default)]
    pub stop_force: bool,

    /// Optional URL for the proxy health check performed on each `start`.
    /// Defaults to `https://www.google.com` when `None`. Set to a URL that
    /// is only reachable through the proxy for a meaningful check.
    #[serde(default)]
    pub check_url: Option<String>,
}

impl Default for CrashConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            start_time: 0,
            core: Core::default(),
            proxy: Proxy::default(),
            target: *get_local_target().first().unwrap_or(&Target::default()),
            web: WebConfig::default(),
            url: String::new(),
            stop_force: false,
            max_runtime_hours: 0,
            check_url: None,
        }
    }
}

impl CrashConfig {
    /// Load configuration from disk, creating a default if it does not exist.
    pub fn load() -> Result<Self> {
        let config_path = get_config_path();
        log_info!("Loading configuration from {}", config_path.display());

        let config = if config_path.exists() {
            let data = std::fs::read_to_string(&config_path).map_err(|e| {
                CrashError::Config(format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ))
            })?;

            let config: CrashConfig = serde_json::from_str(&data)
                .map_err(|e| CrashError::Config(format!("Failed to parse config file: {}", e)))?;
            config.validate()?;
            config
        } else {
            log_info!("Config file not found, creating default configuration");
            let config = Self::default();
            config.save()?;
            config
        };

        Ok(config)
    }

    /// Save configuration to disk atomically.
    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path();
        log_info!("Saving configuration to {}", config_path.display());

        ensure_dir(&get_config_dir())?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CrashError::Config(format!("Failed to serialize config: {}", e)))?;

        atomic_write(&config_path, &json)?;

        Ok(())
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<()> {
        if get_config_dir().to_str().is_none() {
            return Err(CrashError::Config(
                "Config directory path contains invalid UTF-8".to_string(),
            ));
        }

        // Validate web host format: must be "[host]:port" with a parseable port.
        let host = self.web.host.trim();
        if host.is_empty() {
            return Err(CrashError::Config("Web host is empty".to_string()));
        }
        let Some(port_part) = host.rsplit_once(':') else {
            return Err(CrashError::Config(format!(
                "Invalid web host format (expected `[host]:port`): {}",
                self.web.host
            )));
        };
        if port_part.1.parse::<u16>().is_err() {
            return Err(CrashError::Config(format!(
                "Invalid web host port (expected 0-65535): {}",
                self.web.host
            )));
        }

        Ok(())
    }

    /// Path to the core's own configuration file.
    pub fn core_config_path(&self) -> PathBuf {
        get_config_dir().join(self.core.config_file_name())
    }

    /// Total size in bytes of the on-disk crash config directory.
    pub fn get_size(&self) -> u64 {
        get_dir_size(&get_config_dir())
    }
}

/// Directory holding the crash config, logs and installed assets.
/// Lives next to the `crash` executable so an install is self-contained.
pub fn get_config_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_CONFIG_DIR)
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join(APP_CONFIG_NAME)
}

pub fn get_log_dir() -> PathBuf {
    get_config_dir().join(APP_LOG_DIR)
}
