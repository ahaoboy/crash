// Configuration management module

use crate::config::core::Core;
use crate::download::download_file;
use crate::error::{CrashError, Result};
use crate::platform::command::execute;
use crate::platform::process::get_pid;
use crate::process::{restart, start, stop};
use crate::utils::fs::{atomic_write, ensure_dir};
use crate::utils::{current_timestamp, file_exists};
use crate::{log_debug, log_info, log_warn};
use github_proxy::Proxy;
use guess_target::Target;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod core;
pub mod web;

pub use web::WebConfig;

const APP_CONFIG_DIR: &str = ".crash_config";
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

    pub fn start(&mut self) -> Result<()> {
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
            return Ok(());
        }

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

        start(&exe_path, args)?;

        self.start_time = current_timestamp();
        self.save()?;

        log_info!("Proxy core started successfully");
        Ok(())
    }

    /// Stop the proxy core
    pub fn stop(&mut self) -> Result<()> {
        let mut config = CrashConfig::load()?;

        log_info!("Stopping proxy core: {}", config.core.name());

        let exe_name = config.core.exe_name();
        stop(&exe_name)?;

        config.start_time = 0;
        config.save()?;

        log_info!("Proxy core stopped successfully");
        Ok(())
    }

    /// Restart the proxy core
    pub fn restart(&mut self) -> Result<()> {
        log_info!("Restarting proxy core");

        let config = CrashConfig::load()?;
        let exe_name = config.core.exe_name();
        let exe_path = config.core.exe_path(&config.config_dir);

        let args = vec![
            "-f".to_string(),
            config.config_path().to_string_lossy().to_string(),
            "-ext-ctl".to_string(),
            config.web.host.clone(),
            "-ext-ui".to_string(),
            config.web.ui_name().to_string(),
            "-d".to_string(),
            config.config_dir.to_string_lossy().to_string(),
        ];

        restart(&exe_name, &exe_path, args)?;

        let mut config = CrashConfig::load()?;

        config.start_time = current_timestamp();
        config.save()?;

        log_info!("Proxy core restarted successfully");
        Ok(())
    }

    /// Get the version of the installed proxy core
    pub fn get_version(&self) -> Result<String> {
        let config = CrashConfig::load()?;

        log_debug!("Getting version for core: {}", config.core.name());

        let exe_path = config.core.exe_path(&config.config_dir);

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

        let config = CrashConfig::load()?;
        self.ensure_default_config(&config)?;

        // Install core
        self.install_core(&config, force).await?;

        // Install UI
        self.install_ui(&config, force).await?;

        // Install geo databases
        self.install_geo_databases(&config, force).await?;

        log_info!("Installation completed successfully");
        Ok(())
    }

    /// Ensure default configuration file exists
    fn ensure_default_config(&self, config: &CrashConfig) -> Result<()> {
        let config_path = config.config_path();

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
    pub async fn install_core(&self, config: &CrashConfig, force: bool) -> Result<()> {
        let exe_path = config.core.exe_path(&config.config_dir);

        if file_exists(&exe_path) && !force {
            log_info!("Core already installed at {}", exe_path.display());
            return Ok(());
        }

        log_info!("Installing proxy core: {}", config.core.name());

        // Ensure config directory exists
        ensure_dir(&config.config_dir)?;

        // Get download URL
        let resource = config.core.repo(&config.target)?;
        let url = config
            .proxy
            .url(resource)
            .ok_or_else(|| CrashError::Download("Failed to get core download URL".to_string()))?;

        log_info!("Downloading core from: {}", url);

        // Use easy-install to download and extract
        let result = easy_install::run_main(easy_install::Args {
            url,
            dir: Some(config.config_dir.to_string_lossy().to_string()),
            install_only: true,
            name: vec![],
            alias: Some(config.core.name().to_string()),
            target: None,
            retry: 3,
            proxy: config.proxy,
            timeout: 600,
        })
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

    /// Install the web UI
    pub async fn install_ui(&self, config: &CrashConfig, force: bool) -> Result<()> {
        let ui_dir = config.web.ui_dir(&config.config_dir);

        if ui_dir.exists() && !force {
            log_info!("UI already installed at {}", ui_dir.display());
            return Ok(());
        }

        log_info!("Installing web UI: {}", config.web.ui_name());

        // Get download URL
        let url = config.web.ui_url(&config.proxy)?;

        log_info!("Downloading UI from: {}", url);

        // Use easy-install to download and extract
        let result = easy_install::run_main(easy_install::Args {
            url,
            dir: Some(ui_dir.to_string_lossy().to_string()),
            install_only: true,
            name: vec![],
            alias: None,
            target: None,
            retry: 3,
            proxy: config.proxy,
            timeout: 600,
        })
        .await;

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

    /// Install GeoIP databases
    pub async fn install_geo_databases(&self, config: &CrashConfig, force: bool) -> Result<()> {
        log_info!("Installing GeoIP databases");

        use crate::config::core::Core;

        let databases = match config.core {
            Core::Mihomo => vec!["geoip.metadb", "geoip.dat", "geosite.dat"],
            Core::Clash => vec![
                "china_ip_list.txt",
                "china_ipv6_list.txt",
                "cn_mini.mmdb",
                "Country.mmdb",
                "geoip_cn.db",
                "geosite.dat",
                "geosite_cn.db",
                "mrs_geosite_cn.mrs",
                "srs_geoip_cn.srs",
                "srs_geosite_cn.srs",
            ],
            Core::Singbox => {
                log_warn!("GeoIP database installation not implemented for Singbox");
                return Ok(());
            }
        };

        for db_name in databases {
            let db_path = config.config_dir.join(db_name);

            if file_exists(&db_path) && !force {
                log_info!("Database {} already exists", db_name);
                continue;
            }

            log_info!("Downloading GeoIP database: {}", db_name);

            let url = self.get_geo_database_url(config, db_name)?;

            download_file(&url, &db_path).await?;

            log_info!("Downloaded {} successfully", db_name);
        }

        log_info!("GeoIP databases installed successfully");
        Ok(())
    }

    /// Get the download URL for a GeoIP database
    fn get_geo_database_url(&self, config: &CrashConfig, db_name: &str) -> Result<String> {
        use crate::config::core::Core;
        use github_proxy::Resource;

        let resource = match config.core {
            Core::Mihomo => Resource::Release {
                owner: "MetaCubeX".to_string(),
                repo: "meta-rules-dat".to_string(),
                tag: "latest".to_string(),
                name: db_name.to_string(),
            },
            Core::Clash => Resource::File {
                owner: "juewuy".to_string(),
                repo: "ShellCrash".to_string(),
                reference: "master".to_string(),
                path: format!("bin/geodata/{}", db_name),
            },
            Core::Singbox => {
                return Err(CrashError::Config(
                    "GeoIP databases not supported for Singbox".to_string(),
                ));
            }
        };

        config
            .proxy
            .url(resource)
            .ok_or_else(|| CrashError::Download("Failed to get geo database URL".to_string()))
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
pub fn get_log_dir() -> PathBuf {
    get_config_dir().join(APP_LOG_DIR)
}
