// Logging system

use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Logger {
    log_file: Option<Mutex<File>>,
    enable_color: bool,
    push_config: Option<PushConfig>,
}

#[derive(Debug, Clone)]
pub struct PushConfig {
    pub telegram: Option<TelegramConfig>,
    pub pushdeer: Option<String>,
    pub bark: Option<String>,
    pub pushover: Option<PushoverConfig>,
    pub pushplus: Option<String>,
    pub synochat: Option<SynoChatConfig>,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone)]
pub struct PushoverConfig {
    pub token: String,
    pub user_key: String,
}

#[derive(Debug, Clone)]
pub struct SynoChatConfig {
    pub url: String,
    pub token: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Logger {
    /// Create a new logger
    pub fn new() -> Self {
        Self {
            log_file: None,
            enable_color: true,
            push_config: None,
        }
    }

    /// Set push configuration
    pub fn with_push_config(mut self, config: PushConfig) -> Self {
        self.push_config = Some(config);
        self
    }

    /// Set log file path
    pub fn with_file(mut self, path: PathBuf) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        self.log_file = Some(Mutex::new(file));
        Ok(self)
    }

    /// Enable or disable colored output
    pub fn with_color(mut self, enable: bool) -> Self {
        self.enable_color = enable;
        self
    }

    /// Log a message
    pub fn log(&self, level: LogLevel, message: &str) {
        let timestamp = Local::now().format("%Y-%m-%d_%H:%M:%S");
        let log_text = format!("{}~{}", timestamp, message);

        // Print to console with color
        if self.enable_color {
            let colored_msg = match level {
                LogLevel::Debug => format!("\x1b[36m{}\x1b[0m", message),
                LogLevel::Info => format!("\x1b[32m{}\x1b[0m", message),
                LogLevel::Warn => format!("\x1b[33m{}\x1b[0m", message),
                LogLevel::Error => format!("\x1b[31m{}\x1b[0m", message),
            };
            println!("{}", colored_msg);
        } else {
            println!("{}", message);
        }

        // Write to log file
        if let Some(ref file) = self.log_file {
            if let Ok(mut f) = file.lock() {
                let _ = writeln!(f, "{}", log_text);
            }
        }
    }

    /// Log debug message
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log info message
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log warning message
    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    /// Log error message
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    /// Log with custom color code
    pub fn log_colored(&self, message: &str, color_code: u8) {
        if self.enable_color {
            println!("\x1b[{}m{}\x1b[0m", color_code, message);
        } else {
            println!("{}", message);
        }

        // Also write to log file
        if let Some(ref file) = self.log_file {
            if let Ok(mut f) = file.lock() {
                let timestamp = Local::now().format("%Y-%m-%d_%H:%M:%S");
                let _ = writeln!(f, "{}~{}", timestamp, message);
            }
        }
    }

    /// Push log message to configured services
    pub async fn push_log(&self, message: &str) {
        if let Some(ref config) = self.push_config {
            let full_message = if let Some(ref device) = config.device_name {
                format!("{}({})", message, device)
            } else {
                message.to_string()
            };

            // Push to Telegram
            if let Some(ref tg) = config.telegram {
                let _ = self.push_telegram(&full_message, tg).await;
            }

            // Push to PushDeer
            if let Some(ref key) = config.pushdeer {
                let _ = self.push_pushdeer(&full_message, key).await;
            }

            // Push to Bark
            if let Some(ref url) = config.bark {
                let _ = self.push_bark(&full_message, url).await;
            }

            // Push to Pushover
            if let Some(ref po) = config.pushover {
                let _ = self.push_pushover(&full_message, po).await;
            }

            // Push to PushPlus
            if let Some(ref token) = config.pushplus {
                let _ = self.push_pushplus(&full_message, token).await;
            }

            // Push to SynoChat
            if let Some(ref sc) = config.synochat {
                let _ = self.push_synochat(&full_message, sc).await;
            }
        }
    }

    async fn push_telegram(&self, message: &str, config: &TelegramConfig) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = format!("https://api.telegram.org/bot{}/sendMessage", config.token);
        let body = serde_json::json!({
            "chat_id": config.chat_id,
            "text": message
        });
        client.post(&url).json(&body).send().await?;
        Ok(())
    }

    async fn push_pushdeer(&self, message: &str, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = "https://api2.pushdeer.com/message/push";
        let body = serde_json::json!({
            "pushkey": key,
            "text": message
        });
        client.post(url).json(&body).send().await?;
        Ok(())
    }

    async fn push_bark(&self, message: &str, url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "body": message,
            "title": "ShellCrash日志推送",
            "level": "passive",
            "badge": "1"
        });
        client.post(url).json(&body).send().await?;
        Ok(())
    }

    async fn push_pushover(&self, message: &str, config: &PushoverConfig) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = "https://api.pushover.net/1/messages.json";
        let body = serde_json::json!({
            "token": config.token,
            "user": config.user_key,
            "title": "ShellCrash日志推送",
            "message": message
        });
        client.post(url).json(&body).send().await?;
        Ok(())
    }

    async fn push_pushplus(&self, message: &str, token: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = "http://www.pushplus.plus/send";
        let body = serde_json::json!({
            "token": token,
            "title": "ShellCrash日志推送",
            "content": message
        });
        client.post(url).json(&body).send().await?;
        Ok(())
    }

    async fn push_synochat(&self, message: &str, config: &SynoChatConfig) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/webapi/entry.cgi?api=SYNO.Chat.External&method=chatbot&version=2&token={}",
            config.url, config.token
        );
        let body = format!("payload={{\"text\":\"{}\", \"user_ids\":[{}]}}", message, config.user_id);
        client.post(&url).body(body).send().await?;
        Ok(())
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}
