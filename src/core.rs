use crate::{common::Language, download::Proxy};
use guess_target::Target;
use once_cell::sync::Lazy;
use std::sync::RwLock;

const APP_CONFIG_DIR: &str = ".crash_config";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AppConfig {
    version: String,
    config_dir: String,
    start_time: usize,
    language: Language,
    core: CrashCore,
    proxy: Proxy,
    target: Target,
}

fn app_config_dir() -> String {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()));
    let d = exe.unwrap_or(".".into()).join(APP_CONFIG_DIR);
    d.to_string_lossy().to_string()
}

pub fn ensure_app_config_dir() {
    let dir = app_config_dir();
    if !std::fs::exists(&dir).unwrap_or(false) {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("Failed to create config directory {}: {}", dir, e);
        }
    }
}

pub static APP_CONFIG: Lazy<RwLock<AppConfig>> = Lazy::new(|| {
    AppConfig {
        version: env!("CARGO_PKG_VERSION").to_string(),
        config_dir: app_config_dir(),
        ..Default::default()
    }
    .into()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum CrashCore {
    #[default]
    Mihomo,
    Clash,
    Singbox,
}
