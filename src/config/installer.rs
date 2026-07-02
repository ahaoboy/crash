// Installation, upgrade and configuration-update operations.
//
// Split out of `config/mod.rs` to keep storage/validation logic small and
// let this file focus on downloading / extracting / updating assets.

use super::CrashConfig;
use super::get_config_dir;
use super::patcher::patch_config;
use crate::cli::UpgradeRepo;
use crate::error::{CrashError, Result};
use crate::log_info;
use crate::utils::download::download_text;
use crate::utils::fs::{atomic_write, ensure_dir, file_exists};
use crate::utils::{is_url, strip_suffix};
use easy_install::{InstallConfig, ei};
use github_proxy::{Proxy, Resource};
use std::path::Path;

impl CrashConfig {
    /// Install the proxy core, web UI and geo databases.
    pub async fn install(&self, force: bool) -> Result<()> {
        log_info!("Installing proxy core and UI (force: {})", force);

        self.ensure_default_config()?;

        self.install_core(force).await?;
        self.install_ui(force).await?;
        self.install_geo(force).await?;

        log_info!("Installation completed successfully");
        Ok(())
    }

    /// Ensure the default core configuration file exists on disk.
    fn ensure_default_config(&self) -> Result<()> {
        let config_path = self.core_config_path();

        if config_path.exists() {
            return Ok(());
        }

        log_info!(
            "Creating default configuration file: {}",
            config_path.display()
        );

        let default_config = include_str!("../assets/mihomo.yaml");
        atomic_write(&config_path, default_config)?;

        Ok(())
    }

    /// Install the proxy core binary.
    pub async fn install_core(&self, force: bool) -> Result<()> {
        let exe_path = self.core.exe_path(&get_config_dir());

        if file_exists(&exe_path) && !force {
            log_info!("Core already installed at {}", exe_path.display());
            return Ok(());
        }

        log_info!("Installing proxy core: {}", self.core.name());

        ensure_dir(&get_config_dir())?;

        let resource = self.core.repo(&self.target)?;
        let url = self
            .proxy
            .url(resource)
            .ok_or_else(|| CrashError::Download("Failed to get core download URL".to_string()))?;

        log_info!("Downloading core from: {}", url);

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

        if !file_exists(&exe_path) {
            return Err(CrashError::Download(format!(
                "Core binary not found after installation: {}",
                exe_path.display()
            )));
        }

        log_info!("Core installed successfully at {}", exe_path.display());
        Ok(())
    }

    /// Build an `easy_install` config derived from this crash config.
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

    /// Install the web UI assets.
    pub async fn install_ui(&self, force: bool) -> Result<()> {
        let config_dir = get_config_dir();
        let ui_dir = self.web.ui_dir(&config_dir);

        if ui_dir.exists() && !force {
            log_info!("UI already installed at {}", ui_dir.display());
            return Ok(());
        }

        log_info!("Installing web UI: {}", self.web.ui_name());

        let url = self.web.ui_url()?;

        log_info!("Downloading UI from: {}", url);

        let result = ei(
            &url,
            &self.ei_config(
                &config_dir.to_string_lossy(),
                Some(self.web.ui_name().to_string()),
            ),
        )
        .await;

        if result.is_err() {
            return Err(CrashError::Download("Failed to install UI".to_string()));
        }

        if !ui_dir.exists() {
            return Err(CrashError::Download(format!(
                "UI directory not found after installation: {}",
                ui_dir.display()
            )));
        }

        log_info!("UI installed successfully at {}", ui_dir.display());
        Ok(())
    }

    /// Install GeoIP / geosite databases for the active core.
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

    /// Upgrade the `crash` (or `crash-assets`) binary in place.
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

    /// Update the core configuration file from the configured URL or local path.
    pub async fn update_config(&self, force: bool) -> Result<()> {
        let dest = &self.core_config_path();
        let source = &self.url;

        if source.is_empty() {
            return Err(CrashError::Config(
                "Configuration URL is empty. Please set it first with 'crash config url <url>'"
                    .to_string(),
            ));
        }

        if file_exists(dest) && !force {
            log_info!("Configuration file already exists at {}", dest.display());
            return Ok(());
        }

        log_info!("Updating configuration from: {}", source);

        let content = if is_url(source) {
            log_info!("Downloading configuration from URL: {}", source);
            download_text(source).await.map_err(|e| {
                CrashError::Config(format!("Failed to download configuration from URL: {}", e))
            })?
        } else {
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

        let patched_content = patch_config(self.core, &self.web, &content);

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
