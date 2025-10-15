// Configuration management

use crate::common::error::{Result, ShellCrashError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub crash_dir: PathBuf,

    #[serde(default)]
    pub bin_dir: PathBuf,

    #[serde(default = "default_tmp_dir")]
    pub tmp_dir: PathBuf,

    #[serde(default)]
    pub ports: PortConfig,

    #[serde(default)]
    pub dns: DnsConfig,

    #[serde(default)]
    pub firewall: FirewallConfig,

    #[serde(default)]
    pub core: CoreConfig,

    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    #[serde(default = "default_mix_port")]
    pub mix_port: u16,

    #[serde(default = "default_redir_port")]
    pub redir_port: u16,

    #[serde(default = "default_tproxy_port")]
    pub tproxy_port: u16,

    #[serde(default = "default_db_port")]
    pub db_port: u16,

    #[serde(default = "default_dns_port")]
    pub dns_port: u16,

    #[serde(default = "default_fwmark")]
    pub fwmark: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    #[serde(default = "default_nameserver")]
    pub nameserver: Vec<String>,

    #[serde(default = "default_fallback")]
    pub fallback: Vec<String>,

    #[serde(default = "default_dns_mode")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallConfig {
    #[serde(default = "default_firewall_mod")]
    pub firewall_mod: String,

    #[serde(default)]
    pub ipv6_redir: String,

    #[serde(default)]
    pub redir_mod: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoreConfig {
    #[serde(default)]
    pub crashcore: String,

    #[serde(default)]
    pub core_v: String,

    #[serde(default)]
    pub target: String,
}

// Default value functions
fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_tmp_dir() -> PathBuf {
    PathBuf::from("/tmp/ShellCrash")
}

fn default_mix_port() -> u16 {
    7890
}

fn default_redir_port() -> u16 {
    7892
}

fn default_tproxy_port() -> u16 {
    7893
}

fn default_db_port() -> u16 {
    9999
}

fn default_dns_port() -> u16 {
    1053
}

fn default_fwmark() -> u16 {
    7892
}

fn default_nameserver() -> Vec<String> {
    vec!["114.114.114.114".to_string(), "223.5.5.5".to_string()]
}

fn default_fallback() -> Vec<String> {
    vec!["1.0.0.1".to_string(), "8.8.4.4".to_string()]
}

fn default_dns_mode() -> String {
    "fake-ip".to_string()
}

fn default_firewall_mod() -> String {
    "iptables".to_string()
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            mix_port: default_mix_port(),
            redir_port: default_redir_port(),
            tproxy_port: default_tproxy_port(),
            db_port: default_db_port(),
            dns_port: default_dns_port(),
            fwmark: default_fwmark(),
        }
    }
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            nameserver: default_nameserver(),
            fallback: default_fallback(),
            mode: default_dns_mode(),
        }
    }
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            firewall_mod: default_firewall_mod(),
            ipv6_redir: "未开启".to_string(),
            redir_mod: "纯净模式".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| ShellCrashError::ConfigError(format!("无法读取配置文件: {}", e)))?;

        // Try to parse as different formats
        if let Ok(config) = toml::from_str::<Config>(&content) {
            return Ok(config);
        }

        if let Ok(config) = serde_yaml::from_str::<Config>(&content) {
            return Ok(config);
        }

        // Parse as key=value format (original shell script format)
        Self::parse_shell_config(&content)
    }

    /// Parse shell script style config (key=value)
    fn parse_shell_config(content: &str) -> Result<Self> {
        let mut config = Config::default();
        let mut extra = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('\'').trim_matches('"');

                match key {
                    "mix_port" => {
                        config.ports.mix_port = value.parse().unwrap_or(default_mix_port())
                    }
                    "redir_port" => {
                        config.ports.redir_port = value.parse().unwrap_or(default_redir_port())
                    }
                    "db_port" => config.ports.db_port = value.parse().unwrap_or(default_db_port()),
                    "dns_port" => {
                        config.ports.dns_port = value.parse().unwrap_or(default_dns_port())
                    }
                    "crashcore" => config.core.crashcore = value.to_string(),
                    "core_v" => config.core.core_v = value.to_string(),
                    "redir_mod" => config.firewall.redir_mod = value.to_string(),
                    _ => {
                        extra.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        config.extra = extra;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Save in shell script format for compatibility
        let mut content = String::from("#ShellCrash配置文件，不明勿动！\n");

        // Add basic config
        content.push_str(&format!("mix_port={}\n", self.ports.mix_port));
        content.push_str(&format!("redir_port={}\n", self.ports.redir_port));
        content.push_str(&format!("db_port={}\n", self.ports.db_port));
        content.push_str(&format!("dns_port={}\n", self.ports.dns_port));
        content.push_str(&format!("crashcore={}\n", self.core.crashcore));
        content.push_str(&format!("core_v={}\n", self.core.core_v));
        content.push_str(&format!("redir_mod={}\n", self.firewall.redir_mod));

        // Add extra fields
        for (key, value) in &self.extra {
            content.push_str(&format!("{}={}\n", key, value));
        }

        fs::write(path.as_ref(), content)
            .map_err(|e| ShellCrashError::ConfigError(format!("无法写入配置文件: {}", e)))?;

        Ok(())
    }

    /// Set a configuration value
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "mix_port" => {
                self.ports.mix_port = value.parse().map_err(|_| {
                    ShellCrashError::ConfigError(format!("无效的端口号: {}", value))
                })?;
            }
            "redir_port" => {
                self.ports.redir_port = value.parse().map_err(|_| {
                    ShellCrashError::ConfigError(format!("无效的端口号: {}", value))
                })?;
            }
            "db_port" => {
                self.ports.db_port = value.parse().map_err(|_| {
                    ShellCrashError::ConfigError(format!("无效的端口号: {}", value))
                })?;
            }
            "dns_port" => {
                self.ports.dns_port = value.parse().map_err(|_| {
                    ShellCrashError::ConfigError(format!("无效的端口号: {}", value))
                })?;
            }
            "crashcore" => self.core.crashcore = value.to_string(),
            "core_v" => self.core.core_v = value.to_string(),
            "redir_mod" => self.firewall.redir_mod = value.to_string(),
            _ => {
                self.extra.insert(key.to_string(), value.to_string());
            }
        }
        Ok(())
    }

    /// Get a configuration value
    pub fn get_value(&self, key: &str) -> Option<String> {
        match key {
            "mix_port" => Some(self.ports.mix_port.to_string()),
            "redir_port" => Some(self.ports.redir_port.to_string()),
            "db_port" => Some(self.ports.db_port.to_string()),
            "dns_port" => Some(self.ports.dns_port.to_string()),
            "crashcore" => Some(self.core.crashcore.clone()),
            "core_v" => Some(self.core.core_v.clone()),
            "redir_mod" => Some(self.firewall.redir_mod.clone()),
            _ => self.extra.get(key).cloned(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            crash_dir: PathBuf::new(),
            bin_dir: PathBuf::new(),
            tmp_dir: default_tmp_dir(),
            ports: PortConfig::default(),
            dns: DnsConfig::default(),
            firewall: FirewallConfig::default(),
            core: CoreConfig::default(),
            extra: HashMap::new(),
        }
    }
}
