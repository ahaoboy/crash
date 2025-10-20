use crate::{
    common::Language,
    download::{Proxy, Repo, RepoRelease},
};
use anyhow::Ok;
use easy_install::Args;
use guess_target::Target;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

const APP_CONFIG_DIR: &str = ".crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub enum UI {
    #[default]
    Yacd,
    Zashboard,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]

pub struct AppConfig {
    pub version: String,
    pub config_dir: String,
    pub start_time: usize,
    pub language: Language,
    pub core: CrashCore,
    pub proxy: Proxy,
    pub target: Target,
    pub ui: UI,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let p = format!("{}/{}", app_config_dir(), APP_CONFIG_NAME);
        let c = if std::fs::exists(&p)? {
            let data = std::fs::read_to_string(&p)?;
            let config: AppConfig = serde_json::from_str(&data)?;
            config
        } else {
            let c = AppConfig {
                version: env!("CARGO_PKG_VERSION").to_string(),
                config_dir: app_config_dir(),
                ..Default::default()
            };

            let s = serde_json::to_string_pretty(&c)?;
            mkdir(&app_config_dir());
            std::fs::write(app_config_path(), s)?;
            c
        };

        Ok(c)
    }
}

pub fn app_config_dir() -> String {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()));
    let d = exe.unwrap_or(".".into()).join(APP_CONFIG_DIR);
    d.to_string_lossy().to_string()
}

pub fn app_config_path() -> String {
    let d = app_config_dir();
    format!("{}/{}", d, APP_CONFIG_NAME)
}

pub fn mkdir(dir: &str) {
    if !std::fs::exists(dir).unwrap_or(false)
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        eprintln!("Failed to create config directory {}: {}", dir, e);
    }
}

pub static APP_CONFIG: Lazy<RwLock<AppConfig>> = Lazy::new(|| {
    AppConfig::load()
        .expect("Failed to load crash config")
        .into()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub enum CrashCore {
    #[default]
    Mihomo,
    Clash,
    Singbox,
}

impl CrashCore {
    pub fn name(&self) -> &'static str {
        match self {
            CrashCore::Mihomo => "mihomo",
            CrashCore::Clash => "clash",
            CrashCore::Singbox => "singbox",
        }
    }

    pub fn config_dir(&self) -> String {
        let d = app_config_dir();
        let s = match self {
            CrashCore::Mihomo => ".mihomo_config",
            CrashCore::Clash => ".clash_config",
            CrashCore::Singbox => ".singbox_config",
        };
        format!("{}/{}", d, s)
    }

    pub fn exe_path(&self) -> String {
        let d = self.config_dir();
        let ext = cfg!(target_os = "windows").then(|| ".exe").unwrap_or("");
        format!("{}/{}{ext}", d, self.name())
    }

    pub async fn install(&self) -> Option<()> {
        if std::fs::exists(&self.exe_path()).ok()? {
            return None;
        }

        let config = APP_CONFIG.read().ok()?;

        mkdir(&config.core.config_dir());

        let url = self.core_url();
        println!("url {}", url);
        easy_install::run_main(easy_install::Args {
            url,
            dir: Some(self.config_dir()),
            install_only: true,
            name: vec![],
            alias: Some(self.name().to_string()),
            target: None,
        })
        .await
        .ok()?;
        None
    }

    pub fn release_file_name(&self) -> String {
        use CrashCore::*;
        let target = &APP_CONFIG.read().unwrap().target;
        match (self, target) {
            (Mihomo, Target::X86_64PcWindowsMsvc | Target::X86_64PcWindowsGnu) => {
                // "mihomo-windows-amd64-v1.19.15.zip".to_string()
                "guess-target-x86_64-pc-windows-msvc.zip".to_string()
            }
            _ => todo!("Not support {:?} on {:?}", self, target),
        }
    }
    pub fn repo(&self) -> RepoRelease {
        match self {
            CrashCore::Mihomo => RepoRelease {
                // repo: Repo {
                //     user: "MetaCubeX".to_string(),
                //     repo: "mihomo".to_string(),
                // },
                // tag: "v1.19.15".to_string(),
                // name: self.release_file_name(),
                repo: Repo {
                    user: "ahaoboy".to_string(),
                    repo: "guess-target".to_string(),
                },
                tag: "nightly".to_string(),
                name: self.release_file_name(),
                // https://github.com/ahaoboy/guess-target/releases/download/nightly/
            },
            CrashCore::Clash => todo!(),
            CrashCore::Singbox => todo!(),
        }
    }

    pub fn core_url(&self) -> String {
        let c = APP_CONFIG.read().unwrap();
        c.proxy.url(self.repo())
    }

    pub fn run(&self, args: Vec<String>) -> Option<()> {
        let exe_path = format!("{}/{}", self.config_dir(), self.name());
        std::process::Command::new(exe_path)
            .args(args)
            .spawn()
            .ok()?;
        Some(())
    }
}
