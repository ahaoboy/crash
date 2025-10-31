// Web UI configuration

use github_proxy::{Proxy, Resource};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use strum::{Display, EnumString, IntoStaticStr};

/// UI type enumeration
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
pub enum UiType {
    #[default]
    Metacubexd,
    Zashboard,
    Yacd,
}

/// Web configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub ui: UiType,
    pub host: String,
    pub secret: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            ui: UiType::default(),
            host: ":9090".to_string(),
            secret: String::new(),
        }
    }
}

impl WebConfig {
    /// Get the UI type name as a string
    pub fn ui_name(&self) -> &'static str {
        self.ui.into()
    }

    /// Get the UI assets directory path
    pub fn ui_dir(&self, config_dir: &Path) -> PathBuf {
        config_dir.join(self.ui_name())
    }

    /// Get the release file name for the UI
    fn ui_release_file_name(&self) -> String {
        use UiType::*;
        match self.ui {
            Yacd => "yacd.tar.xz".to_string(),
            Zashboard => "zashboard.zip".to_string(),
            Metacubexd => "metacubexd.tgz".to_string(),
        }
    }

    /// Get the download URL for the UI
    pub fn ui_url(&self, proxy: &Proxy) -> crate::error::Result<String> {
        proxy
            .url(Resource::Release {
                owner: "ahaoboy".to_string(),
                repo: "crash-assets".to_string(),
                tag: "nightly".to_string(),
                name: self.ui_release_file_name(),
            })
            .ok_or_else(|| {
                crate::error::CrashError::Download("Failed to get UI download URL".to_string())
            })
    }
}
