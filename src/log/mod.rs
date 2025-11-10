// Logging infrastructure for the Crash application

use crate::config::get_log_dir;
use crate::error::{CrashError, Result};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod file_appender;
mod formatter;

pub use file_appender::FileAppender;
pub use formatter::LogFormatter;

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Configuration for the logging system
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub log_dir: PathBuf,
    pub log_level: LogLevel,
    pub max_file_size: u64,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_dir: get_log_dir(),
            log_level: LogLevel::Info,
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Global logger instance
pub struct Logger {
    config: LogConfig,
    appender: Arc<Mutex<FileAppender>>,
}

impl Logger {
    fn new(config: LogConfig) -> Result<Self> {
        let appender = FileAppender::new(config.log_dir.clone(), config.max_file_size)?;

        Ok(Self {
            config,
            appender: Arc::new(Mutex::new(appender)),
        })
    }

    pub fn log(&self, level: LogLevel, module: &str, message: &str) {
        if level < self.config.log_level {
            return;
        }

        let formatted = LogFormatter::format_with_timestamp(level, module, message);

        if let Ok(mut appender) = self.appender.lock() {
            let _ = appender.write_log(level, &formatted);
        }
    }
}

static LOGGER: Lazy<Arc<Mutex<Option<Logger>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

/// Initialize the logging system with the given configuration
pub fn init_logger(config: LogConfig) -> Result<()> {
    let logger = Logger::new(config)?;

    if let Ok(mut global_logger) = LOGGER.lock() {
        *global_logger = Some(logger);
        Ok(())
    } else {
        Err(CrashError::Log("Failed to acquire logger lock".to_string()))
    }
}

/// Get a reference to the global logger
pub fn get_logger() -> Arc<Mutex<Option<Logger>>> {
    LOGGER.clone()
}

/// Log a message at the specified level
pub fn log(level: LogLevel, module: &str, message: &str) {
    if let Ok(logger_guard) = LOGGER.lock()
        && let Some(logger) = logger_guard.as_ref()
    {
        logger.log(level, module, message);
    }
}

/// Logging macros for convenient use throughout the codebase
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::LogLevel::Trace, module_path!(), &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::LogLevel::Debug, module_path!(), &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::LogLevel::Info, module_path!(), &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::LogLevel::Warn, module_path!(), &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::LogLevel::Error, module_path!(), &format!($($arg)*))
    };
}
