// Web UI configuration

use clap::ValueEnum;
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
    ValueEnum,
)]
pub enum UiType {
    #[default]
    Metacubexd,
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

    /// Get the UI version if available
    pub fn ui_version(&self, config_dir: &Path) -> Option<String> {
        let index_html = self.ui_dir(config_dir).join("index.html");
        let content = std::fs::read_to_string(index_html).ok()?;

        match self.ui {
            UiType::Metacubexd => {
                let start_pattern = "appVersion:\"";
                if let Some(start) = content.find(start_pattern) {
                    let start = start + start_pattern.len();
                    if let Some(end) = content[start..].find('"') {
                        return Some(content[start..start + end].to_string());
                    }
                }
            }
            UiType::Yacd => {}
        }
        None
    }

    /// Get the release file name for the UI
    fn ui_release_file_name(&self) -> String {
        use UiType::*;
        match self.ui {
            Yacd => "yacd.tar.gz".to_string(),
            Metacubexd => "metacubexd.tar.gz".to_string(),
        }
    }

    /// Get the download URL for the UI
    pub fn ui_url(&self) -> crate::error::Result<String> {
        Resource::File {
            owner: "ahaoboy".to_string(),
            repo: "crash-assets".to_string(),
            reference: "main".to_string(),
            path: self.ui_release_file_name(),
        }
        .url(&Proxy::Github)
        .ok_or_else(|| {
            crate::error::CrashError::Download("Failed to get UI download URL".to_string())
        })
    }
}
