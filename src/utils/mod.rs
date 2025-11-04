// Utility modules for shared functionality
pub mod fs;
pub mod time;

pub use fs::{atomic_write, ensure_dir, file_exists};
pub use time::{current_timestamp, format_uptime};

use crate::platform::command::execute;

pub fn get_user() -> String {
    if let Ok(v) = std::env::var("USER") {
        return v;
    }

    if let Ok(v) = execute("whoami", &[]) {
        return v;
    }

    return "root".to_string();
}
