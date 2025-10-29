use crate::{
    // common::Language,
    download::download_file,
};
use anyhow::Result;
use github_proxy::{Proxy, Resource};
use guess_target::Target;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use strum::{Display, EnumString, IntoStaticStr};

const APP_CONFIG_DIR: &str = ".crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";

#[derive(
    Debug,
    EnumString,
    Display,
    IntoStaticStr,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Default,
    Deserialize,
    Serialize,
)]
pub enum UI {
    #[default]
    Metacubexd,
    Zashboard,
    Yacd,
}

impl UI {
    pub fn name(&self) -> &'static str {
        self.into()
    }
    pub fn assets_dir(&self) -> String {
        let d = app_config_dir();
        format!("{}/{}", d, self.name())
    }
    pub fn release_file_name(&self) -> String {
        use UI::*;
        match self {
            Yacd => "yacd.tar.xz".to_string(),
            Zashboard => "zashboard.zip".to_string(),
            Metacubexd => "metacubexd.tgz".to_string(),
        }
    }
    pub fn url(&self) -> String {
        let c = APP_CONFIG.read().unwrap();
        c.proxy
            .url(Resource::Release {
                owner: "ahaoboy".to_string(),
                repo: "crash-assets".to_string(),
                tag: "nightly".to_string(),
                name: self.release_file_name(),
            })
            .expect("Failed to get proxy url")
    }

    pub async fn install(&self) -> Option<()> {
        if std::fs::exists(self.assets_dir()).ok()? {
            return None;
        }

        let url = self.url();
        easy_install::run_main(easy_install::Args {
            url,
            dir: Some(self.assets_dir()),
            install_only: true,
            name: vec![],
            alias: None,
            target: None,
            retry: 3,
            proxy: Proxy::Github,
            timeout: 600,
        })
        .await
        .ok()?;

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]

pub struct CrashConfig {
    pub version: String,
    pub config_dir: String,
    pub start_time: usize,
    // pub language: Language,
    pub core: CrashCore,
    pub proxy: Proxy,
    pub target: Target,
    pub ui: UI,
    pub url: String,
}

impl CrashConfig {
    pub fn load() -> anyhow::Result<Self> {
        let p = format!("{}/{}", app_config_dir(), APP_CONFIG_NAME);
        let c = if std::fs::exists(&p)? {
            let data = std::fs::read_to_string(&p)?;
            let config: CrashConfig = serde_json::from_str(&data)?;
            config
        } else {
            CrashConfig {
                version: env!("CARGO_PKG_VERSION").to_string(),
                config_dir: app_config_dir(),
                ..Default::default()
            }
        };
        c.save()?;
        Ok(c)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        mkdir(&app_config_dir());
        std::fs::write(app_config_path(), s)?;
        Ok(())
    }

    /// Update GeoIP database
    pub async fn update_geoip(&self) -> Result<()> {
        let databases = vec![
            "china_ip_list.txt",
            "china_ipv6_list.txt",
            "cn_mini.mmdb",
            "Country.mmdb",
            "geoip_cn.db",
            "geosite.dat",
            "geosite_cn.db",
            "mrs_geosite_cn.mrs",
            "srs_geoip_cn.srs",
            "srs_geosite_cn.srs",
        ];

        for db in databases {
            let db_path = format!("{}/{}", self.config_dir, db);
            if !std::fs::exists(db_path).unwrap_or(false) {
                self.download_geoip(db).await?;
            }
        }
        Ok(())
    }

    pub async fn download_geoip(&self, db_type: &str) -> Result<()> {
        let dest = format!("{}/{}", self.config_dir, db_type);
        let url = Resource::File {
            owner: "juewuy".to_string(),
            repo: "ShellCrash".to_string(),
            reference: "master".to_string(),
            path: format!("bin/geodata/{}", db_type),
        }.url(&self.proxy).expect("Failed to get geo url");;

        download_file(&url, &dest).await?;
        Ok(())
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

pub static APP_CONFIG: Lazy<RwLock<CrashConfig>> = Lazy::new(|| {
    CrashConfig::load()
        .expect("Failed to load crash config")
        .into()
});

#[derive(
    Debug,
    EnumString,
    Display,
    IntoStaticStr,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Default,
    Deserialize,
    Serialize,
)]
pub enum CrashCore {
    #[default]
    Mihomo,
    Clash,
    Singbox,
}

