// Configuration management module

use crate::config::core::Core;
use crate::error::{CrashError, Result};
use crate::platform::command::execute;
use crate::platform::process::get_pid;
use crate::process::{restart, start, stop};
use crate::utils::fs::{atomic_write, ensure_dir};
use crate::utils::{current_timestamp, file_exists};
use crate::{log_debug, log_info};
use github_proxy::{Proxy, Resource};
use guess_target::Target;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod core;
pub mod web;

pub use web::WebConfig;

const APP_CONFIG_DIR: &str = "crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";
const APP_LOG_DIR: &str = "logs";

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

    pub fn start(&mut self, force: bool) -> Result<()> {
        log_info!("Starting proxy core: {}", self.core.name());

        let exe_path = self.core.exe_path(&self.config_dir);

        if !exe_path.exists() {
            return Err(CrashError::Process(format!(
                "Core executable not found: {}. Please run 'install' first.",
                exe_path.display()
            )));
        }

        if get_pid(&self.core.exe_name()).is_ok() {
            log_info!("Skip starting proxy core: {}", self.core.name());
            if force {
                self.stop()?;
            } else {
                return Ok(());
            }
        }

        let args = match self.core {
            Core::Mihomo | Core::Clash => vec![
                "-f".to_string(),
                self.config_path().to_string_lossy().to_string(),
                "-ext-ctl".to_string(),
                self.web.host.clone(),
                "-ext-ui".to_string(),
                self.web.ui_name().to_string(),
                "-d".to_string(),
                self.config_dir.to_string_lossy().to_string(),
            ],
            Core::Singbox => {
                vec![
                    "run".to_string(),
                    "-c".to_string(),
                    self.config_path().to_string_lossy().to_string(),
                    "-D".to_string(),
                    self.config_dir.to_string_lossy().to_string(),
                ]
            }
        };

        start(&exe_path, args)?;

        self.start_time = current_timestamp();
        self.save()?;

        log_info!("Proxy core started successfully");
        Ok(())
    }

    /// Stop the proxy core
    pub fn stop(&mut self) -> Result<()> {
        let _config = CrashConfig::load()?;

        log_info!("Stopping proxy core: {}", self.core.name());

        let exe_name = self.core.exe_name();
        stop(&exe_name)?;

        self.start_time = 0;
        self.save()?;

        log_info!("Proxy core stopped successfully");
        Ok(())
    }

    /// Restart the proxy core
    pub fn restart(&mut self) -> Result<()> {
        log_info!("Restarting proxy core");

        let exe_name = self.core.exe_name();
        let exe_path = self.core.exe_path(&self.config_dir);

        let args = vec![
            "-f".to_string(),
            self.config_path().to_string_lossy().to_string(),
            "-ext-ctl".to_string(),
            self.web.host.clone(),
            "-ext-ui".to_string(),
            self.web.ui_name().to_string(),
            "-d".to_string(),
            self.config_dir.to_string_lossy().to_string(),
        ];

        restart(&exe_name, &exe_path, args)?;

        let _config = CrashConfig::load()?;

        self.start_time = current_timestamp();
        self.save()?;

        log_info!("Proxy core restarted successfully");
        Ok(())
    }

    /// Get the version of the installed proxy core
    pub fn get_version(&self) -> Result<String> {
        log_debug!("Getting version for core: {}", self.core.name());

        let exe_path = self.core.exe_path(&self.config_dir);

        if !exe_path.exists() {
            log_debug!("Core executable not found: {}", exe_path.display());
            return Err(CrashError::Config("Core executable not found".to_string()));
        }

        let output = execute(exe_path.to_string_lossy().as_ref(), &["-v"])?;

        // Parse version from output (format: "Mihomo version 1.19.15")
        let Some(version) = output.split_whitespace().nth(2).map(|s| s.to_string()) else {
            return Err(CrashError::Config("Core version not found".to_string()));
        };

        log_debug!("Core version: {:?}", version);
        Ok(version)
    }

    /// Install the proxy core and UI
    pub async fn install(&self, force: bool) -> Result<()> {
        log_info!("Installing proxy core and UI (force: {})", force);

        self.ensure_default_config()?;

        // Install core
        self.install_core(force).await?;

        // Install UI
        self.install_ui(force).await?;

        // Install geo databases
        self.install_geo(force).await?;

        log_info!("Installation completed successfully");
        Ok(())
    }

    /// Ensure default configuration file exists
    fn ensure_default_config(&self) -> Result<()> {
        let config_path = self.config_path();

        if config_path.exists() {
            return Ok(());
        }

        log_info!(
            "Creating default configuration file: {}",
            config_path.display()
        );

        // Create default Mihomo config
        let default_config = include_str!("../assets/mihomo.yaml");

        crate::utils::fs::atomic_write(&config_path, default_config)?;

        Ok(())
    }
    pub async fn install_core(&self, force: bool) -> Result<()> {
        let exe_path = self.core.exe_path(&self.config_dir);

        if file_exists(&exe_path) && !force {
            log_info!("Core already installed at {}", exe_path.display());
            return Ok(());
        }

        log_info!("Installing proxy core: {}", self.core.name());

        // Ensure config directory exists
        ensure_dir(&self.config_dir)?;

        // Get download URL
        let resource = self.core.repo(&self.target)?;
        let url = self
            .proxy
            .url(resource)
            .ok_or_else(|| CrashError::Download("Failed to get core download URL".to_string()))?;

        log_info!("Downloading core from: {}", url);

        // Use easy-install to download and extract
        let result = self
            .ei(
                &url,
                &self.config_dir.to_string_lossy(),
                Some(self.core.name().to_string()),
            )
            .await;

        if result.is_err() {
            return Err(CrashError::Download(
                "Failed to install core binary".to_string(),
            ));
        }

        // Verify installation
        if !file_exists(&exe_path) {
            return Err(CrashError::Download(format!(
                "Core binary not found after installation: {}",
                exe_path.display()
            )));
        }

        log_info!("Core installed successfully at {}", exe_path.display());
        Ok(())
    }

    pub async fn ei(&self, url: &str, dir: &str, alias: Option<String>) -> anyhow::Result<()> {
        easy_install::run_main(easy_install::Args {
            url: url.to_string(),
            dir: Some(dir.to_string()),
            install_only: true,
            proxy: self.proxy,
            alias,
            ..Default::default()
        })
        .await
    }
    /// Install the web UI
    pub async fn install_ui(&self, force: bool) -> Result<()> {
        let ui_dir = self.web.ui_dir(&self.config_dir);

        if ui_dir.exists() && !force {
            log_info!("UI already installed at {}", ui_dir.display());
            return Ok(());
        }

        log_info!("Installing web UI: {}", self.web.ui_name());

        // Get download URL
        let url = self.web.ui_url(&self.proxy)?;

        log_info!("Downloading UI from: {}", url);

        // Use easy-install to download and extract
        let result = self.ei(&url, &ui_dir.to_string_lossy(), None).await;

        if result.is_err() {
            return Err(CrashError::Download("Failed to install UI".to_string()));
        }

        // Verify installation
        if !ui_dir.exists() {
            return Err(CrashError::Download(format!(
                "UI directory not found after installation: {}",
                ui_dir.display()
            )));
        }

        log_info!("UI installed successfully at {}", ui_dir.display());
        Ok(())
    }

    pub async fn install_geo(&self, force: bool) -> Result<()> {
        log_info!("Installing GeoIP databases");

        for name in self.core.get_geo_files() {
            let Some(url) = Resource::Release {
                owner: "ahaoboy".to_string(),
                repo: "crash-assets".to_string(),
                tag: "nightly".to_string(),
                name: name.to_string(),
            }
            .url(&self.proxy) else {
                log_info!("Database {} not found.", name);
                continue;
            };

            let db_path = self.config_dir.join(name.replace(".tar.xz", ""));

            if file_exists(&db_path) && !force {
                log_info!("Database {} already exists", name);
                continue;
            }

            log_info!("Downloading GeoIP database: {}", name);

            if self
                .ei(&url, &self.config_dir.to_string_lossy(), None)
                .await
                .is_ok()
            {
                log_info!("Downloaded {} successfully", name);
            } else {
                log_info!("Downloaded {} error", name);
            }
        }

        log_info!("GeoIP databases installed successfully");
        Ok(())
    }
}

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
