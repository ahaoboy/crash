use crate::{
    // common::Language,
    download::download_file,
    tools::{exec::exec, stop},
};
use anyhow::Result;
use github_proxy::{Proxy, Resource};
use guess_target::Target;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    process::Stdio,
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{Display, EnumString, IntoStaticStr};

const APP_CONFIG_DIR: &str = ".crash_config";
const APP_CONFIG_NAME: &str = "crash_config.json";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Web {
    pub ui: UI,
    pub host: String,
    pub secret: String,
}

impl Default for Web {
    fn default() -> Self {
        Self {
            ui: Default::default(),
            host: ":9090".to_string(),
            secret: Default::default(),
        }
    }
}

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

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

#[cfg(unix)]
pub fn get_pid(name: &str) -> anyhow::Result<u64> {
    use anyhow::Context;
    let s = exec("pidof", vec![name])?;
    let pid = s
        .trim()
        .split_whitespace()
        .next()
        .context("No process found")?
        .parse::<u64>()?;
    Ok(pid)
}

#[cfg(windows)]
pub fn get_pid(name: &str) -> anyhow::Result<u64> {
    let output = exec("tasklist", vec!["/FI", &format!("IMAGENAME eq {name}")])?;
    for line in output.lines() {
        if line.to_lowercase().starts_with(&name.to_lowercase())
            && let Some(pid_str) = line.split_whitespace().nth(1)
            && let Ok(pid) = pid_str.parse::<u64>()
        {
            return Ok(pid);
        }
    }

    Err(anyhow::format_err!("Process '{}' not found", name))
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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]

pub struct CrashConfig {
    pub version: String,
    pub config_dir: String,
    pub start_time: u64,
    // pub language: Language,
    pub core: CrashCore,
    pub proxy: Proxy,
    pub target: Target,
    pub web: Web,
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
    pub fn config_path(&self) -> String {
        format!("{}/{}", self.config_dir, self.core.config_file_name())
    }

    pub fn restart(&mut self) -> anyhow::Result<()> {
        if get_pid(&self.core.exe_name()).is_ok() {
            self.stop()?;
        }
        self.start()?;
        Ok(())
    }
    pub async fn update(&self, url: &str) -> Option<()> {
        let dest = self.config_path();
        if std::fs::exists(&dest).unwrap_or(false) {
            return Some(());
        }
        download_file(url, &dest).await.ok()?;
        Some(())
    }

    pub fn install_task(&self) -> anyhow::Result<()> {
        let cron = "0 3 * * 3";
        let exe = std::env::current_exe()?;
        let exe_path = exe.to_string_lossy();
        let cmd = format!("{} run-task", exe_path);
        let s = format!("{} {}", cron, cmd);

        if let Ok(list) = exec("crontab", vec!["-l"])
            && !list.lines().any(|line| line == s)
        {
            let sh = format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", s);
            exec("bash", vec!["-c", &sh])?;
        }
        Ok(())
    }

    pub async fn update_url(&self, force: bool) -> anyhow::Result<()> {
        let dest = self.config_path();
        if std::fs::exists(&dest).unwrap_or(false) && !force {
            return Ok(());
        }
        download_file(&self.url, &dest).await?;
        Ok(())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        mkdir(&app_config_dir());
        std::fs::write(app_config_path(), s)?;
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        stop::stop_process(&self.core.exe_path())?;
        self.start_time = 0;
        self.save()?;
        Ok(())
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        let v = vec![
            "-f".to_string(),
            self.config_path(),
            "-ext-ctl".to_string(),
            self.web.host.clone(),
            "-ext-ui".to_string(),
            self.web.ui.name().to_string(),
            "-d".to_string(),
            self.config_dir.to_string(),
        ];
        self.core.run(v);
        self.start_time = now();
        self.save()?;
        Ok(())
    }
    pub fn ensure_config(&self) {
        match self.core {
            CrashCore::Mihomo => {
                let config_path = self.config_path();
                if !std::fs::exists(&config_path).unwrap_or(false)
                    && let Err(e) =
                        std::fs::write(&config_path, include_str!("./assets/mihomo.yaml"))
                {
                    eprintln!("Failed to write default mihomo config: {}", e);
                }
            }
            _ => {
                todo!()
            }
        }
    }

    pub fn core_url(&self) -> String {
        self.proxy
            .url(self.core.repo())
            .expect("Failed to get core url")
    }

