// Menu system - corresponds to scripts/menu.sh

use crate::common::{Config, Logger, Result, ShellCrashError, ShellExecutor};
use crate::common::i18n::t;
use crate::scripts::init::VERSION;
use dialoguer::{Input, Select};
use std::time::Duration;

pub struct MenuSystem {
    config: Config,
    shell: ShellExecutor,
    logger: Logger,
}

#[derive(Debug)]
pub enum MenuAction {
    StartService,
    StopService,
    RestartService,
    UpdateConfig,
    ShowStatus,
    ConfigPorts,
    ConfigDNS,
    ConfigFirewall,
    ConfigIPv6,
    LogPusher,
    TaskManager,
    Exit,
}

#[derive(Debug, Clone)]
pub enum ServiceStatus {
    Running {
        pid: u32,
        uptime: Duration,
        memory: u64,
        mode: String,
    },
    Stopped,
    Error(String),
}

impl MenuSystem {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            shell: ShellExecutor::new(),
            logger: Logger::new(),
        }
    }

    /// Show main menu
    pub fn show_main_menu(&self) -> Result<()> {
        loop {
            self.display_status()?;

            let options = vec![
                format!("1 {}", t("menu_start_restart")),
                format!("2 {}", t("menu_stop")),
                format!("3 {}", t("menu_config_ports")),
                format!("4 {}", t("menu_config_dns")),
                format!("5 {}", t("menu_config_firewall")),
                format!("6 {}", t("menu_config_ipv6")),
                format!("7 {}", t("menu_log_push")),
                format!("8 {}", t("menu_task_manager")),
                format!("9 {}", t("menu_update_config")),
                format!("L {}", t("menu_language")),
                format!("0 {}", t("menu_exit")),
            ];

            let selection = Select::new()
                .with_prompt(t("prompt_select"))
                .items(&options)
                .interact()
                .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

            match selection {
                0 => {
                    // Start/Restart service
                    use crate::scripts::ServiceManager;
                    let service = ServiceManager::new(self.config.clone());
                    if let Err(e) = service.restart() {
                        self.logger.error(&format!("服务启动失败: {}", e));
                    } else {
                        self.logger.info("服务已启动/重启");
                    }
                }
                1 => {
                    // Stop service
                    use crate::scripts::ServiceManager;
                    let service = ServiceManager::new(self.config.clone());
                    if let Err(e) = service.stop() {
                        self.logger.error(&format!("服务停止失败: {}", e));
                    } else {
                        self.logger.info("服务已停止");
                    }
                }
                2 => self.config_ports()?,
                3 => self.config_dns()?,
                4 => self.config_firewall()?,
                5 => self.config_ipv6()?,
                6 => self.config_log_pusher()?,
                7 => {
                    use crate::scripts::TaskManager;
                    let mut task_manager = TaskManager::new(self.config.clone());
                    if let Err(e) = task_manager.show_menu() {
                        self.logger.error(&format!("任务管理失败: {}", e));
                    }
                }
                8 => {
                    self.logger.info(&t("updating_config"));
                    if let Err(e) = self.update_config() {
                        self.logger.error(&format!("{}: {}", t("error_config"), e));
                    }
                }
                9 => {
                    // Language switch
                    self.switch_language()?;
                }
                10 => break,
                _ => {}
            }
        }
        Ok(())
    }

    /// Display current status
    fn display_status(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("\x1b[30;46m{}\x1b[0m\t\t{}: {}", t("welcome"), t("version"), VERSION);

        let status = self.check_status();
        match status {
            ServiceStatus::Running { pid, uptime, memory, mode } => {
                println!("{} \x1b[32m{}（{}）\x1b[0m", t("service_status"), t("service_running"), mode);
                println!(
                    "{}: \x1b[44m{:.2} MB\x1b[0m，{}: \x1b[46;30m{:?}\x1b[0m",
                    t("memory_usage"),
                    memory as f64 / 1024.0,
                    t("uptime"),
                    uptime
                );
            }
            ServiceStatus::Stopped => {
                println!("{}: \x1b[31m{}\x1b[0m", t("service_status"), t("service_stopped"));
            }
            ServiceStatus::Error(e) => {
                println!("{}: \x1b[31m{} - {}\x1b[0m", t("service_status"), t("error"), e);
            }
        }

        println!("TG Channel: \x1b[36;4mhttps://t.me/ShellClash\x1b[0m");
        println!("-----------------------------------------------");

        Ok(())
    }

    /// Switch language
    fn switch_language(&self) -> Result<()> {
        use crate::common::{get_language, set_language, Language};

        let current = get_language();
        let current_name = match current {
            Language::English => "English",
            Language::Chinese => "中文",
        };

        println!("-----------------------------------------------");
        println!("{}: {}", t("current_language"), current_name);
        println!("-----------------------------------------------");

        let options = vec![
            "English",
            "中文 (Chinese)",
        ];

        let selection = Select::new()
            .with_prompt(t("prompt_select"))
            .items(&options)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let new_lang = match selection {
            0 => Language::English,
            1 => Language::Chinese,
            _ => return Ok(()),
        };

        if new_lang != current {
            set_language(new_lang);

            // Save language preference
            if let Some(config_dir) = dirs::config_dir() {
                let lang_file = config_dir.join("shellcrash").join("language");
                if let Some(parent) = lang_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&lang_file, new_lang.code());
            }

            self.logger.info(&t("language_changed"));
        }

        Ok(())
    }

    /// Check service status
    pub fn check_status(&self) -> ServiceStatus {
        // Try to get PID of CrashCore
        if let Ok(output) = self.shell.execute("pidof CrashCore") {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = pid_str.split_whitespace().last().unwrap_or("").parse::<u32>() {
                // Get memory usage
                let memory = self
                    .shell
                    .execute(&format!("cat /proc/{}/status | grep VmRSS | awk '{{print $2}}'", pid))
                    .ok()
                    .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
                    .unwrap_or(0);

                // Get uptime
                // Calculate uptime from start time file
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

    /// Configure ports
    fn config_ports(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("{}", t("port_config"));
        println!("-----------------------------------------------");
        println!(" 1 {}:\t\x1b[36m{}\x1b[0m", t("modify_http_port"), self.config.ports.mix_port);
        println!(" 2 {}:\t\x1b[36m{}\x1b[0m", t("modify_redir_port"), self.config.ports.redir_port);
        println!(" 3 {}:\t\x1b[36m{}\x1b[0m", t("modify_dns_port"), self.config.ports.dns_port);
        println!(" 4 {}:\t\x1b[36m{}\x1b[0m", t("modify_panel_port"), self.config.ports.db_port);
        println!(" 0 {}", t("return_menu"));

        let choice: String = Input::new()
            .with_prompt(t("prompt_input"))
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" => self.set_port("mix_port", &t("modify_http_port"))?,
            "2" => self.set_port("redir_port", &t("modify_redir_port"))?,
            "3" => self.set_port("dns_port", &t("modify_dns_port"))?,
            "4" => self.set_port("db_port", &t("modify_panel_port"))?,
            "0" => return Ok(()),
            _ => self.logger.error(&t("error_invalid_input")),
        }

        Ok(())
    }

    fn set_port(&self, port_name: &str, display_name: &str) -> Result<()> {
        let port: String = Input::new()
            .with_prompt(&format!("请输入{}(1-65535)", display_name))
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if let Ok(port_num) = port.parse::<u16>() {
            if port_num > 0 {
                // Update config and save
                let mut config = self.config.clone();
                config.set_value(port_name, &port_num.to_string())?;

                let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
                config.save(&config_file)?;

                self.logger.info(&format!("{}设置为: {}", display_name, port_num));
                return Ok(());
            }
        }

        Err(ShellCrashError::ConfigError("无效的端口号".to_string()).into())
    }

    /// Configure DNS
    fn config_dns(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("DNS配置");
        println!("-----------------------------------------------");
        println!("当前基础DNS：\x1b[32m{}\x1b[0m", self.config.dns.nameserver.join(", "));
        println!("PROXY-DNS：\x1b[36m{}\x1b[0m", self.config.dns.fallback.join(", "));
        println!(" 1 修改基础DNS");
        println!(" 2 修改PROXY-DNS");
        println!(" 3 重置默认DNS配置");
        println!(" 0 返回上级菜单");

        let choice: String = Input::new()
            .with_prompt("请输入对应数字")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" | "2" | "3" => {
                self.logger.info("DNS配置功能待实现");
            }
            "0" => return Ok(()),
            _ => self.logger.error("请输入正确的数字！"),
        }

        Ok(())
    }

    /// Configure firewall
    fn config_firewall(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("防火墙配置");
        println!("-----------------------------------------------");
        println!(" 1 公网访问Dashboard面板");
        println!(" 2 公网访问Socks/Http代理");
        println!(" 3 自定义透明路由ipv4网段");
        println!(" 0 返回上级菜单");

        let choice: String = Input::new()
            .with_prompt("请输入对应数字")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" | "2" | "3" => {
                self.logger.info("防火墙配置功能待实现");
            }
            "0" => return Ok(()),
            _ => self.logger.error("请输入正确的数字！"),
        }

        Ok(())
    }

    /// Configure IPv6
    fn config_ipv6(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("IPv6配置");
        println!("-----------------------------------------------");
        println!(" 1 ipv6透明代理: \x1b[36m{}\x1b[0m", self.config.firewall.ipv6_redir);
        println!(" 0 返回上级菜单");

        let choice: String = Input::new()
            .with_prompt("请输入对应数字")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" => {
                self.logger.info("IPv6配置功能待实现");
            }
            "0" => return Ok(()),
            _ => self.logger.error("请输入正确的数字！"),
        }

        Ok(())
    }

    /// Configure log pusher
    fn config_log_pusher(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("日志推送配置");
        println!("-----------------------------------------------");
        println!(" 1 Telegram推送");
        println!(" 2 PushDeer推送");
        println!(" 3 Bark推送-IOS");
        println!(" 4 Pushover推送");
        println!(" 5 PushPlus推送");
        println!(" 6 SynoChat推送");
        println!(" 0 返回上级菜单");

        let choice: String = Input::new()
            .with_prompt("请输入对应数字")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" => self.config_telegram_push()?,
            "2" => self.config_pushdeer()?,
            "3" => self.config_bark()?,
            "4" => self.config_pushover()?,
            "5" => self.config_pushplus()?,
            "6" => self.config_synochat()?,
            "0" => return Ok(()),
            _ => self.logger.error("请输入正确的数字！"),
        }

        Ok(())
    }

    /// Configure Telegram push
    fn config_telegram_push(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("Telegram 推送配置");
        println!("请先通过 https://t.me/BotFather 申请TG机器人并获取其API TOKEN");

        let token: String = Input::new()
            .with_prompt("请输入API TOKEN")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !token.is_empty() {
            println!("请向你申请的机器人发送任意几条消息！");
            let confirm: String = Input::new()
                .with_prompt("我已经发送完成(1/0)")
                .interact_text()
                .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

            if confirm == "1" {
                let mut config = self.config.clone();
                config.set_value("push_TG", &token)?;
                let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
                config.save(&config_file)?;
                self.logger.info("Telegram推送配置已保存");
            }
        }
        Ok(())
    }

    /// Configure PushDeer
    fn config_pushdeer(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("PushDeer 推送配置");
        println!("请先前往 http://www.pushdeer.com/official.html 获取秘钥");

        let key: String = Input::new()
            .with_prompt("请输入秘钥")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !key.is_empty() {
            let mut config = self.config.clone();
            config.set_value("push_Deer", &key)?;
            let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
            config.save(&config_file)?;
            self.logger.info("PushDeer推送配置已保存");
        }
        Ok(())
    }

    /// Configure Bark
    fn config_bark(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("Bark 推送配置 (仅支持IOS)");

        let url: String = Input::new()
            .with_prompt("请输入Bark推送链接")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !url.is_empty() {
            let mut config = self.config.clone();
            config.set_value("push_bark", &url)?;
            let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
            config.save(&config_file)?;
            self.logger.info("Bark推送配置已保存");
        }
        Ok(())
    }

    /// Configure Pushover
    fn config_pushover(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("Pushover 推送配置");

        let user_key: String = Input::new()
            .with_prompt("请输入User Key")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let api_token: String = Input::new()
            .with_prompt("请输入API Token")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !user_key.is_empty() && !api_token.is_empty() {
            let mut config = self.config.clone();
            config.set_value("push_Po", &api_token)?;
            config.set_value("push_Po_key", &user_key)?;
            let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
            config.save(&config_file)?;
            self.logger.info("Pushover推送配置已保存");
        }
        Ok(())
    }

    /// Configure PushPlus
    fn config_pushplus(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("PushPlus 推送配置");

        let token: String = Input::new()
            .with_prompt("请输入token")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !token.is_empty() {
            let mut config = self.config.clone();
            config.set_value("push_PP", &token)?;
            let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
            config.save(&config_file)?;
            self.logger.info("PushPlus推送配置已保存");
        }
        Ok(())
    }

    /// Configure SynoChat
    fn config_synochat(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("SynoChat 推送配置");

        let url: String = Input::new()
            .with_prompt("请输入Synology DSM主页地址")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let token: String = Input::new()
            .with_prompt("请输入Chat Token")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let user_id: String = Input::new()
            .with_prompt("请输入user_id")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        if !url.is_empty() && !token.is_empty() && !user_id.is_empty() {
            let mut config = self.config.clone();
            config.set_value("push_ChatURL", &url)?;
            config.set_value("push_ChatTOKEN", &token)?;
            config.set_value("push_ChatUSERID", &user_id)?;
            let config_file = self.config.crash_dir.join("configs/ShellCrash.cfg");
            config.save(&config_file)?;
            self.logger.info("SynoChat推送配置已保存");
        }
        Ok(())
    }

    /// Update configuration
    fn update_config(&self) -> Result<()> {
        println!("-----------------------------------------------");
        println!("{}", t("menu_update_config"));
        println!("-----------------------------------------------");
        println!(" 1 {}", t("update_subscription"));
        println!(" 2 {}", t("update_core"));
        println!(" 3 {}", t("update_scripts"));
        println!(" 4 {}", t("update_geoip_db"));
        println!(" 0 {}", t("return_menu"));

        let choice: String = Input::new()
            .with_prompt(t("prompt_input"))
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match choice.as_str() {
            "1" => {
                self.logger.info(&t("updating_subscription"));
                use crate::scripts::Downloader;
                let downloader = Downloader::new(self.config.clone());
                downloader.update_subscription()?;
            }
            "2" => {
                self.logger.info(&t("updating_core"));
                use crate::scripts::Downloader;
                let downloader = Downloader::new(self.config.clone());
                downloader.update_core()?;
            }
            "3" => {
                self.logger.info(&t("updating_scripts"));
                use crate::scripts::Downloader;
                let downloader = Downloader::new(self.config.clone());
                downloader.update_scripts()?;
            }
            "4" => {
                self.logger.info(&t("updating_geoip"));
                use crate::scripts::Downloader;
                let downloader = Downloader::new(self.config.clone());
                downloader.update_geoip()?;
            }
            "0" => return Ok(()),
            _ => self.logger.error(&t("error_invalid_input")),
        }

        Ok(())
    }

    /// Get service uptime
    fn get_uptime(&self) -> Duration {
        let start_time_file = self.config.tmp_dir.join("crash_start_time");
        if let Ok(content) = std::fs::read_to_string(start_time_file) {
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
