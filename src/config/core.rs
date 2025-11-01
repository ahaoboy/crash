// Core-specific configuration

use crate::error::{CrashError, Result};
use crate::platform::path::exe_extension;
use github_proxy::Resource;
use guess_target::Target;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use strum::{Display, EnumString, IntoStaticStr};

/// Proxy core type enumeration
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Display,
    EnumString,
    IntoStaticStr,
    Serialize,
    Deserialize,
)]
pub enum Core {
    #[default]
    Mihomo,
    Clash,
    Singbox,
}
impl Core {
    /// Get the core type name as a string
    pub fn name(&self) -> &'static str {
        self.into()
    }

    /// Get the executable name with platform-specific extension
    pub fn exe_name(&self) -> String {
        format!("{}{}", self.name(), exe_extension())
    }

    /// Get the full path to the executable
    pub fn exe_path(&self, config_dir: &Path) -> PathBuf {
        config_dir.join(self.exe_name())
    }

    /// Get the configuration file name
    pub fn config_file_name(&self) -> String {
        format!("{}.yaml", self.name())
    }

    /// Get the platform-specific release file name
    pub fn release_file_name(&self, target: &Target) -> Result<String> {
        use Core::*;

        let filename = match (self, target) {
            (Mihomo, Target::X86_64PcWindowsMsvc | Target::X86_64PcWindowsGnu) => {
                "mihomo-windows-amd64-v1.19.15.tar.xz"
            }
            (Mihomo, Target::Aarch64UnknownLinuxMusl) => "mihomo-linux-arm64-v1.19.15.tar.xz",
            (Mihomo, Target::X86_64UnknownLinuxGnu) => "mihomo-linux-amd64-v1.19.15.tar.xz",
            (Mihomo, Target::Aarch64AppleDarwin) => "mihomo-darwin-arm64-v1.19.15.tar.xz",
            (Mihomo, Target::X86_64AppleDarwin) => "mihomo-darwin-amd64-v1.19.15.tar.xz",

            (Clash, Target::Aarch64UnknownLinuxMusl) => "clash-linux-arm64.tar.xz",
            (Clash, Target::X86_64UnknownLinuxGnu) => "clash-linux-amd64.tar.xz",
            _ => {
                return Err(CrashError::Config(format!(
                    "Unsupported core type {:?} on target {:?}",
                    self, target
                )));
            }
        };

        Ok(filename.to_string())
    }

    /// Get the repository resource for downloading the core
    pub fn repo(&self, target: &Target) -> Result<Resource> {
        let filename = self.release_file_name(target)?;

        Ok(Resource::Release {
            owner: "ahaoboy".to_string(),
            repo: "crash-assets".to_string(),
            tag: "nightly".to_string(),
            name: filename,
        })
    }

    pub fn get_geo_files(&self) -> Vec<&'static str> {
        match self {
            Core::Mihomo | Core::Clash => vec![
                "geoip.metadb.tar.xz",
                "geoip.dat.tar.xz",
                "geosite.dat.tar.xz",
            ],
            // Core::Clash => vec![
            //     "china_ip_list.txt",
            //     "china_ipv6_list.txt",
            //     "cn_mini.mmdb",
            //     "Country.mmdb",
            //     "geoip_cn.db",
            //     "geosite.dat",
            //     "geosite_cn.db",
            //     "mrs_geosite_cn.mrs",
            //     "srs_geoip_cn.srs",
            //     "srs_geosite_cn.srs",
            // ],
            _ => vec![],
        }
    }

    pub fn patch_config(&self, config: &str) -> String {
        match self {
            Core::Mihomo => {
                let has_tun = config.lines().any(|i| i.starts_with("tun"));
                if has_tun {
                    config.to_string()
                } else {
                    format!(
                        "{}\n{}",
                        config,
                        r#"
# Crash default tun
tun:
  enable: true
  device: Meta
  stack: gVisor
  dns-hijack:
    - 0.0.0.0:53
  auto-route: true
  auto-detect-interface: true
  gso-max-size: 65536
  file-descriptor: 0
  recvmsgx: true
"#
                    )
                }
            }
            Core::Clash => return config.replace("- 'RULE-SET,", "#- 'RULE-SET,").to_string(),
            _ => config.to_string(),
        }
    }
}