    pub fn core_version(&self) -> Option<String> {
        match self.core {
            CrashCore::Mihomo => {
                let s = exec(self.core.exe_path(), vec!["-v"]).ok()?;
                s.split_whitespace().nth(2).map(|s| s.to_string())
            }
            _ => None,
        }
    }
    /// Update GeoIP database
    pub async fn update_geo(&self, force: bool) -> Result<()> {
        match self.core {
            CrashCore::Clash => {
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
                    if !std::fs::exists(db_path).unwrap_or(false) || force {
                        let dest = format!("{}/{}", self.config_dir, db);
                        let url = Resource::File {
                            owner: "juewuy".to_string(),
                            repo: "ShellCrash".to_string(),
                            reference: "master".to_string(),
                            path: format!("bin/geodata/{}", db),
                        }
                        .url(&self.proxy)
                        .expect("Failed to get geo url");
                        download_file(&url, &dest).await?;
                    }
                }
            }
            CrashCore::Mihomo => {
                let databases = vec!["geoip.metadb", "geoip.dat", "geosite.dat"];
                for db in databases {
                    let db_path = format!("{}/{}", self.config_dir, db);
                    if !std::fs::exists(db_path).unwrap_or(false) || force {
                        let dest = format!("{}/{}", self.config_dir, db);
                        let url = Resource::Release {
                            owner: "MetaCubeX".to_string(),
                            repo: "meta-rules-dat".to_string(),
                            tag: "latest".to_string(),
                            name: db.to_string(),
                        }
                        .url(&self.proxy)
                        .expect("Failed to get geo url");
                        download_file(&url, &dest).await?;
                    }
                }
            }
            _ => todo!(),
        }
        Ok(())
    }

    pub fn status(&self) -> String {
        let core_status = if let Some(version) = self.core_version() {
            format!("{}({})", self.core, version)
        } else {
            self.core.to_string()
        };

        let running_status = if get_pid(&self.core.exe_name()).is_ok() {
            "✅".to_string()
        } else {
            "❌".to_string()
        };
        let mut v = vec![
            ("version", env!("CARGO_PKG_VERSION").to_string()),
            ("core", core_status),
        ];

        if let Ok(pid) = get_pid(&self.core.exe_name()) {
            v.push(("pid", pid.to_string()));
        }

        if let Ok(ip) = local_ip_address::local_ip() {
            let port = self.web.host.split(":").nth(1).unwrap_or("9090");
            v.push(("web", format!("{}(http://{}:{}/ui)", self.web.ui, ip, port)));
        }

        if let Some(memory) = exec(
            "cat",
            vec!["/proc/{}/status | grep VmRSS | awk '{{print $2}}'"],
        )
        .ok()
        .and_then(|i| i.parse::<usize>().ok())
        {
            v.push(("memory", humansize::format_size(memory, humansize::DECIMAL)));
        }

        let duration = std::time::Duration::from_secs(if get_pid(&self.core.exe_name()).is_ok() {
            now() - self.start_time
        } else {
            0
        });
        let time = humantime::format_duration(duration).to_string();
        v.push(("status", format!("{} {}", running_status, time)));

        let key_len = v.iter().fold(0, |a, b| a.max(b.0.len()));
        v.iter()
            .map(|(k, v)| format!("{:width$} : {}", k, v, width = key_len))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub async fn install(&self, force: bool) -> Option<()> {
        self.ensure_config();
        self.install_ui(force).await;
        self.install_core(force).await;
        Some(())
    }
    pub async fn install_ui(&self, force: bool) -> Option<()> {
        if std::fs::exists(self.web.ui.assets_dir()).ok()? && !force {
            return None;
        }
        let url = self.web.ui.url();
        easy_install::run_main(easy_install::Args {
            url,
            dir: Some(self.web.ui.assets_dir()),
            install_only: true,
            name: vec![],
            alias: None,
            target: None,
            retry: 3,
            proxy: self.proxy,
            timeout: 600,
        })
        .await
        .ok()?;

        None
    }

    pub async fn install_core(&self, force: bool) -> Option<()> {
        if std::fs::exists(self.core.exe_path()).ok()? && !force {
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
            alias: Some(self.core.name().to_string()),
            target: None,
            retry: 3,
            proxy: self.proxy,
            timeout: 600,
        })
        .await
        .ok()?;
        None
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
    pub fn exe_name(&self) -> String {
        let ext = cfg!(target_os = "windows").then(|| ".exe").unwrap_or("");
        format!("{}{ext}", self.name())
    }

    pub fn exe_path(&self) -> String {
        let d = app_config_dir();
        format!("{}/{}", d, self.exe_name())
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

    pub fn run(&self, args: Vec<String>) -> Option<()> {
        let exe_path = self.exe_path();
        std::process::Command::new(exe_path)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
}
