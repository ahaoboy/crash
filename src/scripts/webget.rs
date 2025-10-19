// Network download module - corresponds to scripts/webget.sh

use crate::common::{Config, Logger, Result};
use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct Downloader {
    client: Client,
    config: Config,
    logger: Logger,
}

impl Downloader {
    pub fn new(config: Config) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            config,
            logger: Logger::new(),
        }
    }

    /// Download a file from URL
    pub fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        self.logger.info(&format!("下载文件: {}", url));

        let response = self.client.get(url).send().context("发送HTTP请求失败")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP请求失败: {}", response.status());
        }

        let bytes = response.bytes().context("读取响应数据失败")?;

        let mut file = File::create(dest).context(format!("创建文件失败: {}", dest.display()))?;
        file.write_all(&bytes).context("写入文件失败")?;

        self.logger.info("下载完成");
        Ok(())
    }

    /// Download with progress bar
    pub fn download_with_progress(&self, url: &str, dest: &Path) -> Result<()> {
        self.logger.info(&format!("下载文件: {}", url));

        let response = self.client.get(url).send().context("发送HTTP请求失败")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP请求失败: {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut file = File::create(dest).context(format!("创建文件失败: {}", dest.display()))?;
        let mut downloaded = 0u64;

        // Read response in chunks
        let bytes = response.bytes().context("读取响应数据失败")?;

        // Write in chunks to show progress
        let chunk_size = 8192;
        for chunk in bytes.chunks(chunk_size) {
            file.write_all(chunk).context("写入文件失败")?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("下载完成");
        Ok(())
    }

    /// Get core configuration from URL
    pub fn get_core_config(&self, url: &str) -> Result<String> {
        self.logger.info(&format!("获取配置: {}", url));

        let response = self.client.get(url).send().context("发送HTTP请求失败")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP请求失败: {}", response.status());
        }

        let text = response.text().context("读取响应文本失败")?;

        Ok(text)
    }

    /// Generate configuration from subscription URL
    pub fn generate_config(
        &self,
        subscription_url: &str,
        target: &str,
        rule_link: Option<&str>,
    ) -> Result<String> {
        self.logger.info("正在连接服务器获取配置文件...");

        // Get server list
        let servers_file = self.config.crash_dir.join("configs/servers.list");
        let servers_content = std::fs::read_to_string(&servers_file).unwrap_or_default();

        // Find conversion server
        let server = servers_content
            .lines()
            .filter(|l| l.starts_with("3") || l.starts_with("4"))
            .nth(0)
            .and_then(|l| l.split_whitespace().nth(2))
            .unwrap_or("https://api.v1.mk");

        // Find rule template
        let rule = rule_link.or_else(|| {
            servers_content
                .lines()
                .filter(|l| l.starts_with("5"))
                .nth(0)
                .and_then(|l| l.split_whitespace().nth(2))
        });

        // URL encode subscription
        let encoded_url = urlencoding::encode(subscription_url);

        // Build conversion URL
        let mut conversion_url = format!(
            "{}/sub?target={}&insert=true&new_name=true&scv=true&udp=true&url={}",
            server, target, encoded_url
        );

        if let Some(rule_url) = rule {
            conversion_url.push_str(&format!("&config={}", urlencoding::encode(rule_url)));
        }

        self.logger.info(&format!("链接地址: {}", conversion_url));

        // Download configuration
        let config_text = self.get_core_config(&conversion_url)?;

        // Validate configuration
        if config_text.is_empty() || config_text.len() < 100 {
            anyhow::bail!("配置文件内容无效");
        }

        self.logger.info("配置文件获取成功");
        Ok(config_text)
    }

    /// Download and extract core binary
    pub fn download_core(&self, core_type: &str, arch: &str) -> Result<()> {
        self.logger
            .info(&format!("下载内核: {} ({})", core_type, arch));

        let update_url = self
            .config
            .extra
            .get("update_url")
            .map(|s| s.as_str())
            .unwrap_or("https://fastly.jsdelivr.net/gh/juewuy/ShellCrash@master");

        let core_url = format!(
            "{}/bin/{}/clash-linux-{}.tar.gz",
            update_url, core_type, arch
        );

        let tmp_file = self.config.tmp_dir.join("core_new.tar.gz");
        self.download_with_progress(&core_url, &tmp_file)?;

        // Extract
        self.logger.info("解压内核文件...");
        let tar_gz = File::open(&tmp_file)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&self.config.tmp_dir)?;

        // Find and move core file
        for entry in std::fs::read_dir(&self.config.tmp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_str().unwrap();
                if name.contains("clash") || name.contains("mihomo") || name.contains("singbox") {
                    let dest = self.config.tmp_dir.join("CrashCore");
                    std::fs::rename(&path, &dest)?;

                    // Make executable
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = std::fs::metadata(&dest)?.permissions();
                        perms.set_mode(0o755);
                        std::fs::set_permissions(&dest, perms)?;
                    }

                    // Create tar.gz for storage
                    let core_tar = self.config.crash_dir.join("CrashCore.tar.gz");
                    let tar_gz = File::create(&core_tar)?;
                    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
                    let mut tar = tar::Builder::new(enc);
                    tar.append_path_with_name(&dest, "CrashCore")?;
                    tar.finish()?;

                    self.logger.info("内核下载完成");
                    return Ok(());
                }
            }
        }

        anyhow::bail!("未找到内核文件")
    }

    /// Download GeoIP database
    pub fn download_geoip(&self, db_type: &str) -> Result<()> {
        self.logger.info(&format!("下载 GeoIP 数据库: {}", db_type));

        let update_url = self
            .config
            .extra
            .get("update_url")
            .map(|s| s.as_str())
            .unwrap_or("https://fastly.jsdelivr.net/gh/juewuy/ShellCrash@master");

        let db_url = format!("{}/bin/geodata/{}", update_url, db_type);
        let dest = self.config.crash_dir.join(db_type);

        self.download_with_progress(&db_url, &dest)?;

        self.logger.info("GeoIP 数据库下载完成");
        Ok(())
    }

    /// Update subscription
    pub fn update_subscription(&self) -> Result<()> {
        self.logger.info("更新订阅...");

        // Get subscription URL from config
        let sub_url = self
            .config
            .extra
            .get("Url")
            .or_else(|| self.config.extra.get("Https"))
            .context("未配置订阅地址")?;

        // Get target type
        let target = self
            .config
            .extra
            .get("target")
            .map(|s| s.as_str())
            .unwrap_or("clash");

        // Generate config
        let config_text = self.generate_config(sub_url, target, None)?;

        // Save config
        let config_file = if target == "singbox" {
            self.config.crash_dir.join("jsons/config.json")
        } else {
            self.config.crash_dir.join("yamls/config.yaml")
        };

        std::fs::write(&config_file, config_text).context("保存配置文件失败")?;

        self.logger.info("订阅更新完成");
        Ok(())
    }

    /// Update core
    pub fn update_core(&self) -> Result<()> {
        self.logger.info("更新内核...");

        // Get core type and arch from config
        let core_type = self
            .config
            .extra
            .get("crashcore")
            .map(|s| s.as_str())
            .unwrap_or("clash");

        let arch = self
            .config
            .extra
            .get("cpucore")
            .map(|s| s.as_str())
            .unwrap_or("amd64");

        // Download core
        self.download_core(core_type, arch)?;

        self.logger.info("内核更新完成");
        Ok(())
    }

    /// Update scripts
    pub fn update_scripts(&self) -> Result<()> {
        self.logger.info("更新脚本...");

        let update_url = self
            .config
            .extra
            .get("update_url")
            .map(|s| s.as_str())
            .unwrap_or("https://fastly.jsdelivr.net/gh/juewuy/ShellCrash@master");

        let script_url = format!("{}/bin/update.tar.gz", update_url);
        let tmp_file = self.config.tmp_dir.join("update.tar.gz");

        self.download_with_progress(&script_url, &tmp_file)?;

        // Extract
        self.logger.info("解压脚本文件...");
        let tar_gz = File::open(&tmp_file).context("打开压缩文件失败")?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(&self.config.crash_dir)
            .context("解压文件失败")?;

        // Clean up
        std::fs::remove_file(tmp_file).context("删除临时文件失败")?;

        self.logger.info("脚本更新完成");
        Ok(())
    }

    /// Update GeoIP database
    pub fn update_geoip(&self) -> Result<()> {
        self.logger.info("更新 GeoIP 数据库...");

        // Update all GeoIP databases
        let databases = vec![
            "Country.mmdb",
            "GeoSite.dat",
            "geoip.db",
            "geosite.db",
            "geosite-cn.mrs",
            "geoip-cn.srs",
            "geosite-cn.srs",
        ];

        for db in databases {
            let db_path = self.config.crash_dir.join(db);
            if db_path.exists()
                && let Err(e) = self.download_geoip(db)
            {
                self.logger.warn(&format!("更新 {} 失败: {}", db, e));
            }
        }

        self.logger.info("GeoIP 数据库更新完成");
        Ok(())
    }
}
