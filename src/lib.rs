// Crash - A tool for managing proxy cores like Clash/Mihomo/SingBox
// Refactored version with improved modularity and error handling

pub mod cli;
pub mod config;
pub mod core;
pub mod download;
pub mod error;
pub mod log;
pub mod platform;
pub mod process;
pub mod utils;

// Re-export commonly used types
pub use error::{CrashError, Result};
