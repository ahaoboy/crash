// Service management - corresponds to scripts/start.sh

use crate::common::{Config, Logger, Result, ShellCrashError, ShellExecutor};
use crate::scripts::menu::ServiceStatus;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub struct ServiceManager {
    config: Config,
    shell: ShellExecutor,
    logger: Logger,
}

impl ServiceManager {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            shell: ShellExecutor::new(),
            logger: Logger::new(),
        }
    }

    /// Start the service
    pub fn start(&self) -> Result<()> {
        self.logger.info("正在启动服务...");

        // Check if already running
        if let ServiceStatus::Running { .. } = self.get_status() {
            self.logger.warn("服务已在运行，将先停止服务");
            self.stop()?;
        }

        // Prepare configuration
        self.prepare_config()?;

        // Start core process
        self.start_core()?;

        // Setup firewall
        if self.config.firewall.redir_mod != "纯净模式" {
            self.setup_firewall()?;
        }

        // Mark start time
        let tmp_dir = self.config.tmp_dir.join("crash_start_time");
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        fs::write(tmp_dir, start_time.to_string())?;

        self.logger.info("服务启动成功！");
        Ok(())
    }

    /// Stop the service
    pub fn stop(&self) -> Result<()> {
        self.logger.info("正在停止服务...");

        // Kill CrashCore process
        if let Ok(output) = self.shell.execute("pidof CrashCore") {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.split_whitespace() {
                let _ = self.shell.execute(&format!("kill {}", pid));
            }
        }

        // Clean up firewall rules
        self.cleanup_firewall()?;

        // Wait for process to stop
        std::thread::sleep(Duration::from_secs(1));

        self.logger.info("服务已停止");
        Ok(())
    }

    /// Restart the service
    pub fn restart(&self) -> Result<()> {
        self.logger.info("正在重启服务...");
        self.stop()?;
        std::thread::sleep(Duration::from_secs(2));
        self.start()?;
        Ok(())
    }

    /// Get service status
    pub fn get_status(&self) -> ServiceStatus {
        if let Ok(output) = self.shell.execute("pidof CrashCore") {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = pid_str.split_whitespace().last().unwrap_or("").parse::<u32>() {
                let memory = self
                    .shell
                    .execute(&format!(
                        "cat /proc/{}/status | grep VmRSS | awk '{{print $2}}'",
                        pid
                    ))
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .trim()
                            .parse::<u64>()
                            .ok()
                    })
                    .unwrap_or(0);

                let uptime = self.get_uptime();

                return ServiceStatus::Running {
                    pid,
                    uptime,
                    memory,
                    mode: self.config.firewall.redir_mod.clone(),
                };
            }
        }

        ServiceStatus::Stopped
    }

    /// Prepare configuration files
    fn prepare_config(&self) -> Result<()> {
        let tmp_dir = &self.config.tmp_dir;
        fs::create_dir_all(tmp_dir)?;

        // Check core file exists
        let core_path = tmp_dir.join("CrashCore");
        if !core_path.exists() {
            // Try to extract from tar.gz
            let core_tar = self.config.crash_dir.join("CrashCore.tar.gz");
            if core_tar.exists() {
                self.shell.execute(&format!(
                    "tar -zxf {} -C {}",
                    core_tar.display(),
                    tmp_dir.display()
                ))?;
            } else {
                return Err(ShellCrashError::PathNotFound(
                    "找不到内核文件".to_string(),
                ).into());
            }
        }

        // Make core executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&core_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&core_path, perms)?;
        }

        // Check config file
        let config_file = if self.config.core.crashcore == "singbox"
            || self.config.core.crashcore == "singboxp"
        {
            self.config.crash_dir.join("jsons/config.json")
        } else {
            self.config.crash_dir.join("yamls/config.yaml")
        };

        if !config_file.exists() {
            return Err(ShellCrashError::ConfigError(
                "找不到配置文件".to_string(),
            ).into());
        }

        Ok(())
    }

    /// Start core process
    fn start_core(&self) -> Result<()> {
        let tmp_dir = &self.config.tmp_dir;
        let bin_dir = &self.config.crash_dir;

        let command = if self.config.core.crashcore == "singbox"
            || self.config.core.crashcore == "singboxp"
        {
            format!(
                "{}/CrashCore run -D {} -C {}/jsons",
                tmp_dir.display(),
                bin_dir.display(),
                tmp_dir.display()
            )
        } else {
            format!(
                "{}/CrashCore -d {} -f {}/config.yaml",
                tmp_dir.display(),
                bin_dir.display(),
                tmp_dir.display()
            )
        };

        // Start in background
        self.shell
            .execute(&format!("nohup {} > /dev/null 2>&1 &", command))?;

        // Wait for startup
        std::thread::sleep(Duration::from_secs(2));

        // Verify started
        if let ServiceStatus::Stopped = self.get_status() {
            return Err(ShellCrashError::ServiceNotRunning.into());
        }

        Ok(())
    }

    /// Setup firewall rules
    pub fn setup_firewall(&self) -> Result<()> {
        self.logger.info("设置防火墙规则...");

        let firewall_mod = &self.config.firewall.firewall_mod;
        let redir_port = self.config.ports.redir_port;
        let dns_port = self.config.ports.dns_port;

        if firewall_mod == "iptables" {
            self.setup_iptables(redir_port, dns_port)?;
        } else if firewall_mod == "nftables" {
            self.setup_nftables(redir_port, dns_port)?;
        }

        Ok(())
    }

    fn setup_iptables(&self, redir_port: u16, dns_port: u16) -> Result<()> {
        // Create SHELLCRASH chain
        let _ = self.shell.execute("iptables -t nat -N SHELLCRASH");

        // Redirect DNS
        self.shell.execute(&format!(
            "iptables -t nat -A SHELLCRASH -p udp --dport 53 -j REDIRECT --to-ports {}",
            dns_port
        ))?;

        // Redirect TCP traffic
        self.shell.execute(&format!(
            "iptables -t nat -A SHELLCRASH -p tcp -j REDIRECT --to-ports {}",
            redir_port
        ))?;

        // Apply to PREROUTING
        self.shell
            .execute("iptables -t nat -A PREROUTING -j SHELLCRASH")?;

        Ok(())
    }

    fn setup_nftables(&self, redir_port: u16, dns_port: u16) -> Result<()> {
        // Create table and chain
        self.shell
            .execute("nft add table inet shellcrash 2>/dev/null || true")?;
        self.shell
            .execute("nft add chain inet shellcrash prerouting { type nat hook prerouting priority -100 \\; }")?;

        // Redirect DNS
        self.shell.execute(&format!(
            "nft add rule inet shellcrash prerouting udp dport 53 redirect to :{}",
            dns_port
        ))?;

        // Redirect TCP
        self.shell.execute(&format!(
            "nft add rule inet shellcrash prerouting tcp dport != 22 redirect to :{}",
            redir_port
        ))?;

        Ok(())
    }

    /// Cleanup firewall rules
    fn cleanup_firewall(&self) -> Result<()> {
        self.logger.info("清理防火墙规则...");

        let firewall_mod = &self.config.firewall.firewall_mod;

        if firewall_mod == "iptables" {
            let _ = self
                .shell
                .execute("iptables -t nat -D PREROUTING -j SHELLCRASH");
            let _ = self.shell.execute("iptables -t nat -F SHELLCRASH");
            let _ = self.shell.execute("iptables -t nat -X SHELLCRASH");
        } else if firewall_mod == "nftables" {
            let _ = self.shell.execute("nft delete table inet shellcrash");
        }

        Ok(())
    }

    fn get_uptime(&self) -> Duration {
        let start_time_file = self.config.tmp_dir.join("crash_start_time");
        if let Ok(content) = fs::read_to_string(start_time_file) {
            if let Ok(start_time) = content.trim().parse::<u64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                return Duration::from_secs(now - start_time);
            }
        }
        Duration::from_secs(0)
    }
}
