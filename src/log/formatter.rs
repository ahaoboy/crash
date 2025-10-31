// Log message formatting

use crate::log::LogLevel;
use chrono::Local;

pub struct LogFormatter;

impl LogFormatter {
    /// Format a log message with timestamp, level, module, and message
    pub fn format_with_timestamp(level: LogLevel, module: &str, message: &str) -> String {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let sanitized_message = Self::sanitize_sensitive_info(message);
        format!(
            "[{}] [{}] [{}] {}",
            timestamp,
            level.as_str(),
            module,
            sanitized_message
        )
    }

    /// Format a log message without timestamp
    pub fn format(level: LogLevel, module: &str, message: &str) -> String {
        let sanitized_message = Self::sanitize_sensitive_info(message);
        format!("[{}] [{}] {}", level.as_str(), module, sanitized_message)
    }

    /// Sanitize sensitive information from log messages
    /// Replaces common patterns for secrets, passwords, tokens, etc.
    fn sanitize_sensitive_info(message: &str) -> String {
        let mut sanitized = message.to_string();

        // List of sensitive keywords to redact
        let sensitive_patterns = [
            ("password", "password="),
            ("secret", "secret="),
            ("token", "token="),
            ("api_key", "api_key="),
            ("apikey", "apikey="),
            ("auth", "auth="),
            ("authorization", "authorization:"),
        ];

        for (_keyword, pattern) in &sensitive_patterns {
            if let Some(pos) = sanitized.to_lowercase().find(pattern) {
                let start = pos + pattern.len();

                // Find the end of the value (space, comma, quote, or end of string)
                let end = sanitized[start..]
                    .find(|c: char| c.is_whitespace() || c == ',' || c == '"' || c == '\'')
                    .map(|i| start + i)
                    .unwrap_or(sanitized.len());

                // Replace the value with asterisks
                if end > start {
                    let replacement = "*".repeat(end - start);
                    sanitized.replace_range(start..end, &replacement);
                }
            }
        }

        sanitized
    }
}
