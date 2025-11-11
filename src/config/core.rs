// Core-specific configuration

use crate::error::{CrashError, Result};
use crate::utils::path::exe_extension;
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

    pub fn github(&self) -> &'static str {
        match self {
            Core::Mihomo => "https://github.com/MetaCubeX/mihomo",
            Core::Clash => "https://github.com/Dreamacro/clash",
            Core::Singbox => "https://github.com/SagerNet/sing-box",
        }
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
        match self {
            Core::Mihomo | Core::Clash => format!("{}.yaml", self.name()),
            Core::Singbox => format!("{}.json", self.name()),
        }
    }

    /// Get the platform-specific release file name
    pub fn release_file_name(&self, target: &Target) -> Result<String> {
        use Core::*;

        let filename = match (self, target) {
            (Mihomo, Target::X86_64PcWindowsMsvc | Target::X86_64PcWindowsGnu) => {
                "mihomo-windows-amd64-v1.19.15.tar.gz"
            }
            (Mihomo, Target::Aarch64UnknownLinuxMusl | Target::Aarch64UnknownLinuxGnu) => {
                "mihomo-linux-arm64-v1.19.15.tar.gz"
            }
            (Mihomo, Target::X86_64UnknownLinuxGnu | Target::X86_64UnknownLinuxMusl) => {
                "mihomo-linux-amd64-v1.19.15.tar.gz"
            }
            (Mihomo, Target::Aarch64AppleDarwin) => "mihomo-darwin-arm64-v1.19.15.tar.gz",
            (Mihomo, Target::X86_64AppleDarwin) => "mihomo-darwin-amd64-v1.19.15.tar.gz",

            (Clash, Target::Aarch64UnknownLinuxMusl | Target::Aarch64UnknownLinuxGnu) => {
                "clash-linux-arm64.tar.gz"
            }
            (Clash, Target::X86_64UnknownLinuxGnu) => "clash-linux-amd64.tar.gz",

            (Singbox, Target::X86_64PcWindowsMsvc | Target::X86_64PcWindowsGnu) => {
                "sing-box-1.12.12-windows-amd64.tar.gz"
            }
            (Singbox, Target::Aarch64UnknownLinuxMusl | Target::Aarch64UnknownLinuxGnu) => {
                "sing-box-1.12.12-linux-arm64.tar.gz"
            }
            (Singbox, Target::X86_64UnknownLinuxGnu) => "sing-box-1.12.12-linux-amd64.tar.gz",
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

        Ok(Resource::File {
            owner: "ahaoboy".to_string(),
            repo: "crash-assets".to_string(),
            reference: "main".to_string(),
            path: filename.to_string(),
        })
    }

    pub fn get_geo_files(&self) -> Vec<&'static str> {
        match self {
            Core::Mihomo | Core::Clash => vec![
                "geoip.metadb.tar.gz",
                "geoip.dat.tar.gz",
                "geosite.dat.tar.gz",
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

    pub fn envs(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            Core::Mihomo => vec![],
            Core::Clash => vec![],
            Core::Singbox => vec![("ENABLE_DEPRECATED_SPECIAL_OUTBOUNDS", "true")],
        }
    }
}
