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
                "mihomo-windows-amd64-v1.19.15.zip"
            }
            (Mihomo, Target::Aarch64UnknownLinuxMusl) => "mihomo-linux-arm64-v1.19.15.tgz",
            (Mihomo, Target::X86_64UnknownLinuxGnu) => "mihomo-linux-amd64-v1.19.15.tgz",
            (Mihomo, Target::Aarch64AppleDarwin) => "mihomo-darwin-arm64-v1.19.15.tgz",
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
}
