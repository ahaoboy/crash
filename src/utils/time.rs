// Time and duration utilities

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humantime::format_duration;

/// Returns the current Unix timestamp in seconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

/// Formats uptime from a start timestamp to current time
pub fn format_uptime(start_time: u64) -> String {
    let current = current_timestamp();
    if current < start_time {
        return "0s".to_string();
    }

    let duration = Duration::from_secs(current - start_time);
    format_duration(duration).to_string()
}
