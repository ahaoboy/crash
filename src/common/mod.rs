// Common utilities and shared functionality

pub mod config;
pub mod error;
pub mod i18n;
pub mod logger;
pub mod shell;

// Re-export commonly used items
pub use config::Config;
pub use error::{Result, ShellCrashError};
pub use i18n::{get_language, set_language, Language};
pub use logger::Logger;
pub use shell::ShellExecutor;
