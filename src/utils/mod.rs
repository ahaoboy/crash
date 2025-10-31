// Utility modules for shared functionality
pub mod fs;
pub mod time;

pub use fs::{atomic_write, ensure_dir, file_exists};
pub use time::{current_timestamp, format_uptime};
