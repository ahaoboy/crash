// Time and duration utilities

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

/// Formats a duration into a human-readable string
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, mins)
    } else {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}

/// Formats uptime from a start timestamp to current time
pub fn format_uptime(start_time: u64) -> String {
    let current = current_timestamp();
    if current < start_time {
        return "0s".to_string();
    }

    let duration = Duration::from_secs(current - start_time);
    format_duration(duration)
}