impl CrashCore {
    pub fn name(&self) -> &'static str {
        self.into()
    }
    pub fn exe_path(&self) -> String {
        let d = app_config_dir();
        let ext = cfg!(target_os = "windows").then(|| ".exe").unwrap_or("");
        format!("{}/{}{ext}", d, self.name())
    }
    pub fn make_config(&self) {
        match self {
            CrashCore::Mihomo => {
                let config_path = self.config_path();
                if !std::fs::exists(&config_path).unwrap_or(false) {
                    let c = APP_CONFIG.read().unwrap();
                    let mut default_config: serde_clash::Config =
                        serde_yaml::from_str(include_str!("./assets/mihomo.yaml"))
                            .expect("Failed to load default config");

                    default_config.external_ui = format!("./{}", c.ui.name());

                    if let Ok(config_str) = serde_yaml::to_string(&default_config)
                        && let Err(e) = std::fs::write(&config_path, config_str)
                    {
                        eprintln!("Failed to write default mihomo config: {}", e);
                    }
                }
            }
            _ => {
                todo!()
            }
        }
    }

    pub async fn install(&self) -> Option<()> {
        self.make_config();

        if std::fs::exists(self.exe_path()).ok()? {
            return None;
        }

        let config = APP_CONFIG.read().ok()?;

        mkdir(&config.config_dir);

        let url = self.core_url();
        easy_install::run_main(easy_install::Args {
            url,
            dir: Some(config.config_dir.clone()),
            install_only: true,
            name: vec![],
            alias: Some(self.name().to_string()),
            target: None,
            retry: 3,
            proxy: Proxy::Github,
            timeout: 600,
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
                "mihomo-windows-amd64-v1.19.15.zip".to_string()
            }
            (Mihomo, Target::Aarch64UnknownLinuxMusl) => {
                "mihomo-linux-arm64-v1.19.15.tgz".to_string()
            }
            (Mihomo, Target::X86_64UnknownLinuxGnu) => {
                "mihomo-linux-amd64-v1.19.15.tgz".to_string()
            }
            _ => todo!("Not support {:?} on {:?}", self, target),
        }
    }
    pub fn repo(&self) -> Resource {
        match self {
            CrashCore::Mihomo => Resource::Release {
                owner: "ahaoboy".to_string(),
                repo: "crash-assets".to_string(),
                tag: "nightly".to_string(),
                name: self.release_file_name(),
            },
            CrashCore::Clash => todo!(),
            CrashCore::Singbox => todo!(),
        }
    }

    pub fn core_url(&self) -> String {
        let c = APP_CONFIG.read().unwrap();
        c.proxy.url(self.repo()).expect("Failed to get core url")
    }

    pub fn run(&self, args: Vec<String>) -> Option<()> {
        let exe_path = self.exe_path();
        std::process::Command::new(exe_path)
            .args(args)
            .spawn()
            .ok()?;
        Some(())
    }

    pub fn stop(&self) -> Option<()> {
        use std::process::Command;
        let name = self.name();
        if cfg!(target_os = "windows") {
            Command::new("taskkill")
                .args(["/F", "/IM", &format!("{name}.exe")])
                .spawn()
                .ok()?;
        } else {
            Command::new("pkill").args([name]).spawn().ok()?;
        }
        Some(())
    }

    pub fn config_file_name(&self) -> String {
        format!("{}.yaml", self.name())
    }

    pub fn config_path(&self) -> String {
        let c = APP_CONFIG.read().unwrap();
        format!("{}/{}", c.config_dir, self.config_file_name())
    }

    pub async fn update(&self, url: &str) -> Option<()> {
        let dest = self.config_path();
        if std::fs::exists(&dest).unwrap_or(false) {
            return Some(());
        }
        download_file(url, &dest).await.ok()?;
        Some(())
    }
}
