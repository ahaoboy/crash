use std::sync::RwLock;
use once_cell::sync::Lazy;
use crate::common::Language;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConfig {
    version: String,
    start_time: usize,
    language: Language,
}

pub static APP_CONFIG: Lazy<RwLock<AppConfig>> = Lazy::new(|| {
    AppConfig {
        version: env!("CARGO_PKG_VERSION").to_string(),
        start_time: 0,
        language: Language::Chinese,
    }
    .into()
});
