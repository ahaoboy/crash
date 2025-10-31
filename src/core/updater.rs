// Updater component for configuration and geo databases

use crate::config::CrashConfig;
use crate::download::DownloadManager;
use crate::error::Result;
use crate::utils::fs::file_exists;
use crate::{log_info, log_warn};
use std::path::Path;

/// Updater for configuration files and geo databases
#[derive(Clone)]
pub struct Updater {
    download_manager: DownloadManager,
}

impl Updater {
    /// Create a new updater
    pub fn new(download_manager: DownloadManager) -> Self {
        Self { download_manager }
    }

    /// Update configuration file from URL
    pub async fn update_config(&self, url: &str, dest: &Path, force: bool) -> Result<()> {
        if file_exists(dest) && !force {
            log_info!("Configuration file already exists at {}", dest.display());
            return Ok(());
        }

        log_info!("Updating configuration from: {}", url);

        self.download_manager.download_file(url, dest).await?;

        log_info!("Configuration updated successfully");
        Ok(())
    }

    /// Update GeoIP databases
    pub async fn update_geo(&self, config: &CrashConfig, force: bool) -> Result<()> {
        log_info!("Updating GeoIP databases (force: {})", force);

        use crate::config::core::Core;
        use github_proxy::Resource;

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
                log_warn!("GeoIP database update not implemented for Singbox");
                return Ok(());
            }
        };

        for db_name in databases {
            let db_path = config.config_dir.join(db_name);

            if file_exists(&db_path) && !force {
                log_info!("Database {} already exists, skipping", db_name);
                continue;
            }

            log_info!("Updating GeoIP database: {}", db_name);

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
                Core::Singbox => continue,
            };

            let url = config.proxy.url(resource).ok_or_else(|| {
                crate::error::CrashError::Download("Failed to get geo database URL".to_string())
            })?;

            self.download_manager.download_file(&url, &db_path).await?;

            log_info!("Updated {} successfully", db_name);
        }

        log_info!("GeoIP databases updated successfully");
        Ok(())
    }
}
