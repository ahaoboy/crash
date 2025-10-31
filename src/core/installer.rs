// Installer component for proxy cores and UI

use crate::config::CrashConfig;
use crate::download::DownloadManager;
use crate::error::{CrashError, Result};
use crate::utils::fs::{ensure_dir, file_exists};
use crate::{log_info, log_warn};

/// Installer for proxy cores and UI components
#[derive(Clone)]
pub struct Installer {
    download_manager: DownloadManager,
}

impl Installer {
    /// Create a new installer
    pub fn new(download_manager: DownloadManager) -> Self {
        Self { download_manager }
    }

    /// Install the proxy core binary
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

            self.download_manager.download_file(&url, &db_path).await?;

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
