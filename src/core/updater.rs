// Updater component for configuration and geo databases

use crate::config::CrashConfig;
use crate::download::download_file;
use crate::error::Result;
use crate::utils::fs::file_exists;
use crate::{log_info, log_warn};

/// Update configuration file from URL
pub async fn update_config(force: bool) -> Result<()> {
    let config = CrashConfig::load()?;
    let dest = &config.config_path();
    let url = &config.url;
    if file_exists(dest) && !force {
        log_info!("Configuration file already exists at {}", dest.display());
        return Ok(());
    }

    log_info!("Updating configuration from: {}", url);

    download_file(url, dest).await?;

    let s = std::fs::read_to_string(dest)?;
    let patch_s = config.core.patch_config(&s);
    if s != patch_s {
        std::fs::write(dest, patch_s)?;
    }
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
