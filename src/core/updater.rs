// Updater component for configuration and geo databases

use crate::config::CrashConfig;
use crate::download::{download_file, download_text};
use crate::error::{CrashError, Result};
use crate::utils::fs::file_exists;
use crate::{log_info, log_warn};
use std::path::Path;

/// Check if a string is a valid URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Update configuration file from URL or local file
pub async fn update_config(force: bool) -> Result<()> {
    let config = CrashConfig::load()?;
    let dest = &config.core_config_path();
    let source = &config.url;

    if file_exists(dest) && !force {
        log_info!("Configuration file already exists at {}", dest.display());
        return Ok(());
    }

    log_info!("Updating configuration from: {}", source);

    // Get configuration content from URL or local file
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
    let patched_content = config.patch_config(&content);

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

/// Update GeoIP databases
pub async fn update_geo(config: &CrashConfig, force: bool) -> Result<()> {
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

        download_file(&url, &db_path).await?;

        log_info!("Updated {} successfully", db_name);
    }

    log_info!("GeoIP databases updated successfully");
    Ok(())
}
