// Configuration management module

use crate::config::core::Core;
use crate::error::{CrashError, Result};
use crate::log_info;
use crate::utils::fs::{atomic_write, ensure_dir};
use github_proxy::Proxy;
use guess_target::Target;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod core;
pub mod web;

pub use web::WebConfig;

const APP_CONFIG_DIR: &str = ".crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";

/// Main configuration structure for the Crash application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashConfig {
    pub version: String,
    pub config_dir: PathBuf,
    pub start_time: u64,
    pub core: Core,
    pub proxy: Proxy,
    pub target: Target,
    pub web: WebConfig,
    pub url: String,
}

impl Default for CrashConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_dir: get_config_dir(),
            start_time: 0,
            core: Core::default(),
            proxy: Proxy::default(),
            target: Target::default(),
            web: WebConfig::default(),
            url: String::new(),
        }
    }
}

impl CrashConfig {
    /// Load configuration from disk, creating default if not exists
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

            serde_json::from_str(&data)
                .map_err(|e| CrashError::Config(format!("Failed to parse config file: {}", e)))?
        } else {
            log_info!("Config file not found, creating default configuration");
            Self::default()
        };

        config.validate()?;
        config.save()?;

        Ok(config)
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path();

        log_info!("Saving configuration to {}", config_path.display());

        // Ensure config directory exists
        ensure_dir(&self.config_dir)?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CrashError::Config(format!("Failed to serialize config: {}", e)))?;

        atomic_write(&config_path, &json)?;

        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate config directory path
        if self.config_dir.to_str().is_none() {
            return Err(CrashError::Config(
                "Config directory path contains invalid UTF-8".to_string(),
            ));
        }

        // Validate web host format
        if !self.web.host.starts_with(':') && !self.web.host.contains(':') {
            return Err(CrashError::Config(format!(
                "Invalid web host format: {}",
                self.web.host
            )));
        }

        Ok(())
    }

    /// Get the path to the core configuration file
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join(self.core.config_file_name())
    }
}

/// Get the configuration directory path
pub fn get_config_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_CONFIG_DIR)
}

/// Get the configuration file path
pub fn get_config_path() -> PathBuf {
    get_config_dir().join(APP_CONFIG_NAME)
}
