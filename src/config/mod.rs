// Configuration management module

use crate::cli::UpgradeRepo;
use crate::config::core::Core;
use crate::error::{CrashError, Result};
use crate::utils::command::execute;
use crate::utils::download::download_text;
use crate::utils::fs::{atomic_write, ensure_dir};
use crate::utils::process::get_pid;
use crate::utils::process::{start, stop};
use crate::utils::{current_timestamp, file_exists, get_dir_size, is_url, strip_suffix};
use crate::{log_debug, log_info};
use easy_install::{InstallConfig, ei};
use github_proxy::{Proxy, Resource};
use guess_target::Target;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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
    // pub config_dir: PathBuf,
    pub start_time: u64,
    pub core: Core,
    pub proxy: Proxy,
    pub target: Target,
    pub web: WebConfig,
    pub url: String,
    pub max_runtime_hours: u64,

    #[serde(default)]
    pub stop_force: bool,
}

impl Default for CrashConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            // config_dir: get_config_dir(),
            start_time: 0,
            core: Core::default(),
            proxy: Proxy::default(),
            target: Target::default(),
            web: WebConfig::default(),
            url: String::new(),
            stop_force: false,
            max_runtime_hours: 0,
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
        ensure_dir(&get_config_dir())?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CrashError::Config(format!("Failed to serialize config: {}", e)))?;

        atomic_write(&config_path, &json)?;

        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate config directory path
        if get_config_dir().to_str().is_none() {
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
    pub fn core_config_path(&self) -> PathBuf {
        get_config_dir().join(self.core.config_file_name())
    }

    pub fn start(&mut self, force: bool) -> Result<()> {
        log_info!("Starting proxy core: {}", self.core.name());

        if self.stop_force {
            if !force {
                return Err(CrashError::Process(
                    "Skip starting proxy core: run 'crash start -f' instead.".to_string(),
                ));
            } else {
                self.stop_force = false;
                self.save()?;
            }
        }

        let exe_path = self.core.exe_path(&get_config_dir());

        if !exe_path.exists() {
            return Err(CrashError::Process(format!(
                "Core executable not found: {}. Please run 'install' first.",
                exe_path.display()
            )));
        }

        if get_pid(&self.core.exe_name()).is_ok() {
            log_info!("Skip starting proxy core: {}", self.core.name());

            // Check if max runtime has been exceeded
            if self.max_runtime_hours > 0 && self.start_time > 0 {
                let current_time = current_timestamp();
                let runtime_seconds = current_time.saturating_sub(self.start_time);
                let max_runtime_seconds = self.max_runtime_hours * 3600;

                if runtime_seconds >= max_runtime_seconds {
                    log_info!(
                        "Process has been running for {} hours, exceeding max runtime of {} hours. Restarting...",
                        runtime_seconds / 3600,
                        self.max_runtime_hours
                    );
                    self.stop(false)?;
                    // Continue to start the process below
                } else if force {
                    self.stop(false)?;
                } else {
                    return Ok(());
                }
            } else if force {
                self.stop(false)?;
            } else {
                return Ok(());
            }
        }

        let args = match self.core {
            Core::Mihomo | Core::Clash => vec![
                "-f".to_string(),
                self.core_config_path().to_string_lossy().to_string(),
                "-ext-ctl".to_string(),
                self.web.host.clone(),
                "-ext-ui".to_string(),
                self.web.ui_name().to_string(),
                "-d".to_string(),
                get_config_dir().to_string_lossy().to_string(),
            ],
            Core::Singbox => {
                vec![
                    "run".to_string(),
                    "-c".to_string(),
                    self.core_config_path().to_string_lossy().to_string(),
                    "-D".to_string(),
                    get_config_dir().to_string_lossy().to_string(),
                ]
            }
        };

        start(&exe_path, args, self.core.envs())?;

        self.start_time = current_timestamp();
        self.save()?;

        log_info!("Proxy core started successfully");
        Ok(())
    }

    /// Stop the proxy core
    pub fn stop(&mut self, force: bool) -> Result<()> {
        log_info!("Stopping proxy core: {}", self.core.name());

        self.stop_force = force;
        let exe_name = self.core.exe_name();
        stop(&exe_name)?;

        self.start_time = 0;
        self.save()?;

        log_info!("Proxy core stopped successfully");
        Ok(())
    }

    /// Get the version of the installed proxy core
    pub fn get_version(&self) -> Result<String> {
        log_debug!("Getting version for core: {}", self.core.name());

        let exe_path = self.core.exe_path(&get_config_dir());

        if !exe_path.exists() {
            log_debug!("Core executable not found: {}", exe_path.display());
            return Err(CrashError::Config("Core executable not found".to_string()));
        }

        let args = match self.core {
            Core::Mihomo | Core::Clash => &["-v"],
            Core::Singbox => &["version"],
        };
        let output = execute(exe_path.to_string_lossy().as_ref(), args)?;

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
        let config_path = self.core_config_path();

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
        let exe_path = self.core.exe_path(&get_config_dir());

        if file_exists(&exe_path) && !force {
            log_info!("Core already installed at {}", exe_path.display());
            return Ok(());
        }

        log_info!("Installing proxy core: {}", self.core.name());

        // Ensure config directory exists
        ensure_dir(&get_config_dir())?;

        // Get download URL
        let resource = self.core.repo(&self.target)?;
        let url = self
            .proxy
            .url(resource)
            .ok_or_else(|| CrashError::Download("Failed to get core download URL".to_string()))?;

        log_info!("Downloading core from: {}", url);

        // Use easy-install to download and extract
        let result = ei(
            &url,
            &self.ei_config(
                &get_config_dir().to_string_lossy(),
                Some(self.core.name().to_string()),
            ),
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

    pub fn ei_config(&self, dir: &str, alias: Option<String>) -> InstallConfig {
        easy_install::InstallConfig {
            dir: Some(dir.to_string()),
            install_only: true,
            proxy: self.proxy,
            alias,
            target: Some(self.target),
            ..Default::default()
        }
    }
    /// Install the web UI
    pub async fn install_ui(&self, force: bool) -> Result<()> {
        let config_dir = get_config_dir();
        let ui_dir = self.web.ui_dir(&config_dir);

        if ui_dir.exists() && !force {
            log_info!("UI already installed at {}", ui_dir.display());
            return Ok(());
        }

        log_info!("Installing web UI: {}", self.web.ui_name());

        // Get download URL
        let url = self.web.ui_url()?;

        log_info!("Downloading UI from: {}", url);

        // Use easy-install to download and extract
        let result = ei(
            &url,
            &self.ei_config(
                &(config_dir.to_string_lossy()),
                Some(self.web.ui_name().to_string()),
            ),
        )
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

    pub async fn install_geo(&self, force: bool) -> Result<()> {
        log_info!("Installing GeoIP databases");

        for name in self.core.get_geo_files() {
            let Some(url) = Resource::File {
                owner: "ahaoboy".to_string(),
                repo: "crash-assets".to_string(),
                reference: "main".to_string(),
                path: name.to_string(),
            }
            .url(&Proxy::Github) else {
                log_info!("Database {} not found.", name);
                continue;
            };

            let db_path = get_config_dir().join(strip_suffix(name));

            if file_exists(&db_path) && !force {
                log_info!("Database {} already exists", name);
                continue;
            }

            log_info!("Downloading GeoIP database: {}", name);

            if ei(
                &url,
                &self.ei_config(&get_config_dir().to_string_lossy(), None),
            )
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

    pub fn patch_config(&self, config: &str) -> String {
        match self.core {
            Core::Mihomo => {
                let has_tun = config.lines().any(|i| i.starts_with("tun"));
                if has_tun {
                    config.to_string()
                } else {
                    format!(
                        "{}\n{}",
                        config,
                        r#"
# Crash default tun
tun:
  enable: true
  device: Meta
  stack: gVisor
  dns-hijack:
    - 0.0.0.0:53
  auto-route: true
  auto-detect-interface: true
  gso-max-size: 65536
  file-descriptor: 0
  recvmsgx: true
"#
                    )
                }
            }
            Core::Clash => config.replace("- 'RULE-SET,", "#- 'RULE-SET,").to_string(),
            Core::Singbox => {
                let Ok(mut v) = serde_json::from_str::<Value>(config) else {
                    return config.to_string();
                };

                // FATAL[0000] decode config at ./Singbox.json: outbounds[5].server_port: json: cannot unmarshal string into Go value of type uint16
                if let Some(outbounds) = v.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
                    for item in outbounds {
                        if let Some(port_val) = item.get_mut("server_port")
                            && let Some(port_str) = port_val.as_str()
                            && let Ok(port_num) = port_str.parse::<u64>()
                        {
                            *port_val = json!(port_num);
                        }
                    }
                }

                fn merge_json(dst: &mut Value, src: &Value) {
                    if let (Value::Object(dst_map), Value::Object(src_map)) = (dst, src) {
                        for (k, v) in src_map {
                            match dst_map.get_mut(k) {
                                Some(dst_v) => merge_json(dst_v, v),
                                None => {
                                    dst_map.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                }

                let ui = self.web.ui.to_string();
                let secret = self.web.secret.to_string();
                // clash_api
                let patch = json!({
                    "experimental": {
                        "cache_file": {
                            "enabled": true
                        },
                        "clash_api": {
                            "external_controller": ":9090",
                            "external_ui": ui,
                            "secret": secret
                        }
                    }
                });
                merge_json(&mut v, &patch);

                serde_json::to_string_pretty(&v).unwrap_or(config.to_string())
            }
        }
    }

    pub fn get_size(&self) -> u64 {
        get_dir_size(&get_config_dir())
    }

    pub async fn upgrade(&self, repo: UpgradeRepo) -> Result<()> {
        let exe = std::env::current_exe()?;
        let dir = exe
            .parent()
            .ok_or_else(|| CrashError::Download("crash dir not found".to_string()))?;
        let dir = &dir.to_string_lossy();
        let url = match repo {
            UpgradeRepo::Crash => "ahaoboy/crash",
            UpgradeRepo::CrashAssets => "ahaoboy/crash-assets",
        };
        ei(
            url,
            &InstallConfig {
                name: vec!["crash".to_string()],
                upx: repo == UpgradeRepo::Crash,
                ..self.ei_config(dir, Some("crash".to_string()))
            },
        )
        .await
        .map_err(|e| CrashError::Download(e.to_string()))?;

        Ok(())
    }

    pub async fn update_config(&self, force: bool) -> Result<()> {
        let dest = &self.core_config_path();
        let source = &self.url;

        if source.is_empty() {
            return Err(CrashError::Config(
                "Configuration URL is empty. Please set it first with 'crash config url <url>'".to_string(),
            ));
        }

        if file_exists(dest) && !force {
            log_info!("Configuration file already exists at {}", dest.display());
            return Ok(());
        }

        log_info!("Updating configuration from: {}", source);

        let content = if is_url(source) {
            // Download from URL
            log_info!("Downloading configuration from URL: {}", source);
            download_text(source).await.map_err(|e| {
                CrashError::Config(format!("Failed to download configuration from URL: {}", e))
            })?
        } else {
            // Read from local file
            let source_path = Path::new(source);
            if !source_path.exists() {
                return Err(CrashError::Config(format!(
                    "Configuration source not found: {} (not a valid URL or local file)",
                    source
                )));
            }

            log_info!("Reading configuration from local file: {}", source);
            std::fs::read_to_string(source_path).map_err(|e| {
                CrashError::Config(format!("Failed to read local configuration file: {}", e))
            })?
        };

        // Apply patches to configuration
        let patched_content = self.patch_config(&content);

        // Write to destination
        std::fs::write(dest, patched_content).map_err(|e| {
            CrashError::Config(format!(
                "Failed to write configuration to {}: {}",
                dest.display(),
                e
            ))
        })?;

        log_info!("Configuration updated successfully");
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
