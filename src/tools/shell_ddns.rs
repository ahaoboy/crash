// DDNS management - corresponds to tools/ShellDDNS.sh

use crate::common::{Config, Logger, Result, ShellCrashError, ShellExecutor};
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};

pub struct DDNSManager {
    config: Config,
    shell: ShellExecutor,
    logger: Logger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DDNSService {
    pub name: String,
    pub service_name: String,
    pub domain: String,
    pub username: String,
    pub password: String,
    pub check_interval: u32,
    pub force_interval: u32,
    pub use_ipv6: bool,
    pub enabled: bool,
}

impl DDNSManager {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            shell: ShellExecutor::new(),
            logger: Logger::new(),
        }
    }

    /// Check if OpenWrt DDNS is available
    pub fn check_ddns_available(&self) -> bool {
        std::path::Path::new("/etc/config/ddns").exists()
            && std::path::Path::new("/etc/ddns").is_dir()
    }

    /// Show main menu
    pub fn show_menu(&mut self) -> Result<()> {
        if !self.check_ddns_available() {
            self.logger
                .error("本脚本依赖OpenWrt内置的DDNS服务,当前设备无法运行");
            return Err(ShellCrashError::Unknown("DDNS服务不可用".to_string()).into());
        }

        loop {
            println!("-----------------------------------------------");
            println!("\x1b[30;46m欢迎使用ShellDDNS！\x1b[0m");
            println!("TG群：\x1b[36;4mhttps://t.me/ShellCrash\x1b[0m");
            println!("-----------------------------------------------");

            let services = self.list_services();

            if !services.is_empty() {
                println!("列表      域名       启用     IP地址");
                println!("-----------------------------------------------");
                for (idx, service) in services.iter().enumerate() {
                    let ip = self.get_service_ip(&service.name).unwrap_or_default();
                    let enabled = if service.enabled { "1" } else { "0" };
                    println!(" {}   {}  {}   {}", idx + 1, service.domain, enabled, ip);
                }
            }

            let options = vec!["添加DDNS服务", "退出"];
            let selection = Select::new()
                .with_prompt("请选择")
                .items(&options)
                .interact()
                .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

            if selection == options.len() - 1 {
                break;
            } else if selection < services.len() {
                self.manage_service(&services[selection])?;
            } else {
                self.add_service_interactive()?;
            }
        }

        Ok(())
    }

    /// Add a DDNS service interactively
    pub fn add_service_interactive(&mut self) -> Result<()> {
        // Select network type
        let network_types = vec!["IPv4", "IPv6"];
        let network_selection = Select::new()
            .with_prompt("请选择网络模式")
            .items(&network_types)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let use_ipv6 = network_selection == 1;

        // Select service provider
        let services_file = if use_ipv6 {
            "/etc/ddns/services_ipv6"
        } else {
            "/etc/ddns/services"
        };

        let services_content = std::fs::read_to_string(services_file)?;
        let providers: Vec<&str> = services_content
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split('"').nth(1))
            .collect();

        let provider_selection = Select::new()
            .with_prompt("请选择服务提供商")
            .items(&providers)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let service_name = providers[provider_selection].to_string();

        // Get service details
        let domain: String = Input::new()
            .with_prompt("请输入你的域名")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let username: String = Input::new()
            .with_prompt("请输入用户名或邮箱")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let password: String = Input::new()
            .with_prompt("请输入密码或令牌秘钥")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let check_interval: String = Input::new()
            .with_prompt("请输入检测更新间隔(单位:分钟;默认为10)")
            .default("10".to_string())
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let force_interval: String = Input::new()
            .with_prompt("请输入强制更新间隔(单位:小时;默认为24)")
            .default("24".to_string())
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let service = DDNSService {
            name: format!("ddns_{}", domain.replace('.', "_")),
            service_name,
            domain: domain.clone(),
            username,
            password,
            check_interval: check_interval.parse().unwrap_or(10),
            force_interval: force_interval.parse().unwrap_or(24),
            use_ipv6,
            enabled: true,
        };

        // Confirm
        println!("-----------------------------------------------");
        println!("请核对如下信息：");
        println!("服务商：\t\t\x1b[32m{}\x1b[0m", service.service_name);
        println!("域名：\t\t\t\x1b[32m{}\x1b[0m", service.domain);
        println!("用户名：\t\t\x1b[32m{}\x1b[0m", service.username);
        println!("检测间隔：\t\t\x1b[32m{}\x1b[0m", service.check_interval);
        println!("-----------------------------------------------");

        let confirm: String = Input::new()
            .with_prompt("确认添加？(1/0)")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if confirm == "1" {
            self.add_service(service)?;
        }

        Ok(())
    }

    /// Add a DDNS service
    pub fn add_service(&mut self, service: DDNSService) -> Result<()> {
        self.logger
            .info(&format!("添加DDNS服务: {}", service.domain));

        // Write to UCI config
        self.shell
            .execute(&format!("uci set ddns.{}=service", service.name))?;
        self.shell
            .execute(&format!("uci set ddns.{}.enabled='1'", service.name))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.service_name='{}'",
            service.name, service.service_name
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.domain='{}'",
            service.name, service.domain
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.username='{}'",
            service.name, service.username
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.password='{}'",
            service.name, service.password
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.check_interval='{}'",
            service.name, service.check_interval
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.force_interval='{}'",
            service.name, service.force_interval
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.use_ipv6='{}'",
            service.name,
            if service.use_ipv6 { "1" } else { "0" }
        ))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.lookup_host='{}'",
            service.name, service.domain
        ))?;
        self.shell
            .execute(&format!("uci set ddns.{}.use_https='0'", service.name))?;
        self.shell
            .execute(&format!("uci set ddns.{}.ip_source='web'", service.name))?;
        self.shell.execute(&format!(
            "uci set ddns.{}.check_unit='minutes'",
            service.name
        ))?;
        self.shell
            .execute(&format!("uci set ddns.{}.force_unit='hours'", service.name))?;
        self.shell
            .execute(&format!("uci set ddns.{}.interface='wan'", service.name))?;

        self.shell.execute("uci commit ddns")?;

        // Start the service
        self.shell.execute(&format!(
            "/usr/lib/ddns/dynamic_dns_updater.sh -S {} start",
            service.name
        ))?;

        self.logger.info("服务已经添加！");
        Ok(())
    }

    /// Remove a DDNS service
    pub fn remove_service(&mut self, service_name: &str) -> Result<()> {
        self.logger.info(&format!("删除DDNS服务: {}", service_name));

        self.shell
            .execute(&format!("uci delete ddns.{}", service_name))?;
        self.shell.execute("uci commit ddns")?;

        self.logger.info("服务已删除");
        Ok(())
    }

    /// List all DDNS services
    pub fn list_services(&self) -> Vec<DDNSService> {
        let mut services = Vec::new();

        if let Ok(output) = self.shell.execute("uci show ddns") {
            let config = String::from_utf8_lossy(&output.stdout);

            let mut current_service: Option<DDNSService> = None;
            let mut current_name = String::new();

            for line in config.lines() {
                if line.contains("=service") {
                    if let Some(service) = current_service.take() {
                        services.push(service);
                    }

                    current_name = line
                        .split('.')
                        .nth(1)
                        .unwrap_or("")
                        .split('=')
                        .next()
                        .unwrap_or("")
                        .to_string();

                    current_service = Some(DDNSService {
                        name: current_name.clone(),
                        service_name: String::new(),
                        domain: String::new(),
                        username: String::new(),
                        password: String::new(),
                        check_interval: 10,
                        force_interval: 24,
                        use_ipv6: false,
                        enabled: false,
                    });
                } else if let Some(ref mut service) = current_service {
                    if line.contains(&format!("ddns.{}.domain", current_name)) {
                        service.domain = line.split('\'').nth(1).unwrap_or("").to_string();
                    } else if line.contains(&format!("ddns.{}.enabled", current_name)) {
                        service.enabled = line.contains("'1'");
                    } else if line.contains(&format!("ddns.{}.service_name", current_name)) {
                        service.service_name = line.split('\'').nth(1).unwrap_or("").to_string();
                    }
                }
            }

            if let Some(service) = current_service {
                services.push(service);
            }
        }

        services
    }

    /// Update a DDNS service
    pub fn update_service(&self, service_name: &str) -> Result<()> {
        self.logger.info(&format!("更新DDNS服务: {}", service_name));

        self.shell.execute(&format!(
            "/usr/lib/ddns/dynamic_dns_updater.sh -S {} start",
            service_name
        ))?;

        self.logger.info("服务已更新");
        Ok(())
    }

    /// Manage a service
    fn manage_service(&mut self, service: &DDNSService) -> Result<()> {
        let options = vec![
            "立即更新",
            "编辑当前服务",
            if service.enabled {
                "停用当前服务"
            } else {
                "启用当前服务"
            },
            "移除当前服务",
            "返回上级菜单",
        ];

        let selection = Select::new()
            .with_prompt("请选择操作")
            .items(&options)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match selection {
            0 => self.update_service(&service.name)?,
            1 => {
                self.logger.info("编辑功能待实现");
            }
            2 => {
                let new_state = if service.enabled { "0" } else { "1" };
                self.shell.execute(&format!(
                    "uci set ddns.{}.enabled='{}'",
                    service.name, new_state
                ))?;
                self.shell.execute("uci commit ddns")?;
            }
            3 => self.remove_service(&service.name)?,
            4 => return Ok(()),
            _ => {}
        }

        Ok(())
    }

    fn get_service_ip(&self, service_name: &str) -> Option<String> {
        let log_file = format!("/var/log/ddns/{}.log", service_name);
        if let Ok(content) = std::fs::read_to_string(log_file) {
            for line in content.lines().rev() {
                if line.contains("Local IP") {
                    return line.split('\'').nth(1).map(|s| s.to_string());
                }
            }
        }
        None
    }
}
