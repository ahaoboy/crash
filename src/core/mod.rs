// Core management module for proxy cores

use crate::config::CrashConfig;
use crate::download::DownloadManager;
use crate::error::{CrashError, Result};
use crate::platform::command::CommandExecutor;
use crate::platform::process::{get_pid, is_running};
use crate::process::{restart, start, stop};
use crate::utils::time::current_timestamp;
use crate::{log_debug, log_info};

pub mod geo;
pub mod installer;
pub mod updater;

pub use installer::Installer;
pub use updater::Updater;

/// Core manager for managing proxy core lifecycle
pub struct CoreManager {
    installer: Installer,
    updater: Updater,
}

impl CoreManager {
    /// Create a new core manager
    pub fn new() -> Self {
        let download_manager = DownloadManager::default();

        Self {
            installer: Installer::new(download_manager.clone()),
            updater: Updater::new(download_manager),
        }
    }

    /// Start the proxy core
    pub fn start(&mut self) -> Result<()> {
        let mut config = CrashConfig::load()?;

        log_info!("Starting proxy core: {}", config.core.name());

        let exe_path = config.core.exe_path(&config.config_dir);

        if !exe_path.exists() {
            return Err(CrashError::Process(format!(
                "Core executable not found: {}. Please run 'install' first.",
                exe_path.display()
            )));
        }

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

        start(&exe_path, args)?;

        config.start_time = current_timestamp();
        config.save()?;

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

        drop(config); // Release read lock before acquiring write lock

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

        let output = CommandExecutor.execute(exe_path.to_string_lossy().as_ref(), &["-v"])?;

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

        let config_clone = {
            let config = CrashConfig::load()?;

            // Ensure default config file exists
            self.ensure_default_config(&config)?;

            config.clone()
        }; // Lock is dropped here

        // Install core
        self.installer.install_core(&config_clone, force).await?;

        // Install UI
        self.installer.install_ui(&config_clone, force).await?;

        // Install geo databases
        self.installer
            .install_geo_databases(&config_clone, force)
            .await?;

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

    /// Get reference to the installer
    pub fn installer(&self) -> &Installer {
        &self.installer
    }

    /// Get reference to the updater
    pub fn updater(&self) -> &Updater {
        &self.updater
    }

    /// Check if the process is running
    pub fn is_running(&self, exe_name: &str) -> bool {
        is_running(exe_name)
    }

    /// Get the process ID
    pub fn get_pid(&self, exe_name: &str) -> Result<u32> {
        get_pid(exe_name)
    }
}
